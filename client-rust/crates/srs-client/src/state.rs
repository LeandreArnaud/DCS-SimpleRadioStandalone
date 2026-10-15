//! Local client state (`ClientStateSingleton` in C#): identity, last known
//! name, DCS radio data and coalition metadata.

use srs_protocol::{LatLng, RadioInfo, ShortGuid, SrClient};

#[derive(Debug, Clone)]
pub struct ClientState {
    /// New on every launch, never persisted.
    pub guid: ShortGuid,
    /// `LastSeenName` setting, overwritten by the DCS export `name`.
    pub last_seen_name: String,
    /// Radios as seen by other clients (from DCS or EAM synthesis).
    pub radio_info: RadioInfo,
    /// 0 spectator, 1 red, 2 blue (from the GameGUI export or EAM).
    pub coalition: i32,
    pub seat: i32,
    pub position: LatLng,
    pub allow_record: bool,
    pub dis_entity_id: i32,
}

impl ClientState {
    pub fn new(last_seen_name: String) -> Self {
        Self {
            guid: ShortGuid::new_random(),
            last_seen_name,
            radio_info: RadioInfo::default(),
            coalition: 0,
            seat: 0,
            position: LatLng::default(),
            allow_record: false,
            dis_entity_id: -1,
        }
    }

    /// The client record sent in the SYNC request (`MainWindowViewModel.ConnectAsync`).
    pub fn initial_client(&self) -> SrClient {
        let mut c = SrClient {
            client_guid: self.guid.to_string(),
            coalition: 0,
            seat: 0,
            allow_record: self.allow_record,
            radio_info: Some(self.radio_info.clone()),
            lat_lng_position: Some(LatLng::default()),
            dis_entity_id: self.dis_entity_id,
            ..Default::default()
        };
        c.set_name(&self.last_seen_name);
        c
    }

    /// Full record for RADIO_UPDATE / UPDATE (`DCSRadioSyncHandler.ProcessRadioInfoAsync`).
    pub fn current_client(&self) -> SrClient {
        let mut c = SrClient {
            client_guid: self.guid.to_string(),
            coalition: self.coalition,
            seat: self.seat,
            allow_record: self.allow_record,
            radio_info: Some(self.radio_info.clone()),
            lat_lng_position: Some(self.position),
            dis_entity_id: self.dis_entity_id,
            ..Default::default()
        };
        c.set_name(&self.last_seen_name);
        c
    }
}
