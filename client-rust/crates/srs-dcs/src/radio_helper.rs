//! Port of `DCS-SR-Client/Utils/RadioHelper.cs`: every manual action on the
//! radio state (overlay, bindings, UDP commands). Each function returns
//! `true` when the state changed in a way that must be re-sent to the server
//! (`ClientStateSingleton.LastSent = 0` in C#).

use srs_protocol::radio_calc::{link16, MHZ};
use srs_protocol::Modulation;

use crate::model::{
    DcsPlayerRadioInfo, EncryptionMode, FreqMode, RetransmitMode, SwitchControls, VolumeMode, UNIT_ID_OFFSET,
};

/// One entry of a radio's channel list (`PresetChannel`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PresetChannel {
    pub text: String,
    /// Frequency in Hz.
    pub value: f64,
    /// 1-based channel number.
    pub channel: i32,
    /// Link-16 channel (MIDS presets only), 0 otherwise.
    pub mids_channel: i32,
}

/// `GetRadio`: only when the radio data is current and the index exists.
fn radio_ok(info: &DcsPlayerRadioInfo, radio_id: usize) -> bool {
    info.is_current() && radio_id < info.radios.len()
}

pub fn toggle_guard(info: &mut DcsPlayerRadioInfo, radio_id: usize) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    let r = &mut info.radios[radio_id];
    if (r.freq_mode == FreqMode::Overlay || r.guard_freq_mode == FreqMode::Overlay)
        && r.modulation != Modulation::Mids
    {
        r.sec_freq = if r.sec_freq > 0.0 { 0.0 } else { 1.0 };
        return true;
    }
    false
}

pub fn set_guard(info: &mut DcsPlayerRadioInfo, radio_id: usize, enabled: bool) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    let r = &mut info.radios[radio_id];
    if (r.freq_mode == FreqMode::Overlay || r.guard_freq_mode == FreqMode::Overlay)
        && r.modulation != Modulation::Mids
    {
        r.sec_freq = if enabled { 1.0 } else { 0.0 };
        return true;
    }
    false
}

/// `UpdateRadioFrequency`. Returns `(changed, in_limit)`.
pub fn update_frequency(
    info: &mut DcsPlayerRadioInfo,
    requested: f64,
    radio_id: usize,
    delta: bool,
    in_mhz: bool,
    rotary_style: bool,
) -> (bool, bool) {
    let mut in_limit = true;
    let mut frequency = requested;
    if in_mhz {
        frequency *= MHZ;
    }
    if radio_id == 0 || !radio_ok(info, radio_id) {
        return (false, in_limit);
    }
    let r = &mut info.radios[radio_id];
    if r.modulation == Modulation::Disabled
        || r.modulation == Modulation::Intercom
        || r.freq_mode != FreqMode::Overlay
    {
        return (false, in_limit);
    }
    if delta {
        match r.modulation {
            Modulation::Mids => r.freq = link16::validate_frequency(r.freq + frequency).round(),
            _ => {
                if rotary_style {
                    let adjusted = (r.freq / frequency).round().abs() as i64;
                    let delta_position = adjusted % 10;
                    let roll_over = if frequency < 0.0 { 0 } else { 9 };
                    let future = frequency + r.freq;
                    if frequency.abs() <= 1_000_000.0 {
                        if delta_position == roll_over {
                            frequency *= -9.0;
                        }
                    } else if (frequency < 0.0 && r.freq_min > future) || future > r.freq_max {
                        frequency = 0.0;
                    }
                }
                r.freq = (r.freq + frequency).round().trunc();
            }
        }
    } else {
        match r.modulation {
            Modulation::Mids => r.freq = link16::channel_to_frequency(requested as i32),
            _ => r.freq = frequency.round().trunc(),
        }
    }
    if r.freq > r.freq_max {
        in_limit = false;
        r.freq = r.freq_max;
    } else if r.freq < r.freq_min {
        in_limit = false;
        r.freq = r.freq_min;
    }
    r.channel = -1;
    (true, in_limit)
}

/// `SelectRadio`: only with HOTAS control and an enabled radio.
pub fn select_radio(info: &mut DcsPlayerRadioInfo, radio_id: usize) -> bool {
    if !radio_ok(info, radio_id) {
        return false;
    }
    if info.radios[radio_id].modulation != Modulation::Disabled && info.control == SwitchControls::Hotas {
        info.selected = radio_id as i16;
        return true;
    }
    false
}

