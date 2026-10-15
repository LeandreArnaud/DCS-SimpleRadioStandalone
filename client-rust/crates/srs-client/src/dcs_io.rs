//! Local UDP channels with DCS (and tools) — ports of `DCSRadioSyncHandler`
//! (listener part), `DCSGameGuiHandler`, `DCSLineOfSightHandler`,
//! `DCSAutoConnectHandler` and `UDPCommandHandler`.
//!
//! All listeners bind `0.0.0.0:<port>` and retry every 500 ms while the port
//! is busy (another SRS client). Payloads are UTF-8 JSON (or plain text for
//! auto-connect). Decoded messages are delivered as [`DcsEvent`]s on a std
//! channel; the client applies them on its own thread.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use srs_dcs::DcsPlayerRadioInfo;
use srs_net::SharedState;
use srs_protocol::message::lenient;
use srs_protocol::{LatLng, ServerSettingsKey};
use tokio::net::UdpSocket;
use tokio::sync::watch;

use crate::radio_state::SharedRadioState;

/// Ports from `global.cfg` `[Network Settings]`.
#[derive(Debug, Clone)]
pub struct DcsPorts {
    pub export_in: u16,      // DCSIncomingUDP 9084
    pub gamegui_in: u16,     // DCSIncomingGameGUIUDP 5068
    pub los_in: u16,         // DCSLOSIncomingUDP 9085
    pub los_out: u16,        // DCSLOSOutgoingUDP 9086
    pub autoconnect_in: u16, // DCSAutoConnectUDP 5069
    pub command_in: u16,     // CommandListenerUDP 9040
    pub info_out: u16,       // OutgoingDCSUDPInfo 7080
    pub other_out: u16,      // OutgoingDCSUDPOther 7082
}

impl Default for DcsPorts {
    fn default() -> Self {
        Self {
            export_in: 9084,
            gamegui_in: 5068,
            los_in: 9085,
            los_out: 9086,
            autoconnect_in: 5069,
            command_in: 9040,
            info_out: 7080,
            other_out: 7082,
        }
    }
}

/// `DCSPlayerSideInfo` (GameGUI export).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct GameGuiInfo {
    pub name: String,
    #[serde(deserialize_with = "lenient::i32")]
    pub seat: i32,
    #[serde(deserialize_with = "lenient::i32")]
    pub side: i32,
}

/// `DCSLosCheckResult`.
#[derive(Debug, Clone, Deserialize)]
pub struct LosResult {
    pub id: String,
    #[serde(deserialize_with = "lenient::f32")]
    pub los: f32,
}

/// `DCSLosCheckRequest`.
#[derive(Debug, Clone, Serialize)]
pub struct LosRequest {
    pub id: String,
    pub lat: f64,
    pub lng: f64,
    pub alt: f64,
}

/// `UDPInterfaceCommand.UDPCommandType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum UdpCommandType {
    FrequencyDelta = 0,
    ActiveRadio = 1,
    ToggleGuard = 2,
    ChannelUp = 3,
    ChannelDown = 4,
    SetVolume = 5,
    TransponderPower = 6,
    TransponderM1Code = 7,
    TransponderM3Code = 8,
    TransponderM4 = 9,
    TransponderIdent = 10,
    Guard = 11,
    FrequencySet = 12,
    TransponderM2Code = 13,
    ToggleRadioRetransmit = 14,
    RadioRetransmit = 15,
    ToggleGlobalSimultaneousTransmission = 16,
    GlobalSimultaneousTransmission = 17,
    ToggleSimultaneousRadio = 18,
    SimultaneousRadio = 19,
    IntercomChannel = 20,
}

impl UdpCommandType {
    pub const fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::FrequencyDelta,
            1 => Self::ActiveRadio,
            2 => Self::ToggleGuard,
            3 => Self::ChannelUp,
            4 => Self::ChannelDown,
            5 => Self::SetVolume,
            6 => Self::TransponderPower,
            7 => Self::TransponderM1Code,
            8 => Self::TransponderM3Code,
            9 => Self::TransponderM4,
            10 => Self::TransponderIdent,
            11 => Self::Guard,
            12 => Self::FrequencySet,
            13 => Self::TransponderM2Code,
            14 => Self::ToggleRadioRetransmit,
            15 => Self::RadioRetransmit,
            16 => Self::ToggleGlobalSimultaneousTransmission,
            17 => Self::GlobalSimultaneousTransmission,
            18 => Self::ToggleSimultaneousRadio,
            19 => Self::SimultaneousRadio,
            20 => Self::IntercomChannel,
            _ => return None,
        })
    }
}

