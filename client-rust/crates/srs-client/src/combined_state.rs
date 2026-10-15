//! `CombinedRadioState`: the JSON pushed to the in-game overlay (7080) and
//! external panels (7082) after every DCS export message.
//!
//! The C# `[JsonDCSIgnoreSerialization]` filter is broken (it tests the
//! attribute on the property *type*), so every field of
//! `DCSPlayerRadioInfo` is emitted; this port does the same.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use srs_dcs::DcsPlayerRadioInfo;
use srs_net::SharedState;
use srs_protocol::constants::MAX_RADIOS;
use srs_protocol::{can_hear_transmission, Modulation, ServerSettingsKey};

use crate::radio_state::RadioState;

#[derive(Debug, Serialize)]
pub struct RadioSendingStateJson {
    #[serde(rename = "IsSending")]
    pub is_sending: bool,
    #[serde(rename = "SendingOn")]
    pub sending_on: i32,
    #[serde(rename = "IsEncrypted")]
    pub is_encrypted: i32,
}

#[derive(Debug, Serialize)]
pub struct RadioReceivingStateJson {
    /// .NET ticks (100 ns since 0001-01-01), like `DateTime.Now.Ticks`.
    #[serde(rename = "LastReceivedAt")]
    pub last_received_at: i64,
    #[serde(rename = "IsSecondary")]
    pub is_secondary: bool,
    #[serde(rename = "IsSimultaneous")]
    pub is_simultaneous: bool,
    #[serde(rename = "ReceivedOn")]
    pub received_on: i32,
    #[serde(rename = "PlayedEndOfTransmission")]
    pub played_end_of_transmission: bool,
    #[serde(rename = "SentBy")]
    pub sent_by: Option<String>,
    #[serde(rename = "IsReceiving")]
    pub is_receiving: bool,
}

#[derive(Debug, Serialize)]
pub struct CombinedRadioState<'a> {
    #[serde(rename = "RadioInfo")]
    pub radio_info: &'a DcsPlayerRadioInfo,
    #[serde(rename = "RadioSendingState")]
    pub radio_sending_state: RadioSendingStateJson,
    #[serde(rename = "RadioReceivingState")]
    pub radio_receiving_state: Vec<Option<RadioReceivingStateJson>>,
    #[serde(rename = "ClientCountConnected")]
    pub client_count_connected: usize,
    #[serde(rename = "TunedClients")]
    pub tuned_clients: Vec<i32>,
}

/// Unix epoch in .NET ticks.
const DOTNET_EPOCH_TICKS: i64 = 621_355_968_000_000_000;

fn instant_to_dotnet_ticks(t: Instant) -> i64 {
    let age = t.elapsed();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let then = now.saturating_sub(age);
    DOTNET_EPOCH_TICKS + (then.as_nanos() / 100) as i64
}

/// `ClientStateSingleton.ClientsOnFreq`: other clients that could hear a
/// transmission on `freq`/`modulation` from us.
pub fn clients_on_freq(
    shared: &SharedState,
    rs: &RadioState,
    my_guid: &str,
    freq: f64,
    modulation: Modulation,
) -> i32 {
    let (show, security, globals) = {
        let s = shared.server_settings.read().expect("server settings lock");
        (
            s.get_bool(ServerSettingsKey::SHOW_TUNED_COUNT),
            s.get_bool(ServerSettingsKey::COALITION_AUDIO_SECURITY),
            s.global_frequencies_hz.clone(),
        )
    };
    if !show {
        return 0;
    }
    let global = globals.contains(&freq);
    let my_unit = rs.dcs.unit_id;
    let my_side = rs.side.side;
    let mut count = 0;
    for c in shared.snapshot() {
        if c.client.client_guid == my_guid {
            continue;
        }
        if !(global || !security || c.client.coalition == my_side) {
            continue;
        }
        let Some(ri) = &c.client.radio_info else { continue };
        if can_hear_transmission(&ri.radios, ri.unit_id, freq, modulation, 0, false, my_unit, &[]).is_some() {
            count += 1;
        }
    }
    count
}

/// Build the JSON line (with trailing `\n`) for the current state.
pub fn build_line(shared: &SharedState, rs: &RadioState, my_guid: &str, connected: bool) -> Vec<u8> {
    let mut tuned = vec![0i32; MAX_RADIOS];
    if connected && rs.dcs.is_current() {
        for (i, r) in rs.dcs.radios.iter().enumerate().take(MAX_RADIOS) {
            if r.modulation != Modulation::Disabled {
                tuned[i] = clients_on_freq(shared, rs, my_guid, r.freq, r.modulation);
            }
        }
    }
    let receiving = rs
        .receiving
        .iter()
        .map(|r| {
            r.last_received_at.map(|t| RadioReceivingStateJson {
                last_received_at: instant_to_dotnet_ticks(t),
                is_secondary: r.is_secondary,
                is_simultaneous: r.is_simultaneous,
                received_on: r.received_on as i32,
                played_end_of_transmission: false,
                sent_by: Some(r.sent_by.clone()),
                is_receiving: r.is_receiving(),
            })
        })
        .collect();
    let state = CombinedRadioState {
        radio_info: &rs.dcs,
        radio_sending_state: RadioSendingStateJson {
            is_sending: rs.sending.is_sending,
            sending_on: rs.sending.sending_on.map(|i| i as i32).unwrap_or(0),
            is_encrypted: 0,
        },
        radio_receiving_state: receiving,
        client_count_connected: shared.client_count(),
        tuned_clients: tuned,
    };
    let mut v = serde_json::to_vec(&state).unwrap_or_default();
    v.push(b'\n');
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_has_csharp_shape() {
        let shared = SharedState::new();
        let rs = RadioState::new();
        let line = build_line(&shared, &rs, "me", false);
        let v: serde_json::Value = serde_json::from_slice(&line[..line.len() - 1]).unwrap();
        for k in [
            "RadioInfo",
            "RadioSendingState",
            "RadioReceivingState",
            "ClientCountConnected",
            "TunedClients",
        ] {
            assert!(v.get(k).is_some(), "{k}");
        }
        assert_eq!(v["TunedClients"].as_array().unwrap().len(), 11);
        assert_eq!(v["RadioReceivingState"].as_array().unwrap().len(), 11);
        let ri = &v["RadioInfo"];
        for k in [
            "radios", "selected", "unit", "unitId", "iff", "ambient", "ptt", "name", "latLng", "control",
        ] {
            assert!(ri.get(k).is_some(), "{k}");
        }
        assert_eq!(ri["radios"].as_array().unwrap().len(), 11);
        assert!(ri["radios"][0].get("volume").is_some());
        assert!(line.ends_with(b"\n"));
    }
}