pub fn select_next_radio(info: &mut DcsPlayerRadioInfo) {
    if !info.is_current() || info.control != SwitchControls::Hotas {
        return;
    }
    let n = info.radios.len() as i32;
    let sel = i32::from(info.selected);
    if sel < 0 || sel > n || sel + 1 > n {
        select_radio(info, 1);
        return;
    }
    for i in (sel + 1)..n {
        if select_radio(info, i as usize) {
            return;
        }
    }
    for i in 1..sel {
        if select_radio(info, i as usize) {
            return;
        }
    }
}

pub fn select_previous_radio(info: &mut DcsPlayerRadioInfo) {
    if !info.is_current() || info.control != SwitchControls::Hotas {
        return;
    }
    let n = info.radios.len() as i32;
    let sel = i32::from(info.selected);
    if sel < 0 || sel > n {
        info.selected = 1;
        return;
    }
    for i in (1..sel).rev() {
        if select_radio(info, i as usize) {
            return;
        }
    }
    // C# iterates from radios.Length down to current+1 (index Length is out of
    // range and silently ignored by GetRadio).
    for i in ((sel + 1)..=n).rev() {
        if (i as usize) < info.radios.len() && select_radio(info, i as usize) {
            return;
        }
    }
}

pub fn toggle_encryption(info: &mut DcsPlayerRadioInfo, radio_id: usize) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    let r = &mut info.radios[radio_id];
    if r.modulation != Modulation::Disabled
        && r.modulation != Modulation::Mids
        && r.enc_mode == EncryptionMode::EncryptionJustOverlay
    {
        r.enc = !r.enc;
        return true;
    }
    false
}

pub fn set_encryption_key(info: &mut DcsPlayerRadioInfo, radio_id: usize, key: i32) -> bool {
    if !radio_ok(info, radio_id) {
        return false;
    }
    let r = &mut info.radios[radio_id];
    if r.modulation != Modulation::Disabled
        && r.modulation != Modulation::Mids
        && matches!(
            r.enc_mode,
            EncryptionMode::EncryptionCockpitToggleOverlayCode | EncryptionMode::EncryptionJustOverlay
        )
    {
        r.enc_key = key.clamp(1, 252) as u8;
        return true;
    }
    false
}

pub fn increase_encryption_key(info: &mut DcsPlayerRadioInfo, radio_id: usize) -> bool {
    if !radio_ok(info, radio_id) {
        return false;
    }
    let k = i32::from(info.radios[radio_id].enc_key);
    set_encryption_key(info, radio_id, k + 1)
}

pub fn decrease_encryption_key(info: &mut DcsPlayerRadioInfo, radio_id: usize) -> bool {
    if !radio_ok(info, radio_id) {
        return false;
    }
    let k = i32::from(info.radios[radio_id].enc_key);
    set_encryption_key(info, radio_id, k - 1)
}

/// `SelectRadioChannel`.
pub fn select_channel(info: &mut DcsPlayerRadioInfo, radio_id: usize, preset: &PresetChannel) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    let r = &mut info.radios[radio_id];
    if r.modulation != Modulation::Disabled
        && r.modulation != Modulation::Intercom
        && r.freq_mode == FreqMode::Overlay
    {
        r.freq = preset.value;
        r.channel = preset.channel;
        return true;
    }
    false
}

/// `RadioChannelUp`: `channels` is that radio's preset list.
pub fn channel_up(info: &mut DcsPlayerRadioInfo, radio_id: usize, channels: &[PresetChannel]) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) || channels.is_empty() {
        return false;
    }
    if info.radios[radio_id].modulation == Modulation::Disabled || info.control != SwitchControls::Hotas {
        return false;
    }
    let current = info.radios[radio_id].channel;
    let next = current + 1;
    let preset = if (channels.len() as i32) < next || current < 1 {
        &channels[0]
    } else {
        &channels[(next - 1) as usize]
    };
    let preset = preset.clone();
    select_channel(info, radio_id, &preset)
}

/// `RadioChannelDown`.
pub fn channel_down(info: &mut DcsPlayerRadioInfo, radio_id: usize, channels: &[PresetChannel]) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) || channels.is_empty() {
        return false;
    }
    if info.radios[radio_id].modulation == Modulation::Disabled || info.control != SwitchControls::Hotas {
        return false;
    }
    let previous = info.radios[radio_id].channel - 1;
    let preset = if previous < 1 {
        channels.last().unwrap()
    } else {
        &channels[((previous - 1) as usize).min(channels.len() - 1)]
    };
    let preset = preset.clone();
    select_channel(info, radio_id, &preset)
}