/// `UDPInterfaceCommand` (keys matched case-insensitively).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UdpCommand {
    #[serde(rename = "radioid", deserialize_with = "lenient::i32")]
    pub radio_id: i32,
    #[serde(deserialize_with = "lenient::f64")]
    pub frequency: f64,
    #[serde(deserialize_with = "lenient::i32")]
    pub command: i32,
    #[serde(deserialize_with = "lenient::f32")]
    pub volume: f32,
    #[serde(deserialize_with = "lenient::bool")]
    pub enabled: bool,
    #[serde(deserialize_with = "lenient::i32")]
    pub code: i32,
}

impl UdpCommand {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        srs_protocol::server_settings::from_json_case_insensitive(json)
    }

    pub fn kind(&self) -> Option<UdpCommandType> {
        UdpCommandType::from_i32(self.command)
    }
}

#[derive(Debug, Clone)]
pub enum DcsEvent {
    /// Radio export (9084).
    Export(Box<DcsPlayerRadioInfo>),
    /// GameGUI side info (5068).
    GameGui(GameGuiInfo),
    /// LOS results (9085).
    LosResults(Vec<LosResult>),
    /// Auto-connect request (5069): `host:port`.
    AutoConnect(String),
    /// External command (9040).
    Command(UdpCommand),
}

/// Handle to the running DCS tasks. Dropping it (or calling `stop`) ends them.
pub struct DcsIo {
    stop_tx: watch::Sender<bool>,
    out_tx: mpsc::Sender<Vec<u8>>,
}

impl DcsIo {
    pub fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }

    /// Send a `CombinedRadioState` line to the in-game overlay (7080) and
    /// external panels (7082).
    pub fn send_state(&self, json_line: Vec<u8>) {
        let _ = self.out_tx.send(json_line);
    }
}

