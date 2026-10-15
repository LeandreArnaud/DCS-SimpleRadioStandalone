//! UDP voice transport: port of `Common/Network/Client/UDPVoiceHandler.cs`.
//!
//! - One connected UDP socket to the server (same host:port as TCP).
//! - A "ping" is the bare 22-byte client GUID, sent immediately and then
//!   every 15 s. The server answers with 22 bytes; the first answer sets
//!   `Ready`. Voice datagrams (> 22 bytes) are only accepted once ready.
//! - Any datagram resets a 42 s watchdog; when it fires the socket is
//!   recreated (new ephemeral port) and `Ready` drops to `false`.
//! - Outgoing packets are only sent while ready.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use srs_protocol::short_guid::SHORT_GUID_LEN;
use srs_protocol::ShortGuid;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

pub const PING_INTERVAL: Duration = Duration::from_secs(15);
pub const VOIP_TIMEOUT: Duration = Duration::from_secs(42);
/// Largest datagram we accept (the C# `UdpClient` default is 64 KiB).
const MAX_DATAGRAM: usize = 65_536;

#[derive(Debug, Clone)]
pub struct UdpConfig {
    pub ping_interval: Duration,
    pub timeout: Duration,
}

impl Default for UdpConfig {
    fn default() -> Self {
        Self {
            ping_interval: PING_INTERVAL,
            timeout: VOIP_TIMEOUT,
        }
    }
}

/// Events from the voice transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoipEvent {
    /// `Ready` changed (`VOIPStatusMessage`).
    Ready(bool),
    /// 42 s without any datagram: socket recreated.
    Timeout,
    /// Transport stopped.
    Stopped,
}

#[derive(Clone)]
pub struct UdpHandle {
    send_tx: mpsc::Sender<Vec<u8>>,
    stop_tx: mpsc::Sender<()>,
    ready: Arc<AtomicBool>,
}

impl UdpHandle {
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Queue an encoded voice packet. Dropped when not ready or the queue is
    /// full (the C# code also drops when not ready).
    pub fn send(&self, packet: Vec<u8>) -> bool {
        if !self.is_ready() {
            return false;
        }
        self.send_tx.try_send(packet).is_ok()
    }

    pub fn stop(&self) {
        let _ = self.stop_tx.try_send(());
    }
}

pub struct UdpVoice;

impl UdpVoice {
    /// Spawn the transport on the current tokio runtime. Received voice
    /// datagrams are delivered on the returned receiver (raw bytes, already
    /// filtered: only > 22 bytes and only while ready).
    pub fn spawn(
        server: SocketAddr,
        guid: ShortGuid,
        config: UdpConfig,
    ) -> (UdpHandle, mpsc::Receiver<Vec<u8>>, mpsc::Receiver<VoipEvent>) {
        let (send_tx, send_rx) = mpsc::channel::<Vec<u8>>(256);
        let (stop_tx, stop_rx) = mpsc::channel::<()>(1);
        let (voice_tx, voice_rx) = mpsc::channel::<Vec<u8>>(512);
        let (evt_tx, evt_rx) = mpsc::channel::<VoipEvent>(32);
        let ready = Arc::new(AtomicBool::new(false));
        tokio::spawn(run(
            server,
            guid,
            config,
            send_rx,
            stop_rx,
            voice_tx,
            evt_tx,
            ready.clone(),
        ));
        (
            UdpHandle {
                send_tx,
                stop_tx,
                ready,
            },
            voice_rx,
            evt_rx,
        )
    }
}

async fn setup_socket(server: SocketAddr) -> std::io::Result<UdpSocket> {
    let bind: SocketAddr = if server.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };
    let sock = UdpSocket::bind(bind).await?;
    sock.connect(server).await?;
    Ok(sock)
}

#[allow(clippy::too_many_arguments)]
async fn run(
    server: SocketAddr,
    guid: ShortGuid,
    config: UdpConfig,
    mut send_rx: mpsc::Receiver<Vec<u8>>,
    mut stop_rx: mpsc::Receiver<()>,
    voice_tx: mpsc::Sender<Vec<u8>>,
    evt_tx: mpsc::Sender<VoipEvent>,
    ready: Arc<AtomicBool>,
) {
    let set_ready = |value: bool, evt_tx: &mpsc::Sender<VoipEvent>| {
        if ready.swap(value, Ordering::Relaxed) != value {
            let _ = evt_tx.try_send(VoipEvent::Ready(value));
        }
    };

    let mut sock = match setup_socket(server).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Unable to create UDP voice socket: {e}");
            let _ = evt_tx.send(VoipEvent::Stopped).await;
            return;
        }
    };
    log::info!("Pinging Server - Starting");
    let mut buf = vec![0u8; MAX_DATAGRAM];
    let mut ping_tick = tokio::time::interval(config.ping_interval);
    ping_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let timeout = tokio::time::sleep(config.timeout);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = ping_tick.tick() => {
                if let Err(e) = sock.send(guid.as_bytes()).await {
                    log::warn!("Voice handler exception (ping): {e}");
                }
            }
            res = sock.recv(&mut buf) => {
                match res {
                    Ok(n) => {
                        if n == SHORT_GUID_LEN {
                            if !ready.load(Ordering::Relaxed) {
                                log::info!("Received initial Ping Back from Server");
                            }
                            set_ready(true, &evt_tx);
                        } else if n > SHORT_GUID_LEN && ready.load(Ordering::Relaxed) {
                            // Drop when the decoder cannot keep up rather than block the socket.
                            let _ = voice_tx.try_send(buf[..n].to_vec());
                        }
                        timeout.as_mut().reset(tokio::time::Instant::now() + config.timeout);
                    }
                    Err(e) => {
                        // ICMP port unreachable etc. The C# loop logs and carries on.
                        log::warn!("Voice handler exception (recv): {e}");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
            pkt = send_rx.recv() => {
                match pkt {
                    Some(p) => {
                        if ready.load(Ordering::Relaxed) {
                            if let Err(e) = sock.send(&p).await {
                                log::warn!("Voice handler exception (send): {e}");
                            }
                        }
                    }
                    None => break,
                }
            }
            _ = &mut timeout => {
                log::error!("VoIP Timeout - Recreating VoIP Connection");
                set_ready(false, &evt_tx);
                let _ = evt_tx.try_send(VoipEvent::Timeout);
                match setup_socket(server).await {
                    Ok(s) => sock = s,
                    Err(e) => log::error!("Unable to recreate UDP voice socket: {e}"),
                }
                ping_tick.reset_immediately();
                timeout.as_mut().reset(tokio::time::Instant::now() + config.timeout);
            }
            _ = stop_rx.recv() => break,
        }
    }
    set_ready(false, &evt_tx);
    let _ = evt_tx.send(VoipEvent::Stopped).await;
    log::info!("UDP Voice Handler Thread Stop");
}