pub fn set_volume(info: &mut DcsPlayerRadioInfo, radio_id: usize, volume: f32) {
    let volume = volume.clamp(0.0, 1.0);
    if !radio_ok(info, radio_id) {
        return;
    }
    let r = &mut info.radios[radio_id];
    if r.modulation != Modulation::Disabled && r.vol_mode == VolumeMode::Overlay {
        r.volume = volume;
    }
}

pub fn volume_up(info: &mut DcsPlayerRadioInfo, radio_id: usize) {
    if !radio_ok(info, radio_id) {
        return;
    }
    let r = &mut info.radios[radio_id];
    if r.modulation != Modulation::Disabled && r.vol_mode == VolumeMode::Overlay {
        r.volume = (r.volume + 0.1).min(1.0);
    }
}

pub fn volume_down(info: &mut DcsPlayerRadioInfo, radio_id: usize) {
    if !radio_ok(info, radio_id) {
        return;
    }
    let r = &mut info.radios[radio_id];
    if r.modulation != Modulation::Disabled && r.vol_mode == VolumeMode::Overlay {
        r.volume = (r.volume - 0.1).max(0.0);
    }
}

pub fn toggle_retransmit(info: &mut DcsPlayerRadioInfo, radio_id: usize) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    let r = &mut info.radios[radio_id];
    if r.rt_mode == RetransmitMode::Overlay {
        r.retransmit = !r.retransmit;
        return true;
    }
    false
}

pub fn set_retransmit(info: &mut DcsPlayerRadioInfo, radio_id: usize, enabled: bool) -> bool {
    if radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    let r = &mut info.radios[radio_id];
    if r.rt_mode == RetransmitMode::Overlay {
        r.retransmit = enabled;
        return true;
    }
    false
}

pub fn set_global_simultaneous_transmission(info: &mut DcsPlayerRadioInfo, enabled: bool) {
    info.simultaneous_transmission = enabled;
    if !enabled {
        for r in &mut info.radios {
            r.simul = false;
        }
    }
}

pub fn toggle_global_simultaneous_transmission(info: &mut DcsPlayerRadioInfo) {
    let v = !info.simultaneous_transmission;
    set_global_simultaneous_transmission(info, v);
}

pub fn set_simultaneous(info: &mut DcsPlayerRadioInfo, radio_id: usize, enabled: bool) -> bool {
    if !info.simultaneous_transmission || radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    info.radios[radio_id].simul = enabled;
    true
}

pub fn toggle_simultaneous(info: &mut DcsPlayerRadioInfo, radio_id: usize) -> bool {
    if !info.simultaneous_transmission || radio_id == 0 || !radio_ok(info, radio_id) {
        return false;
    }
    let v = !info.radios[radio_id].simul;
    info.radios[radio_id].simul = v;
    true
}