impl Drop for DcsIo {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Spawn every DCS task on the given runtime.
pub fn spawn(
    rt: &tokio::runtime::Handle,
    ports: DcsPorts,
    events: mpsc::Sender<DcsEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
    shared: Arc<SharedState>,
    radio_state: SharedRadioState,
    my_guid: String,
) -> DcsIo {
    let (stop_tx, stop_rx) = watch::channel(false);
    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();

    // Listeners.
    let mk = |port: u16, name: &'static str| (port, name, stop_rx.clone(), events.clone(), wake.clone());
    {
        let (port, name, stop, ev, wake) = mk(ports.export_in, "DCS Export");
        rt.spawn(listen(
            port,
            name,
            stop,
            move |text| {
                serde_json::from_str::<DcsPlayerRadioInfo>(text.trim())
                    .ok()
                    .map(|m| DcsEvent::Export(Box::new(m)))
            },
            ev,
            wake,
        ));
    }
    {
        let (port, name, stop, ev, wake) = mk(ports.gamegui_in, "DCS GameGUI");
        rt.spawn(listen(
            port,
            name,
            stop,
            |text| {
                serde_json::from_str::<GameGuiInfo>(text.trim())
                    .ok()
                    .map(DcsEvent::GameGui)
            },
            ev,
            wake,
        ));
    }
    {
        let (port, name, stop, ev, wake) = mk(ports.los_in, "DCS LOS");
        rt.spawn(listen(
            port,
            name,
            stop,
            |text| {
                serde_json::from_str::<Vec<LosResult>>(text.trim())
                    .ok()
                    .map(DcsEvent::LosResults)
            },
            ev,
            wake,
        ));
    }
    {
        let (port, name, stop, ev, wake) = mk(ports.autoconnect_in, "DCS AutoConnect");
        rt.spawn(listen(
            port,
            name,
            stop,
            |text| Some(DcsEvent::AutoConnect(parse_autoconnect(text))),
            ev,
            wake,
        ));
    }
    {
        let (port, name, stop, ev, wake) = mk(ports.command_in, "UDP Command");
        rt.spawn(listen(
            port,
            name,
            stop,
            |text| UdpCommand::parse(text.trim()).ok().map(DcsEvent::Command),
            ev,
            wake,
        ));
    }

    // Outgoing state (7080 / 7082).
    {
        let stop = stop_rx.clone();
        let (info, other) = (ports.info_out, ports.other_out);
        rt.spawn(async move {
            let Ok(sock) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else {
                log::error!("Cannot create DCS state sender socket");
                return;
            };
            let a: SocketAddr = (Ipv4Addr::LOCALHOST, info).into();
            let b: SocketAddr = (Ipv4Addr::LOCALHOST, other).into();
            loop {
                if *stop.borrow() {
                    break;
                }
                // Blocking recv on a std channel from an async task: use a short
                // timeout so the stop flag is observed.
                match out_rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(bytes) => {
                        let _ = sock.send_to(&bytes, a).await;
                        let _ = sock.send_to(&bytes, b).await;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
    }

    // LOS requests (9086), every 250 ms, chunks of 10.
    {
        let stop = stop_rx.clone();
        let port = ports.los_out;
        rt.spawn(async move {
            let Ok(sock) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else {
                return;
            };
            let host: SocketAddr = (Ipv4Addr::LOCALHOST, port).into();
            while !*stop.borrow() {
                let requests = los_requests(&shared, &radio_state, &my_guid);
                for chunk in requests.chunks(10) {
                    let mut line = serde_json::to_vec(chunk).unwrap_or_default();
                    line.push(b'\n');
                    let _ = sock.send_to(&line, host).await;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        });
    }

    DcsIo { stop_tx, out_tx }
}

/// `GenerateDcsLosCheckRequests`.
fn los_requests(shared: &SharedState, radio_state: &SharedRadioState, my_guid: &str) -> Vec<LosRequest> {
    let los_enabled = shared
        .server_settings
        .read()
        .map(|s| s.get_bool(ServerSettingsKey::LOS_ENABLED))
        .unwrap_or(false);
    if !los_enabled {
        return Vec::new();
    }
    let my_pos: LatLng = radio_state.lock().side.position;
    if !my_pos.is_valid() {
        return Vec::new();
    }
    shared
        .snapshot()
        .into_iter()
        .filter(|c| c.client.client_guid != my_guid)
        .filter_map(|c| {
            let p = c.client.lat_lng_position?;
            if !p.is_valid() {
                return None;
            }
            Some(LosRequest {
                id: c.client.client_guid,
                lat: p.lat,
                lng: p.lng,
                alt: p.alt,
            })
        })
        .collect()
}

/// `DCSAutoConnectHandler.HandleMessageAsync`: `host[:port]` → `host:port`.
pub fn parse_autoconnect(text: &str) -> String {
    let t = text.trim();
    match t.split_once(':') {
        Some((h, p)) => format!("{}:{}", h.trim(), p.trim()),
        None => format!("{t}:5002"),
    }
}

async fn listen<F>(
    port: u16,
    name: &'static str,
    mut stop: watch::Receiver<bool>,
    parse: F,
    events: mpsc::Sender<DcsEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
) where
    F: Fn(&str) -> Option<DcsEvent> + Send + 'static,
{
    let sock = loop {
        if *stop.borrow() {
            return;
        }
        match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).await {
            Ok(s) => {
                log::info!("{name} listener started on UDP {port}");
                break s;
            }
            Err(e) => {
                log::warn!("Unable to bind to the {name} socket port {port}: {e}");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                    _ = stop.changed() => return,
                }
            }
        }
    };
    let mut buf = vec![0u8; 65_536];
    loop {
        tokio::select! {
            res = sock.recv_from(&mut buf) => {
                match res {
                    Ok((n, _)) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        match parse(&text) {
                            Some(evt) => {
                                if events.send(evt).is_err() {
                                    return;
                                }
                                wake();
                            }
                            None => log::error!("Exception handling {name} message: {:.200}", text),
                        }
                    }
                    Err(e) => {
                        log::error!("SocketException handling {name} message: {e}");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
            _ = stop.changed() => {
                if *stop.borrow() { break; }
            }
        }
    }
    log::info!("{name} listener stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autoconnect_parsing() {
        assert_eq!(parse_autoconnect("1.2.3.4"), "1.2.3.4:5002");
        assert_eq!(
            parse_autoconnect(" srs.example.org : 5010 \n"),
            "srs.example.org:5010"
        );
    }

    #[test]
    fn command_parsing_is_case_insensitive() {
        let c = UdpCommand::parse(r#"{"Command":12,"RadioId":2,"Frequency":251.5}"#).unwrap();
        assert_eq!(c.kind(), Some(UdpCommandType::FrequencySet));
        assert_eq!(c.radio_id, 2);
        assert_eq!(c.frequency, 251.5);
        let c = UdpCommand::parse(r#"{"command":5,"radioId":1,"volume":0.5}"#).unwrap();
        assert_eq!(c.kind(), Some(UdpCommandType::SetVolume));
        assert_eq!(c.volume, 0.5);
    }

    #[test]
    fn gamegui_parsing() {
        let g: GameGuiInfo = serde_json::from_str(r#"{"name":"Mav","side":2.0,"seat":1}"#).unwrap();
        assert_eq!(g.side, 2);
        assert_eq!(g.seat, 1);
    }
}
