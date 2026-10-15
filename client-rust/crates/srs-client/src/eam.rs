//! External AWACS Mode radios: port of `StartExternalAWACSModeLoop`,
//! `processServerCustomEAMRadio` and `processClientCustomEAMRadio`.
//!
//! Radio sources, in order: server preset (`SERVER_EAM_RADIO_PRESET`, if the
//! profile allows it), `awacs-radios-custom.json`, `awacs-radios.json`
//! (both in the presets folder, i.e. the working directory by default), then
//! eleven "No Radio" placeholders.

use std::path::Path;

use srs_dcs::{
    DcsPlayerRadioInfo, DcsRadio, SimultaneousTransmissionControl, SwitchControls, UNIT_ID_OFFSET,
};
use srs_protocol::constants::MAX_RADIOS;
use srs_protocol::server_settings::{from_json_case_insensitive, DcsRadioCustom};
use srs_protocol::{LatLng, Modulation};

pub const AWACS_RADIOS_FILE: &str = "awacs-radios.json";
pub const AWACS_RADIOS_CUSTOM_FILE: &str = "awacs-radios-custom.json";

/// `processClientCustomEAMRadio`: `None` when the file is missing or invalid.
pub fn load_client_radios(presets_folder: &Path, file: &str) -> Option<Vec<DcsRadio>> {
    let path = presets_folder.join(file);
    let text = std::fs::read_to_string(&path).ok()?;
    match from_json_case_insensitive::<Vec<DcsRadio>>(&text) {
        Ok(mut radios) => {
            for r in &mut radios {
                r.apply_mids_override();
            }
            Some(radios)
        }
        Err(e) => {
            log::warn!("Failed to load {} radio file: {e}", path.display());
            None
        }
    }
}

/// `processServerCustomEAMRadio`: exactly 11 radios or `None`.
pub fn radios_from_server_preset(custom: &[DcsRadioCustom]) -> Option<Vec<DcsRadio>> {
    if custom.len() != MAX_RADIOS {
        return None;
    }
    Some(
        custom
            .iter()
            .map(|c| {
                let mut r = DcsRadio {
                    freq: c.freq,
                    freq_min: c.freq_min,
                    freq_max: c.freq_max,
                    sec_freq: c.sec_freq,
                    modulation: c.modulation,
                    name: c.name.clone(),
                    freq_mode: srs_dcs::FreqMode::from_i32(c.freq_mode),
                    enc_mode: srs_dcs::EncryptionMode::from_i32(c.enc_mode),
                    vol_mode: srs_dcs::VolumeMode::from_i32(c.vol_mode),
                    expansion: c.expansion,
                    channel: c.channel,
                    enc: c.enc,
                    guard_freq_mode: srs_dcs::FreqMode::from_i32(c.guard_freq_mode),
                    model: c.model.clone(),
                    volume: 1.0,
                    rt_mode: srs_dcs::RetransmitMode::from_i32(c.rt_mode),
                    rx_only: c.rx_only,
                    simul: c.simul,
                    enc_key: c.enc_key,
                    ..DcsRadio::no_radio()
                };
                r.apply_mids_override();
                r
            })
            .collect(),
    )
}

/// Eleven "No Radio" placeholders.
pub fn fallback_radios() -> Vec<DcsRadio> {
    (0..MAX_RADIOS).map(|_| DcsRadio::no_radio()).collect()
}

/// Resolve the EAM radio set using the C# precedence.
pub fn resolve_radios(
    allow_server_preset: bool,
    server_preset: &[DcsRadioCustom],
    presets_folder: &Path,
) -> Vec<DcsRadio> {
    let mut radios = None;
    if allow_server_preset {
        radios = radios_from_server_preset(server_preset);
    }
    if radios.is_none() {
        radios = load_client_radios(presets_folder, AWACS_RADIOS_CUSTOM_FILE);
    }
    if radios.is_none() {
        radios = load_client_radios(presets_folder, AWACS_RADIOS_FILE);
    }
    radios.unwrap_or_else(|| {
        log::warn!("Failed to load AWACS radio file from server, default or custom one");
        fallback_radios()
    })
}

/// The synthetic `DCSPlayerRadioInfo` the C# loop feeds every 200 ms.
pub fn synthetic_info(name: &str, radios: Vec<DcsRadio>, intercom_offset: u32) -> DcsPlayerRadioInfo {
    let mut radios = radios;
    radios.resize_with(MAX_RADIOS, DcsRadio::no_radio);
    DcsPlayerRadioInfo {
        control: SwitchControls::Hotas,
        name: name.to_string(),
        ptt: false,
        radios,
        selected: 1,
        lat_lng: LatLng::default(),
        simultaneous_transmission: false,
        simultaneous_transmission_control: SimultaneousTransmissionControl::EnabledInternalSrsControls,
        unit: "EAM".to_string(),
        unit_id: UNIT_ID_OFFSET + intercom_offset,
        in_aircraft: false,
        ..DcsPlayerRadioInfo::default()
    }
}

/// Human readable summary for logs / UI.
pub fn describe(radios: &[DcsRadio]) -> String {
    radios
        .iter()
        .enumerate()
        .filter(|(_, r)| r.modulation != Modulation::Disabled)
        .map(|(i, r)| format!("{i}:{} {:.3} {}", r.name, r.freq / 1e6, r.modulation.name()))
        .collect::<Vec<_>>()
        .join(", ")
}
