//! TCP control session: port of `Common/Network/Client/TCPClientHandler.cs`.
//!
//! One line of JSON per message. Flow:
//! 1. connect (10 s timeout), `TCP_NODELAY`, `SO_KEEPALIVE`;
//! 2. send `SYNC` with our initial [`SrClient`];
//! 3. loop: handle server lines, send updates on command, check the idle
//!    timeout every 30 s;
//! 4. on disconnect, clear the client list.
//!
//! Quirks kept from C#:
//! - `RADIO_UPDATE` is sent on every request (`SRClientBase.Equals` is the
//!   reference comparison, so the "only on change" test is always true; the
//!   real rate limiting happens in `DCSRadioSyncHandler`).
//! - `UPDATE` (metadata) is sent only when `MetaDataEquals` differs or
//!   180 s passed.
//! - The position is zeroed unless `DISTANCE_ENABLED` or `LOS_ENABLED`.
//! - More than 5 consecutive undecodable lines → disconnect with a version
//!   mismatch warning.
//! - There is no automatic reconnect.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use srs_protocol::constants::CLIENT_UPDATE_INTERVAL_LIMIT_SECS;
use srs_protocol::{
    LatLng, MessageType, NetworkMessage, ProtocolVersion, ServerSettingsKey, SrClient, MIN_PROTOCOL_VERSION,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::shared::{ConnectedClient, SharedState};

const MAX_DECODE_ERRORS: u32 = 5;

#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Four-part client version stamped on every message.
    pub client_version: String,
    /// `IdleTimeOut` setting (seconds without sending). Default 600.
    pub idle_timeout: Duration,
    /// How often the idle timeout is checked. 30 s in C#.
    pub idle_check_interval: Duration,
    /// Connection timeout. 10 s in C#.
    pub connect_timeout: Duration,
    /// `SendTimeout`. 90 s in C#.
    pub send_timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            client_version: "2.4.1.0".to_string(),
            idle_timeout: Duration::from_secs(600),
            idle_check_interval: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            send_timeout: Duration::from_secs(90),
        }
    }
}

/// Commands from the rest of the client to the session task.
#[derive(Debug)]
pub enum NetCommand {
    /// Full update (`UnitUpdateMessage { FullUpdate = true }`).
    SendRadioUpdate(SrClient),
    /// Metadata-only update (`UnitUpdateMessage { FullUpdate = false }`).
    SendMetadata(SrClient),
    /// `EAMConnectRequestMessage`: name + password.
    EamLogin { name: String, password: String },
    /// Client-side `PING` (unused by the C# client, available anyway).
    Ping,
    /// `DisconnectRequestMessage`.
    Disconnect,
}

/// Why the session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
    /// Requested by the application.
    Requested,
    /// `CheckIfIdleTimeOut`.
    IdleTimeout,
    /// Server closed the socket or an I/O error occurred.
    ConnectionLost(String),
    /// Server too old, `VERSION_MISMATCH`, or too many decode errors.
    VersionMismatch { server_version: String },
    /// The server did not send a `Version` in its SYNC.
    UnversionedServer,
}

/// Events from the session task to the rest of the client.
#[derive(Debug, Clone)]
pub enum NetEvent {
    /// Socket connected and SYNC sent (`TCPClientStatusMessage(true)`).
    Connected(SocketAddr),
    /// Could not connect (`TCPClientStatusMessage(false, TIMEOUT)`).
    ConnectFailed(String),
    /// Server SYNC reply processed; `SharedState` now holds the client list
    /// and server settings.
    Synced {
        server_version: String,
        client_count: usize,
    },
    /// A client entry was added or updated (`SRClientUpdateMessage`).
    ClientUpdated { guid: String, full: bool },
    /// A client left (`SRClientUpdateMessage(client, false)`).
    ClientRemoved(String),
    /// `SERVER_SETTINGS` received; `SharedState::server_settings` updated.
    ServerSettingsUpdated,
    /// `EXTERNAL_AWACS_MODE_PASSWORD` reply. `coalition == 0` means refused
    /// (`EAMDisconnectMessage`), else `EAMConnectedMessage(coalition)`.
    EamResult { coalition: i32 },
    /// Server forced us out of EAM (`EXTERNAL_AWACS_MODE` turned off).
    EamDisconnected,
    /// Session ended (`TCPClientStatusMessage(false)`).
    Disconnected(DisconnectReason),
}

