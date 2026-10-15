//! Mutable radio state shared between the UI, the DCS/EAM feeders, the
//! decode thread and the encoder (`ClientStateSingleton` in C#).

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use srs_dcs::DcsPlayerRadioInfo;
use srs_protocol::constants::MAX_RADIOS;
use srs_protocol::LatLng;

/// `DCSPlayerSideInfo` (GameGUI export, port 5068) or EAM coalition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SideInfo {
    pub name: String,
    /// 0 front / normal, 1 back seat.
    pub seat: i32,
    /// 0 spectator, 1 red, 2 blue.
    pub side: i32,
    pub position: LatLng,
}

/// `RadioReceivingState`.
#[derive(Debug, Clone, Default)]
pub struct ReceivingState {
    pub last_received_at: Option<Instant>,
    pub is_secondary: bool,
    pub is_simultaneous: bool,
    pub received_on: usize,
    pub sent_by: String,
}

impl ReceivingState {
    /// Audio arrived in the last 350 ms.
    pub fn is_receiving(&self) -> bool {
        self.last_received_at
            .is_some_and(|t| t.elapsed() < Duration::from_millis(350))
    }
}

/// `RadioSendingState`.
#[derive(Debug, Clone, Default)]
pub struct SendingState {
    pub last_sent_at: Option<Instant>,
    pub is_sending: bool,
    pub sending_on: Option<usize>,
}

#[derive(Debug, Default)]
pub struct RadioState {
    pub dcs: DcsPlayerRadioInfo,
    pub side: SideInfo,
    pub receiving: Vec<ReceivingState>,
    pub sending: SendingState,
    /// External AWACS mode active (DCS export ignored).
    pub eam_active: bool,
    /// `IntercomOffset`: EAM intercom group (unitId = 100000000 + offset).
    pub intercom_offset: u32,
    /// `LastSent = 0`: a manual change must be re-sent with the next export.
    pub force_sync: bool,
    /// `DcsExportLastReceived` / `DcsGameGuiLastReceived`.
    pub dcs_export_last_received: Option<Instant>,
    pub dcs_gamegui_last_received: Option<Instant>,
}

impl RadioState {
    pub fn new() -> Self {
        Self {
            dcs: DcsPlayerRadioInfo::default(),
            side: SideInfo::default(),
            receiving: (0..MAX_RADIOS).map(|_| ReceivingState::default()).collect(),
            sending: SendingState::default(),
            eam_active: false,
            intercom_offset: 1,
            force_sync: false,
            dcs_export_last_received: None,
            dcs_gamegui_last_received: None,
        }
    }

    /// `IsGameConnected`: GameGUI and export both seen in the last 10 s.
    pub fn is_game_connected(&self) -> bool {
        let fresh = |t: Option<Instant>| t.is_some_and(|t| t.elapsed() < Duration::from_secs(10));
        fresh(self.dcs_gamegui_last_received) && fresh(self.dcs_export_last_received)
    }

    /// Radios that are currently playing audio (for the UI).
    pub fn receiving_radios(&self) -> Vec<usize> {
        self.receiving
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_receiving())
            .map(|(i, _)| i)
            .collect()
    }
}

pub type SharedRadioState = Arc<Mutex<RadioState>>;

pub fn shared_radio_state() -> SharedRadioState {
    Arc::new(Mutex::new(RadioState::new()))
}
