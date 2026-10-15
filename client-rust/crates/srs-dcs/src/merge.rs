//! Merge a DCS export message into the client's radio state: line-by-line
//! port of `DCSRadioSyncHandler.UpdateRadioAsync` (the biggest and most
//! quirk-laden function of the C# client). Everything that depends on
//! settings or other singletons comes in through [`MergeContext`]; side
//! effects the caller must perform come back as [`MergeEvent`]s.

use std::time::{Duration, Instant};

use srs_protocol::{LatLng, Modulation, Transponder};

use crate::model::{DcsPlayerRadioInfo, DcsRadio, EncryptionMode, FreqMode, RetransmitMode, SwitchControls};

/// Settings and external state the merge needs.
#[derive(Debug, Clone)]
pub struct MergeContext {
    /// Server `RADIO_EXPANSION` AND NOT profile `DisableExpansionRadios`.
    pub expansion_allowed: bool,
    /// Server `ALLOW_RADIO_ENCRYPTION`.
    pub allow_radio_encryption: bool,
    /// Profile `AlwaysAllowHotasControls`.
    pub always_allow_hotas_controls: bool,
    /// Profile `AllowDCSPTT`.
    pub allow_dcs_ptt: bool,
    /// Profile `AlwaysAllowTransponderOverlay`.
    pub always_allow_transponder_overlay: bool,
    /// Profile `AutoSelectPresetChannel`.
    pub auto_select_preset_channel: bool,
    /// Global `AutoSelectSettingsProfile`.
    pub auto_select_settings_profile: bool,
    /// `ClientStateSingleton.ExternalAWACSModeConnected`.
    pub eam_connected: bool,
    /// Global `LastPresetsFolder` (empty → working directory).
    pub presets_folder: std::path::PathBuf,
    /// Profile `ServerPresetSelection`.
    pub preset_source: crate::presets::PresetSource,
    /// Profile `InstructorMode` (UI only).
    pub instructor_mode: bool,
    /// `ShouldUseLotATCPosition()`: when true the DCS position is ignored.
    pub use_lotatc_position: bool,
    /// `RadioSendingState.IsSending` / `SendingOn` (for mic IDENT).
    pub sending_on: Option<usize>,
    pub now: Instant,
}

impl Default for MergeContext {
    fn default() -> Self {
        Self {
            expansion_allowed: false,
            allow_radio_encryption: true,
            always_allow_hotas_controls: false,
            allow_dcs_ptt: true,
            always_allow_transponder_overlay: false,
            auto_select_preset_channel: true,
            auto_select_settings_profile: false,
            eam_connected: false,
            presets_folder: std::path::PathBuf::from("."),
            preset_source: crate::presets::PresetSource::default(),
            instructor_mode: false,
            use_lotatc_position: false,
            sending_on: None,
            now: Instant::now(),
        }
    }
}