/// Handle to a running session.
#[derive(Clone)]
pub struct TcpHandle {
    cmd_tx: mpsc::Sender<NetCommand>,
}

impl TcpHandle {
    pub async fn send(&self, cmd: NetCommand) -> bool {
        self.cmd_tx.send(cmd).await.is_ok()
    }

    /// Non-async variant for UI threads.
    pub fn try_send(&self, cmd: NetCommand) -> bool {
        self.cmd_tx.try_send(cmd).is_ok()
    }

    pub fn disconnect(&self) {
        let _ = self.cmd_tx.try_send(NetCommand::Disconnect);
    }
}

pub struct TcpSession;

impl TcpSession {
    /// Resolve `host[:port]` to the first IPv4 address, like the C# client.
    pub async fn resolve(address: &str) -> Result<SocketAddr, String> {
        let address = address.trim();
        let (host, port) = match address.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (
                h.to_string(),
                p.parse::<u16>()
                    .map_err(|_| "specified port is not valid".to_string())?,
            ),
            _ => (address.to_string(), srs_protocol::constants::DEFAULT_SERVER_PORT),
        };
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|e| e.to_string())?
            .collect();
        addrs
            .into_iter()
            .find(|a| a.is_ipv4())
            .ok_or_else(|| "Invalid IP or Host Name!".to_string())
    }

    /// Spawn the session task on the current tokio runtime.
    ///
    /// `initial` is the client state sent in the SYNC request (name, GUID,
    /// radios, coalition 0, seat 0, empty position).
    pub fn spawn(
        endpoint: SocketAddr,
        initial: SrClient,
        shared: Arc<SharedState>,
        config: SessionConfig,
    ) -> (TcpHandle, mpsc::Receiver<NetEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (evt_tx, evt_rx) = mpsc::channel(256);
        tokio::spawn(run(endpoint, initial, shared, config, cmd_rx, evt_tx));
        (TcpHandle { cmd_tx }, evt_rx)
    }
}

struct Session {
    shared: Arc<SharedState>,
    config: SessionConfig,
    evt_tx: mpsc::Sender<NetEvent>,
    player_state: SrClient,
    last_sent: Instant,
    writer: tokio::net::tcp::OwnedWriteHalf,
}

async fn run(
    endpoint: SocketAddr,
    initial: SrClient,
    shared: Arc<SharedState>,
    config: SessionConfig,
    mut cmd_rx: mpsc::Receiver<NetCommand>,
    evt_tx: mpsc::Sender<NetEvent>,
) {
    shared.clear();
    log::info!("TryConnect @ {endpoint}");
    let stream = match tokio::time::timeout(config.connect_timeout, TcpStream::connect(endpoint)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            log::error!("Could not connect to server {endpoint}: {e}");
            let _ = evt_tx.send(NetEvent::ConnectFailed(e.to_string())).await;
            return;
        }
        Err(_) => {
            log::error!("Failed to connect to server @ {endpoint} (timeout)");
            let _ = evt_tx.send(NetEvent::ConnectFailed("timeout".into())).await;
            return;
        }
    };
    if let Err(e) = stream.set_nodelay(true) {
        log::warn!("set_nodelay failed: {e}");
    }
    if let Err(e) = set_keepalive(&stream) {
        log::warn!("SO_KEEPALIVE failed: {e}");
    }

    let (read_half, writer) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();
    let mut session = Session {
        shared,
        config,
        evt_tx: evt_tx.clone(),
        player_state: initial,
        last_sent: Instant::now(),
        writer,
    };

    // SYNC request.
    let sync = NetworkMessage::with_client(MessageType::Sync, session.player_state.clone());
    if let Err(e) = session.send(&sync).await {
        let _ = evt_tx
            .send(NetEvent::Disconnected(DisconnectReason::ConnectionLost(e)))
            .await;
        return;
    }
    let _ = evt_tx.send(NetEvent::Connected(endpoint)).await;

    let mut idle_tick = tokio::time::interval(session.config.idle_check_interval);
    idle_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    idle_tick.tick().await; // first tick fires immediately; skip it
    let mut decode_errors = 0u32;

    let reason = loop {
        tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        match NetworkMessage::from_line(&line) {
                            Ok(msg) => {
                                decode_errors = 0;
                                if let Some(reason) = session.handle_message(msg).await {
                                    break reason;
                                }
                            }
                            Err(e) => {
                                decode_errors += 1;
                                log::error!("Client exception reading from socket: {e} ({line:.200})");
                                if decode_errors > MAX_DECODE_ERRORS {
                                    break DisconnectReason::VersionMismatch { server_version: "unknown".into() };
                                }
                            }
                        }
                    }
                    Ok(None) => break DisconnectReason::ConnectionLost("server closed the connection".into()),
                    Err(e) => break DisconnectReason::ConnectionLost(e.to_string()),
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(NetCommand::Disconnect) | None => break DisconnectReason::Requested,
                    Some(cmd) => {
                        if let Err(e) = session.handle_command(cmd).await {
                            break DisconnectReason::ConnectionLost(e);
                        }
                    }
                }
            }
            _ = idle_tick.tick() => {
                if session.last_sent.elapsed() > session.config.idle_timeout {
                    log::warn!("Disconnecting - Idle Time out");
                    break DisconnectReason::IdleTimeout;
                }
            }
        }
    };

    log::info!("Disconnecting from server... ({reason:?})");
    let _ = session.writer.shutdown().await;
    session.shared.clear();
    let _ = evt_tx.send(NetEvent::Disconnected(reason)).await;
    log::info!("Disconnected.");
}