/// `SetIntercom`: EAM intercom group. Returns the new offset when applied.
pub fn set_intercom(info: &mut DcsPlayerRadioInfo, channel: u32) -> Option<u32> {
    if info.is_current() && info.unit_id >= UNIT_ID_OFFSET {
        info.unit_id = UNIT_ID_OFFSET + channel;
        return Some(channel);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DcsRadio;

    fn info() -> DcsPlayerRadioInfo {
        let mut i = DcsPlayerRadioInfo::default();
        i.radios[0] = DcsRadio {
            modulation: Modulation::Intercom,
            freq: 100.0,
            ..Default::default()
        };
        i.radios[1] = DcsRadio {
            modulation: Modulation::Am,
            freq: 251e6,
            freq_min: 225e6,
            freq_max: 400e6,
            freq_mode: FreqMode::Overlay,
            vol_mode: VolumeMode::Overlay,
            enc_mode: EncryptionMode::EncryptionJustOverlay,
            ..Default::default()
        };
        i.radios[2] = DcsRadio {
            modulation: Modulation::Fm,
            freq: 30e6,
            freq_min: 30e6,
            freq_max: 76e6,
            ..Default::default()
        };
        i.touch();
        i
    }

    #[test]
    fn frequency_delta_and_set_with_limits() {
        let mut i = info();
        assert_eq!(update_frequency(&mut i, 1.0, 1, true, true, false), (true, true));
        assert_eq!(i.radios[1].freq, 252e6);
        assert_eq!(
            update_frequency(&mut i, 500.0, 1, true, true, false),
            (true, false)
        );
        assert_eq!(i.radios[1].freq, 400e6);
        assert_eq!(
            update_frequency(&mut i, 300.5, 1, false, true, false),
            (true, true)
        );
        assert_eq!(i.radios[1].freq, 300.5e6);
        assert_eq!(i.radios[1].channel, -1);
        // Cockpit-tuned radio refuses.
        assert_eq!(update_frequency(&mut i, 1.0, 2, true, true, false), (false, true));
        // Not current: refuses.
        i.last_update = None;
        assert_eq!(update_frequency(&mut i, 1.0, 1, true, true, false), (false, true));
    }

    #[test]
    fn rotary_increment_rolls_over_digit() {
        let mut i = info();
        i.radios[1].freq = 259e6;
        update_frequency(&mut i, 1.0, 1, true, true, true);
        assert_eq!(i.radios[1].freq, 250e6, "9 -> 0 at the MHz digit");
    }

    #[test]
    fn select_next_previous_skip_disabled() {
        let mut i = info();
        i.selected = 1;
        select_next_radio(&mut i);
        assert_eq!(i.selected, 2);
        select_next_radio(&mut i);
        assert_eq!(i.selected, 1, "wraps over disabled radios");
        select_previous_radio(&mut i);
        assert_eq!(i.selected, 2);
        i.control = SwitchControls::InCockpit;
        assert!(!select_radio(&mut i, 1));
    }

    #[test]
    fn guard_encryption_volume_retransmit() {
        let mut i = info();
        assert!(toggle_guard(&mut i, 1));
        assert_eq!(i.radios[1].sec_freq, 0.0);
        assert!(set_guard(&mut i, 1, true));
        assert_eq!(i.radios[1].sec_freq, 1.0);
        assert!(!toggle_guard(&mut i, 2), "cockpit guard mode");
        assert!(toggle_encryption(&mut i, 1));
        assert!(i.radios[1].enc);
        assert!(set_encryption_key(&mut i, 1, 300));
        assert_eq!(i.radios[1].enc_key, 252);
        assert!(decrease_encryption_key(&mut i, 1));
        assert_eq!(i.radios[1].enc_key, 251);
        set_volume(&mut i, 1, 2.0);
        assert_eq!(i.radios[1].volume, 1.0);
        volume_down(&mut i, 1);
        assert!((i.radios[1].volume - 0.9).abs() < 1e-6);
        assert!(!toggle_retransmit(&mut i, 1), "rtMode disabled");
        i.radios[1].rt_mode = RetransmitMode::Overlay;
        assert!(toggle_retransmit(&mut i, 1));
        assert!(i.radios[1].retransmit);
    }

    #[test]
    fn channels() {
        let mut i = info();
        let ch = vec![
            PresetChannel {
                text: "a".into(),
                value: 230e6,
                channel: 1,
                mids_channel: 0,
            },
            PresetChannel {
                text: "b".into(),
                value: 240e6,
                channel: 2,
                mids_channel: 0,
            },
        ];
        assert!(channel_up(&mut i, 1, &ch));
        assert_eq!(i.radios[1].channel, 1);
        assert!(channel_up(&mut i, 1, &ch));
        assert_eq!(i.radios[1].channel, 2);
        assert!(channel_up(&mut i, 1, &ch));
        assert_eq!(i.radios[1].channel, 1, "wraps");
        assert!(channel_down(&mut i, 1, &ch));
        assert_eq!(i.radios[1].channel, 2, "wraps backwards");
        assert_eq!(i.radios[1].freq, 240e6);
    }

    #[test]
    fn simultaneous_and_intercom() {
        let mut i = info();
        assert!(!set_simultaneous(&mut i, 1, true), "global flag off");
        set_global_simultaneous_transmission(&mut i, true);
        assert!(set_simultaneous(&mut i, 1, true));
        set_global_simultaneous_transmission(&mut i, false);
        assert!(!i.radios[1].simul);
        assert_eq!(set_intercom(&mut i, 3), None, "not an EAM unit");
        i.unit_id = UNIT_ID_OFFSET + 1;
        assert_eq!(set_intercom(&mut i, 3), Some(3));
        assert_eq!(i.unit_id, UNIT_ID_OFFSET + 3);
    }
}
