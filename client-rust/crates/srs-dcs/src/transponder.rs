//! Port of `DCS-SR-Client/Utils/TransponderHelper.cs`: edits the IFF block
//! of the player radio info when the overlay controls it. Every mutation
//! returns `true` when something changed; the caller forces a sync
//! (`LastSent = 0`).

use srs_protocol::Transponder;

use crate::model::DcsPlayerRadioInfo;

/// `GetTransponder`: the IFF block when data is current and the transponder
/// is fitted; with `only_if_overlay_controls`, only when the overlay drives it.
pub fn transponder(info: &DcsPlayerRadioInfo, only_if_overlay_controls: bool) -> Option<&Transponder> {
    if !info.is_current() || info.iff.control == Transponder::CONTROL_DISABLED {
        return None;
    }
    if only_if_overlay_controls && info.iff.control != Transponder::CONTROL_OVERLAY {
        return None;
    }
    Some(&info.iff)
}

fn overlay_transponder(info: &mut DcsPlayerRadioInfo) -> Option<&mut Transponder> {
    if !info.is_current()
        || info.iff.control == Transponder::CONTROL_DISABLED
        || info.iff.control != Transponder::CONTROL_OVERLAY
    {
        return None;
    }
    Some(&mut info.iff)
}

/// `ToggleIdent`: NORMAL ↔ IDENT (nothing when OFF).
pub fn toggle_ident(info: &mut DcsPlayerRadioInfo) -> bool {
    match overlay_transponder(info) {
        Some(t) if t.status != Transponder::STATUS_OFF => {
            t.status = if t.status == Transponder::STATUS_NORMAL {
                Transponder::STATUS_IDENT
            } else {
                Transponder::STATUS_NORMAL
            };
            true
        }
        _ => false,
    }
}

/// `Mode4Toggle` (the C# always returns false).
pub fn mode4_toggle(info: &mut DcsPlayerRadioInfo) -> bool {
    if let Some(t) = overlay_transponder(info) {
        t.mode4 = !t.mode4;
    }
    false
}

/// Octal clamp used by modes 2 and 3: every decimal digit above 7 becomes 7.
fn clamp_octal_digits(code: i32) -> i32 {
    let digits: String = code
        .abs()
        .to_string()
        .chars()
        .map(|c| if c.to_digit(10).unwrap_or(0) > 7 { '7' } else { c })
        .collect();
    digits.parse().unwrap_or(0)
}

/// `SetMode3`: negative = not set, else digits clamped to octal.
pub fn set_mode3(info: &mut DcsPlayerRadioInfo, mode3: i32) -> bool {
    match overlay_transponder(info) {
        Some(t) => {
            t.mode3 = if mode3 < 0 { -1 } else { clamp_octal_digits(mode3) };
            true
        }
        None => false,
    }
}

/// `SetMode2`.
pub fn set_mode2(info: &mut DcsPlayerRadioInfo, mode2: i32) -> bool {
    match overlay_transponder(info) {
        Some(t) => {
            t.mode2 = if mode2 < 0 { -1 } else { clamp_octal_digits(mode2) };
            true
        }
        None => false,
    }
}

/// `SetMode1`: first digit 0..7, second 0..3.
pub fn set_mode1(info: &mut DcsPlayerRadioInfo, mode1: i32) -> bool {
    match overlay_transponder(info) {
        Some(t) => {
            t.mode1 = if mode1 < 0 {
                -1
            } else {
                let first = (mode1 / 10).clamp(0, 7);
                let second = (mode1 % 10).min(3);
                first * 10 + second
            };
            true
        }
        None => false,
    }
}

/// `TogglePower`: OFF ↔ NORMAL.
pub fn toggle_power(info: &mut DcsPlayerRadioInfo) -> bool {
    match overlay_transponder(info) {
        Some(t) => {
            t.status = if t.status == Transponder::STATUS_OFF {
                Transponder::STATUS_NORMAL
            } else {
                Transponder::STATUS_OFF
            };
            true
        }
        None => false,
    }
}

/// `SetMode4`.
pub fn set_mode4(info: &mut DcsPlayerRadioInfo, on: bool) -> bool {
    match overlay_transponder(info) {
        Some(t) => {
            t.mode4 = on;
            true
        }
        None => false,
    }
}

/// `SetIdent`: IDENT or NORMAL, nothing when OFF.
pub fn set_ident(info: &mut DcsPlayerRadioInfo, on: bool) -> bool {
    match overlay_transponder(info) {
        Some(t) if t.status != Transponder::STATUS_OFF => {
            t.status = if on {
                Transponder::STATUS_IDENT
            } else {
                Transponder::STATUS_NORMAL
            };
            true
        }
        _ => false,
    }
}

/// `SetPower`.
pub fn set_power(info: &mut DcsPlayerRadioInfo, on: bool) -> bool {
    match overlay_transponder(info) {
        Some(t) => {
            t.status = if on {
                Transponder::STATUS_NORMAL
            } else {
                Transponder::STATUS_OFF
            };
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(control: i32) -> DcsPlayerRadioInfo {
        let mut i = DcsPlayerRadioInfo::default();
        i.touch();
        i.iff.control = control;
        i.iff.status = Transponder::STATUS_NORMAL;
        i
    }

    #[test]
    fn only_overlay_controlled_transponders_change() {
        let mut cockpit = info(Transponder::CONTROL_COCKPIT);
        assert!(!toggle_ident(&mut cockpit));
        assert!(transponder(&cockpit, false).is_some());
        assert!(transponder(&cockpit, true).is_none());
        let mut disabled = info(Transponder::CONTROL_DISABLED);
        assert!(transponder(&disabled, false).is_none());
        assert!(!set_power(&mut disabled, true));
        let mut stale = info(Transponder::CONTROL_OVERLAY);
        stale.last_update = None;
        assert!(!set_mode3(&mut stale, 1200));
    }

    #[test]
    fn ident_power_and_modes() {
        let mut i = info(Transponder::CONTROL_OVERLAY);
        assert!(toggle_ident(&mut i));
        assert_eq!(i.iff.status, Transponder::STATUS_IDENT);
        assert!(toggle_ident(&mut i));
        assert_eq!(i.iff.status, Transponder::STATUS_NORMAL);
        assert!(toggle_power(&mut i));
        assert_eq!(i.iff.status, Transponder::STATUS_OFF);
        assert!(!toggle_ident(&mut i), "no ident when off");
        assert!(set_power(&mut i, true));
        assert!(set_mode3(&mut i, 1289));
        assert_eq!(i.iff.mode3, 1277);
        assert!(set_mode3(&mut i, -5));
        assert_eq!(i.iff.mode3, -1);
        assert!(set_mode2(&mut i, 98));
        assert_eq!(i.iff.mode2, 77);
        assert!(set_mode1(&mut i, 95));
        assert_eq!(i.iff.mode1, 73);
        assert!(set_mode1(&mut i, 12));
        assert_eq!(i.iff.mode1, 12);
        assert!(!mode4_toggle(&mut i));
        assert!(i.iff.mode4);
        assert!(set_mode4(&mut i, false));
        assert!(!i.iff.mode4);
        assert!(set_ident(&mut i, true));
        assert_eq!(i.iff.status, Transponder::STATUS_IDENT);
        assert!(set_power(&mut i, false));
        assert!(!set_ident(&mut i, true));
    }
}