fn set_keepalive(stream: &TcpStream) -> std::io::Result<()> {
    socket2::SockRef::from(stream).set_keepalive(true)
}

impl Session {
    fn need_valid_position(&self) -> bool {
        let s = self.shared.server_settings.read().expect("server settings lock");
        s.get_bool(ServerSettingsKey::DISTANCE_ENABLED) || s.get_bool(ServerSettingsKey::LOS_ENABLED)
    }

    async fn send(&mut self, msg: &NetworkMessage) -> Result<(), String> {
        self.last_sent = Instant::now();
        let line = msg.to_line(&self.config.client_version);
        if msg.msg_type == MessageType::RadioUpdate {
            log::debug!("Sending Radio Update To Server: {line:.300}");
        }
        tokio::time::timeout(self.config.send_timeout, self.writer.write_all(line.as_bytes()))
            .await
            .map_err(|_| "send timeout".to_string())?
            .map_err(|e| {
                log::error!("Client exception sending to server: {e}");
                e.to_string()
            })
    }

    async fn handle_command(&mut self, cmd: NetCommand) -> Result<(), String> {
        match cmd {
            NetCommand::SendRadioUpdate(mut updated) => {
                // C# compares by reference here, so this always sends.
                if self.need_valid_position() {
                    // keep updated.lat_lng_position
                } else {
                    updated.lat_lng_position = Some(LatLng::default());
                }
                self.player_state = updated.clone();
                self.send(&NetworkMessage::with_client(MessageType::RadioUpdate, updated))
                    .await
            }
            NetCommand::SendMetadata(mut updated) => {
                let need_pos = self.need_valid_position();
                let changed = !self.player_state.metadata_same_as(&updated, need_pos);
                let stale = self.last_sent.elapsed().as_secs() > CLIENT_UPDATE_INTERVAL_LIMIT_SECS;
                if changed || stale {
                    self.player_state.allow_record = updated.allow_record;
                    self.player_state.coalition = updated.coalition;
                    self.player_state.name = updated.name.clone();
                    self.player_state.seat = updated.seat;
                    self.player_state.lat_lng_position = updated.lat_lng_position;
                    updated.radio_info = None;
                    if !need_pos {
                        updated.lat_lng_position = Some(LatLng::default());
                    }
                    self.send(&NetworkMessage::with_client(MessageType::Update, updated))
                        .await
                } else {
                    Ok(())
                }
            }
            NetCommand::EamLogin { name, password } => {
                self.player_state.set_name(&name);
                let mut msg = NetworkMessage::with_client(
                    MessageType::ExternalAwacsModePassword,
                    self.player_state.clone(),
                );
                msg.external_awacs_mode_password = Some(password);
                self.send(&msg).await
            }
            NetCommand::Ping => self.send(&NetworkMessage::new(MessageType::Ping)).await,
            NetCommand::Disconnect => Ok(()),
        }
    }