/// Side effects for the caller.
#[derive(Debug, Clone, PartialEq)]
pub enum MergeEvent {
    /// `NewUnitEnteredMessage` (profile auto-selection).
    NewUnitEntered { unit: String, seat: i32 },
    /// A new aircraft with an overlay-tuned radio: reload its preset
    /// channel list (`FixedChannels[i-1].Reload()`), then either select the
    /// first channel (`AutoSelectPresetChannel`) or, in EAM, the preselected
    /// one (1-based `channel` from the json).
    ReloadFixedChannels {
        radio: usize,
        min: f64,
        max: f64,
        preselected: i32,
    },
    /// A new aircraft with a cockpit-tuned radio: clear its channel list.
    ClearFixedChannels { radio: usize },
    /// Our position changed (sent with the next metadata update).
    PositionUpdated(LatLng),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeOutcome {
    /// `!beforeUpdate.Equals(playerRadioInfo)`: a RADIO_UPDATE is due.
    pub changed: bool,
    pub new_aircraft: bool,
    pub events: Vec<MergeEvent>,
}

/// Persistent bits of the handler (`_identStart`).
#[derive(Debug, Default, Clone)]
pub struct MergeMemory {
    pub ident_start: Option<Instant>,
}

fn disable_radio(r: &mut DcsRadio) {
    r.freq = 1.0;
    r.freq_min = 1.0;
    r.freq_max = 1.0;
    r.sec_freq = 0.0;
    r.retransmit = false;
    r.modulation = Modulation::Disabled;
    r.name = "No Radio".into();
    r.rt_mode = RetransmitMode::Disabled;
    r.freq_mode = FreqMode::Cockpit;
    r.guard_freq_mode = FreqMode::Cockpit;
    r.enc_mode = EncryptionMode::NoEncryption;
    r.vol_mode = crate::model::VolumeMode::Cockpit;
    r.rx_only = false;
}

/// Apply `message` onto `state`. `message` is taken by value because the C#
/// code mutates it too (`control`, `iff.control`).
pub fn merge(
    state: &mut DcsPlayerRadioInfo,
    mut message: DcsPlayerRadioInfo,
    ctx: &MergeContext,
    mem: &mut MergeMemory,
) -> MergeOutcome {
    let before = state.clone();
    let mut events = Vec::new();

    state.name = message.name.clone();
    state.in_aircraft = message.in_aircraft;
    state.intercom_hot_mic = message.intercom_hot_mic;
    state.capabilities = message.capabilities.clone();
    state.ambient = message.ambient.clone();

    if ctx.always_allow_hotas_controls {
        message.control = SwitchControls::Hotas;
        state.control = SwitchControls::Hotas;
    } else {
        state.control = message.control;
    }
    state.simultaneous_transmission_control = message.simultaneous_transmission_control;

    if !ctx.use_lotatc_position {
        state.lat_lng = message.lat_lng;
        events.push(MergeEvent::PositionUpdated(message.lat_lng));
    }

    let new_aircraft = state.unit_id != message.unit_id
        || state.seat != message.seat
        || state.unit != message.unit
        || !state.is_current();
    state.unit = message.unit.clone();
    let override_freq_and_vol = state.unit_id != message.unit_id;
    state.unit_id = message.unit_id;
    state.seat = message.seat;

    if new_aircraft {
        if ctx.auto_select_settings_profile {
            events.push(MergeEvent::NewUnitEntered {
                unit: message.unit.clone(),
                seat: message.seat,
            });
        }
        state.iff = message.iff.clone();
    }
    if override_freq_and_vol {
        state.selected = message.selected;
    }
    if state.control == SwitchControls::InCockpit {
        state.selected = message.selected;
    }

    let mut simul = false;
    for i in 0..state.radios.len() {
        let Some(update) = message.radios.get(i).cloned() else {
            disable_radio(&mut state.radios[i]);
            continue;
        };
        let client_radio = &mut state.radios[i];
        if (update.expansion && !ctx.expansion_allowed) || update.modulation == Modulation::Disabled {
            disable_radio(client_radio);
            client_radio.intercom_unit_id = 0;
            continue;
        }

        client_radio.freq_min = update.freq_min;
        client_radio.freq_max = update.freq_max;
        if state.simultaneous_transmission_control
            == crate::model::SimultaneousTransmissionControl::ExternalDcsControl
        {
            client_radio.simul = update.simul;
        }
        if update.simul {
            simul = true;
        }
        client_radio.name = update.name.clone();
        client_radio.model = update.model.clone();
        client_radio.modulation = update.modulation;
        if client_radio.modulation == Modulation::Intercom {
            client_radio.intercom_unit_id = update.intercom_unit_id;
        }
        client_radio.freq_mode = update.freq_mode;
        client_radio.guard_freq_mode = update.guard_freq_mode;
        client_radio.rt_mode = update.rt_mode;
        client_radio.rx_only = update.rx_only;
        client_radio.enc_mode = if ctx.allow_radio_encryption {
            update.enc_mode
        } else {
            EncryptionMode::NoEncryption
        };
        client_radio.vol_mode = update.vol_mode;

        if update.freq_mode == FreqMode::Cockpit || override_freq_and_vol {
            client_radio.freq = update.freq;
            if new_aircraft && update.guard_freq_mode == FreqMode::Overlay {
                client_radio.sec_freq = 0.0;
            } else if client_radio.sec_freq != 0.0 && update.guard_freq_mode == FreqMode::Overlay {
                client_radio.sec_freq = update.sec_freq;
            } else if client_radio.sec_freq == 0.0 && update.guard_freq_mode == FreqMode::Overlay {
                client_radio.sec_freq = 0.0;
            } else {
                client_radio.sec_freq = update.sec_freq;
            }
            client_radio.channel = update.channel;
        } else {
            if client_radio.sec_freq != 0.0 {
                client_radio.sec_freq = update.sec_freq;
            }
            if client_radio.freq > client_radio.freq_max {
                client_radio.freq = client_radio.freq_max;
            } else if client_radio.freq < client_radio.freq_min {
                client_radio.freq = client_radio.freq_min;
            }
        }

        if override_freq_and_vol {
            client_radio.enc = false;
            client_radio.enc_key = 0;
        }
        // Encryption: note the mix of `update.encMode` and `client.encMode`
        // in the C# conditions, reproduced as is.
        if update.enc_mode == EncryptionMode::EncryptionJustOverlay {
            if client_radio.enc_key == 0 {
                client_radio.enc_key = 1;
            }
        } else if client_radio.enc_mode == EncryptionMode::EncryptionCockpitToggleOverlayCode {
            client_radio.enc = update.enc;
            if client_radio.enc_key == 0 {
                client_radio.enc_key = 1;
            }
        } else if client_radio.enc_mode == EncryptionMode::EncryptionFull {
            client_radio.enc = update.enc;
            client_radio.enc_key = update.enc_key;
        } else {
            client_radio.enc = false;
            client_radio.enc_key = 0;
        }

        if update.vol_mode == crate::model::VolumeMode::Cockpit || override_freq_and_vol {
            client_radio.volume = update.volume;
        }

        if update.rt_mode == RetransmitMode::Cockpit {
            client_radio.rt_mode = update.rt_mode;
            client_radio.retransmit = update.retransmit;
        } else if update.rt_mode == RetransmitMode::Disabled {
            client_radio.rt_mode = update.rt_mode;
            client_radio.retransmit = false;
        }

        if new_aircraft && i > 0 {
            if client_radio.freq_mode == FreqMode::Overlay {
                let preselected = client_radio.channel;
                client_radio.channel = -1;
                events.push(MergeEvent::ReloadFixedChannels {
                    radio: i,
                    min: client_radio.freq_min,
                    max: client_radio.freq_max,
                    preselected,
                });
            } else {
                events.push(MergeEvent::ClearFixedChannels { radio: i });
            }
        }
    }

    if state.simultaneous_transmission_control
        == crate::model::SimultaneousTransmissionControl::ExternalDcsControl
    {
        state.simultaneous_transmission = simul;
    }

    state.ptt = if ctx.allow_dcs_ptt { message.ptt } else { false };

    // IFF / transponder.
    if ctx.always_allow_transponder_overlay && message.iff.control != Transponder::CONTROL_DISABLED {
        state.iff.control = Transponder::CONTROL_OVERLAY;
        message.iff.control = Transponder::CONTROL_OVERLAY;
    }
    if message.iff.control == Transponder::CONTROL_COCKPIT {
        state.iff = message.iff.clone();
    }
    // Mic IDENT: transmitting on the radio wired to the transponder.
    if !state.ptt && state.iff.mic > 0 {
        if let Some(on) = ctx.sending_on {
            if on as i32 == state.iff.mic {
                state.iff.status = Transponder::STATUS_IDENT;
            }
        }
    }
    if state.iff.status == Transponder::STATUS_IDENT {
        let start = *mem.ident_start.get_or_insert(ctx.now);
        if ctx.now.duration_since(start) > Duration::from_secs(40) {
            state.iff.status = Transponder::STATUS_NORMAL;
        }
    } else {
        mem.ident_start = None;
    }

    state.last_update = Some(ctx.now);
    MergeOutcome {
        changed: !before.same_as(state),
        new_aircraft,
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::VolumeMode;

    fn export(unit_id: u32) -> DcsPlayerRadioInfo {
        let mut m = DcsPlayerRadioInfo {
            unit: "FA-18C_hornet".into(),
            unit_id,
            name: "Mav".into(),
            ..Default::default()
        };
        m.radios[0] = DcsRadio {
            modulation: Modulation::Intercom,
            freq: 100.0,
            name: "Intercom".into(),
            ..Default::default()
        };
        m.radios[1] = DcsRadio {
            modulation: Modulation::Am,
            freq: 251e6,
            freq_min: 225e6,
            freq_max: 400e6,
            name: "COMM1".into(),
            freq_mode: FreqMode::Cockpit,
            vol_mode: VolumeMode::Cockpit,
            volume: 0.7,
            enc_mode: EncryptionMode::EncryptionFull,
            enc: true,
            enc_key: 3,
            ..Default::default()
        };
        m.radios[2] = DcsRadio {
            modulation: Modulation::Am,
            freq: 124e6,
            freq_min: 118e6,
            freq_max: 136e6,
            name: "COMM2".into(),
            freq_mode: FreqMode::Overlay,
            vol_mode: VolumeMode::Overlay,
            channel: 2,
            ..Default::default()
        };
        m
    }

    #[test]
    fn first_message_is_new_aircraft_and_loads_channels() {
        let mut state = DcsPlayerRadioInfo::default();
        let mut mem = MergeMemory::default();
        let ctx = MergeContext::default();
        let out = merge(&mut state, export(7), &ctx, &mut mem);
        assert!(out.new_aircraft);
        assert!(out.changed);
        assert!(state.is_current());
        assert_eq!(state.unit_id, 7);
        assert_eq!(state.radios[1].freq, 251e6);
        assert!(state.radios[1].enc);
        assert_eq!(state.radios[1].enc_key, 3);
        assert!(out.events.contains(&MergeEvent::ReloadFixedChannels {
            radio: 2,
            min: 118e6,
            max: 136e6,
            preselected: 2
        }));
        assert!(out.events.contains(&MergeEvent::ClearFixedChannels { radio: 1 }));
        assert_eq!(state.radios[2].channel, -1);
        assert_eq!(state.radios[3].name, "No Radio");
        assert_eq!(state.radios[3].modulation, Modulation::Disabled);
    }

    #[test]
    fn overlay_frequency_survives_cockpit_updates_but_cockpit_one_follows() {
        let mut state = DcsPlayerRadioInfo::default();
        let mut mem = MergeMemory::default();
        let ctx = MergeContext::default();
        merge(&mut state, export(7), &ctx, &mut mem);
        // User tunes radio 2 (overlay) in SRS.
        state.radios[2].freq = 130e6;
        let mut next = export(7);
        next.radios[1].freq = 305e6; // cockpit change
        let out = merge(&mut state, next, &ctx, &mut mem);
        assert!(!out.new_aircraft);
        assert!(out.changed);
        assert_eq!(state.radios[1].freq, 305e6, "cockpit radio follows DCS");
        assert_eq!(
            state.radios[2].freq, 130e6,
            "overlay radio keeps the user's value"
        );
        // Out of range overlay value is clamped.
        state.radios[2].freq = 200e6;
        merge(&mut state, export(7), &ctx, &mut mem);
        assert_eq!(state.radios[2].freq, 136e6);
    }

    #[test]
    fn unit_change_overrides_frequencies_and_resets_encryption() {
        let mut state = DcsPlayerRadioInfo::default();
        let mut mem = MergeMemory::default();
        let ctx = MergeContext::default();
        merge(&mut state, export(7), &ctx, &mut mem);
        state.radios[2].freq = 130e6;
        let out = merge(&mut state, export(8), &ctx, &mut mem);
        assert!(out.new_aircraft);
        assert_eq!(
            state.radios[2].freq, 124e6,
            "new unit: overlay freq reset from DCS"
        );
        assert_eq!(state.radios[2].enc_key, 0);
    }

    #[test]
    fn expansion_and_encryption_server_rules() {
        let mut state = DcsPlayerRadioInfo::default();
        let mut mem = MergeMemory::default();
        let mut msg = export(7);
        msg.radios[2].expansion = true;
        let ctx = MergeContext {
            expansion_allowed: false,
            allow_radio_encryption: false,
            ..Default::default()
        };
        merge(&mut state, msg, &ctx, &mut mem);
        assert_eq!(
            state.radios[2].modulation,
            Modulation::Disabled,
            "expansion radio refused"
        );
        assert_eq!(state.radios[1].enc_mode, EncryptionMode::NoEncryption);
        assert!(!state.radios[1].enc, "encryption forced off by the server");
    }

    #[test]
    fn ptt_and_hotas_settings() {
        let mut state = DcsPlayerRadioInfo::default();
        let mut mem = MergeMemory::default();
        let mut msg = export(7);
        msg.ptt = true;
        msg.control = SwitchControls::InCockpit;
        msg.selected = 2;
        let ctx = MergeContext {
            allow_dcs_ptt: false,
            always_allow_hotas_controls: true,
            ..Default::default()
        };
        merge(&mut state, msg, &ctx, &mut mem);
        assert!(!state.ptt);
        assert_eq!(state.control, SwitchControls::Hotas);
        assert_eq!(
            state.selected, 2,
            "first message: override_freq_and_vol copies selected"
        );
    }

    #[test]
    fn identical_export_is_not_a_change() {
        let mut state = DcsPlayerRadioInfo::default();
        let mut mem = MergeMemory::default();
        let ctx = MergeContext::default();
        let first = merge(&mut state, export(7), &ctx, &mut mem);
        assert!(first.changed);
        let second = merge(&mut state, export(7), &ctx, &mut mem);
        if second.changed {
            let mut again = state.clone();
            merge(&mut again, export(7), &ctx, &mut mem);
            for (i, (a, b)) in state.radios.iter().zip(&again.radios).enumerate() {
                if !a.same_as(b) {
                    panic!("radio {i} differs: {a:?} vs {b:?}");
                }
            }
            panic!(
                "changed but radios equal: unit {} iff {:?}/{:?} ambient {:?}/{:?}",
                state.unit, state.iff, again.iff, state.ambient, again.ambient
            );
        }
        assert!(!second.new_aircraft);
    }

    #[test]
    fn ident_times_out_after_40s() {
        let mut state = DcsPlayerRadioInfo::default();
        let mut mem = MergeMemory::default();
        let mut msg = export(7);
        msg.iff.control = Transponder::CONTROL_COCKPIT;
        msg.iff.status = Transponder::STATUS_IDENT;
        let t0 = Instant::now();
        let ctx = MergeContext {
            now: t0,
            ..Default::default()
        };
        merge(&mut state, msg.clone(), &ctx, &mut mem);
        assert_eq!(state.iff.status, Transponder::STATUS_IDENT);
        let ctx2 = MergeContext {
            now: t0 + Duration::from_secs(41),
            ..Default::default()
        };
        merge(&mut state, msg, &ctx2, &mut mem);
        assert_eq!(state.iff.status, Transponder::STATUS_NORMAL);
    }
}