    /// Returns `Some(reason)` when the session must end.
    async fn handle_message(&mut self, msg: NetworkMessage) -> Option<DisconnectReason> {
        log::debug!("Received {:?}", msg.msg_type);
        match msg.msg_type {
            MessageType::Ping => {}
            MessageType::RadioUpdate | MessageType::Update => {
                if let Some(settings) = &msg.server_settings {
                    self.decode_server_settings(settings);
                }
                let Some(client) = msg.client else {
                    log::error!("{:?} without Client", msg.msg_type);
                    return None;
                };
                let full = msg.msg_type == MessageType::RadioUpdate;
                let guid = client.client_guid.clone();
                {
                    let mut clients = self.shared.clients.write().expect("clients lock");
                    match clients.get_mut(&guid) {
                        Some(existing) => {
                            let e = &mut existing.client;
                            e.allow_record = client.allow_record;
                            e.client_guid = client.client_guid;
                            e.coalition = client.coalition;
                            e.lat_lng_position = client.lat_lng_position;
                            e.name = client.name;
                            e.seat = client.seat;
                            e.dis_entity_id = client.dis_entity_id;
                            if full {
                                if let Some(ri) = client.radio_info {
                                    e.radio_info = Some(ri);
                                }
                            }
                        }
                        None => {
                            clients.insert(
                                guid.clone(),
                                ConnectedClient {
                                    client,
                                    local: Default::default(),
                                },
                            );
                        }
                    }
                }
                let _ = self.evt_tx.send(NetEvent::ClientUpdated { guid, full }).await;
            }
            MessageType::Sync => {
                let Some(version) = msg.version.clone() else {
                    log::error!("Disconnecting Unversioned Server");
                    return Some(DisconnectReason::UnversionedServer);
                };
                let compatible = version
                    .parse::<ProtocolVersion>()
                    .map(|v| v >= MIN_PROTOCOL_VERSION)
                    .unwrap_or(false);
                self.shared
                    .server_settings
                    .write()
                    .expect("server settings lock")
                    .server_version = Some(version.clone());
                if !compatible {
                    log::error!(
                        "Server version ({version}) older than minimum protocol version ({MIN_PROTOCOL_VERSION}) - disconnecting"
                    );
                    return Some(DisconnectReason::VersionMismatch {
                        server_version: version,
                    });
                }
                let mut count = 0;
                if let Some(clients) = msg.clients {
                    let mut map = self.shared.clients.write().expect("clients lock");
                    for c in clients {
                        count += 1;
                        map.insert(
                            c.client_guid.clone(),
                            ConnectedClient {
                                client: c,
                                local: Default::default(),
                            },
                        );
                    }
                }
                if let Some(settings) = &msg.server_settings {
                    self.decode_server_settings(settings);
                }
                let _ = self
                    .evt_tx
                    .send(NetEvent::Synced {
                        server_version: version,
                        client_count: count,
                    })
                    .await;
            }
            MessageType::ServerSettings => {
                if let Some(settings) = &msg.server_settings {
                    self.decode_server_settings(settings);
                }
                if let Some(v) = msg.version {
                    self.shared
                        .server_settings
                        .write()
                        .expect("server settings lock")
                        .server_version = Some(v);
                }
                let eam_enabled = self
                    .shared
                    .server_settings
                    .read()
                    .expect("server settings lock")
                    .get_bool(ServerSettingsKey::EXTERNAL_AWACS_MODE);
                let _ = self.evt_tx.send(NetEvent::ServerSettingsUpdated).await;
                if !eam_enabled {
                    let _ = self.evt_tx.send(NetEvent::EamDisconnected).await;
                }
            }
            MessageType::ClientDisconnect => {
                if let Some(client) = msg.client {
                    let removed = self
                        .shared
                        .clients
                        .write()
                        .expect("clients lock")
                        .remove(&client.client_guid)
                        .is_some();
                    if removed {
                        let _ = self
                            .evt_tx
                            .send(NetEvent::ClientRemoved(client.client_guid))
                            .await;
                    }
                }
            }
            MessageType::VersionMismatch => {
                let server_version = msg.version.unwrap_or_else(|| "unknown".into());
                log::error!(
                    "Version Mismatch Between Client ({}) & Server ({server_version}) - Disconnecting",
                    self.config.client_version
                );
                return Some(DisconnectReason::VersionMismatch { server_version });
            }
            MessageType::ExternalAwacsModePassword => {
                let coalition = msg.client.map(|c| c.coalition).unwrap_or(0);
                if coalition == 0 {
                    log::info!("External AWACS mode authentication failed");
                }
                let _ = self.evt_tx.send(NetEvent::EamResult { coalition }).await;
            }
            other => log::error!("Received unknown message type {other:?}"),
        }
        None
    }

    fn decode_server_settings(&self, settings: &HashMap<String, String>) {
        self.shared
            .server_settings
            .write()
            .expect("server settings lock")
            .decode(settings);
    }
}
