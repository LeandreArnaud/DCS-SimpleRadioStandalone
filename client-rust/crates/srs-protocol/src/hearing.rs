//! "Can this set of radios hear that transmission?" — port of
//! `PlayerRadioInfoBase.CanHearTransmission` (shared by the C# server router
//! and the client's tuned-count), plus the client-side candidate scoring from
//! `UDPClientAudioProcessor`.

use crate::message::Radio;
use crate::modulation::Modulation;

/// `RadioBase.FreqCloseEnough`: within 500 Hz (the comment in C# says 1 kHz).
pub fn freq_close_enough(a: f64, b: f64) -> bool {
    (a - b).abs() < 500.0
}

/// Minimum frequency a radio must be tuned to in order to receive (Hz).
pub const MIN_RX_FREQ_HZ: f64 = 10_000.0;

/// Abstraction over `Radio` (wire model) and the DCS-side radio model.
pub trait RxRadio {
    fn freq(&self) -> f64;
    fn sec_freq(&self) -> f64;
    fn modulation(&self) -> Modulation;
    fn enc(&self) -> bool;
    fn enc_key(&self) -> u8;
    fn intercom_unit_id(&self) -> u32;

    /// Key this radio expects: its own key when encryption is on, else `0`.
    fn effective_key(&self) -> u8 {
        if self.enc() {
            self.enc_key()
        } else {
            0
        }
    }
}

impl RxRadio for Radio {
    fn freq(&self) -> f64 {
        self.freq
    }
    fn sec_freq(&self) -> f64 {
        self.sec_freq
    }
    fn modulation(&self) -> Modulation {
        self.modulation
    }
    fn enc(&self) -> bool {
        self.enc
    }
    fn enc_key(&self) -> u8 {
        self.enc_key
    }
    fn intercom_unit_id(&self) -> u32 {
        self.intercom_unit_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HearResult {
    /// Index into the radio list that receives the transmission.
    pub radio_index: usize,
    /// Received on the guard (secondary) frequency.
    pub is_secondary: bool,
    /// False when a radio is tuned but the encryption key does not match:
    /// the client then plays noise instead of audio.
    pub decryptable: bool,
}

/// Exact port of `PlayerRadioInfoBase.CanHearTransmission`.
///
/// - `blocked_radios`: radios the client may not receive on (IRL TX mode,
///   radios currently transmitting). Only applies to the primary frequency.
/// - A decryptable primary/guard match returns immediately; otherwise the
///   last tuned-but-not-decryptable radio is returned with
///   `decryptable == false`. On a guard match the decryptable flag keeps the
///   value from the last primary match (C# quirk, reproduced).
#[allow(clippy::too_many_arguments)]
pub fn can_hear_transmission<R: RxRadio>(
    radios: &[R],
    my_unit_id: u32,
    frequency: f64,
    modulation: Modulation,
    encryption_key: u8,
    strict_encryption: bool,
    sending_unit_id: u32,
    blocked_radios: &[usize],
) -> Option<HearResult> {
    can_hear_transmission_ext(
        radios,
        my_unit_id,
        frequency,
        modulation,
        encryption_key,
        strict_encryption,
        sending_unit_id,
        blocked_radios,
        false,
    )
}

/// Same as [`can_hear_transmission`] with the client-side variant
/// (`DCSPlayerRadioInfo.CanHearTransmission`) selectable: when
/// `intercom_requires_own_unit_id` is true, an intercom match on our own
/// unit also requires `my_unit_id > 0`.
#[allow(clippy::too_many_arguments)]
pub fn can_hear_transmission_ext<R: RxRadio>(
    radios: &[R],
    my_unit_id: u32,
    frequency: f64,
    modulation: Modulation,
    encryption_key: u8,
    strict_encryption: bool,
    sending_unit_id: u32,
    blocked_radios: &[usize],
    intercom_requires_own_unit_id: bool,
) -> Option<HearResult> {
    let mut best: Option<HearResult> = None;
    let mut best_decryptable = false;

    for (i, r) in radios.iter().enumerate() {
        if modulation == Modulation::Disabled || r.modulation() == Modulation::Disabled {
            continue;
        }

        if r.modulation() == Modulation::Intercom && modulation == Modulation::Intercom {
            let own_ok = (!intercom_requires_own_unit_id || my_unit_id > 0) && my_unit_id == sending_unit_id;
            if sending_unit_id > 0 && (own_ok || r.intercom_unit_id() == sending_unit_id) {
                return Some(HearResult {
                    radio_index: i,
                    is_secondary: false,
                    decryptable: true,
                });
            }
            continue;
        }

        let key_ok = |expected: u8| expected == encryption_key || (!strict_encryption && encryption_key == 0);

        if freq_close_enough(r.freq(), frequency) && r.modulation() == modulation && r.freq() > MIN_RX_FREQ_HZ
        {
            let decryptable = key_ok(r.effective_key());
            if decryptable && !blocked_radios.contains(&i) {
                return Some(HearResult {
                    radio_index: i,
                    is_secondary: false,
                    decryptable: true,
                });
            }
            best = Some(HearResult {
                radio_index: i,
                is_secondary: false,
                decryptable,
            });
            best_decryptable = decryptable;
        }

        if freq_close_enough(r.sec_freq(), frequency) && r.sec_freq() > MIN_RX_FREQ_HZ {
            if key_ok(r.effective_key()) {
                return Some(HearResult {
                    radio_index: i,
                    is_secondary: true,
                    decryptable: true,
                });
            }
            best = Some(HearResult {
                radio_index: i,
                is_secondary: true,
                decryptable: best_decryptable,
            });
        }
    }

    best.map(|mut b| {
        b.decryptable = best_decryptable;
        b
    })
}

/// Client-side ranking of several radios that could play the same
/// transmission (`UDPClientAudioProcessor`): decryptable +16, currently
/// selected +8, volume above zero +4. Highest wins.
pub const fn candidate_score(decryptable: bool, is_selected: bool, volume_positive: bool) -> u8 {
    (decryptable as u8) * 16 + (is_selected as u8) * 8 + (volume_positive as u8) * 4
}

#[cfg(test)]
mod tests {
    use super::*;

    fn radio(freq: f64, m: Modulation) -> Radio {
        Radio {
            freq,
            modulation: m,
            ..Default::default()
        }
    }

    #[test]
    fn matches_within_500hz_same_modulation() {
        let radios = [
            radio(251_000_000.0, Modulation::Am),
            radio(30_000_000.0, Modulation::Fm),
        ];
        let r = can_hear_transmission(&radios, 1, 251_000_400.0, Modulation::Am, 0, false, 2, &[]).unwrap();
        assert_eq!(
            r,
            HearResult {
                radio_index: 0,
                is_secondary: false,
                decryptable: true
            }
        );
        assert!(can_hear_transmission(&radios, 1, 251_000_600.0, Modulation::Am, 0, false, 2, &[]).is_none());
        assert!(can_hear_transmission(&radios, 1, 251_000_000.0, Modulation::Fm, 0, false, 2, &[]).is_none());
    }

    #[test]
    fn intercom_needs_same_unit_or_instructor_override() {
        let mut radios = [radio(1.0, Modulation::Intercom)];
        assert!(can_hear_transmission(&radios, 5, 100.0, Modulation::Intercom, 0, false, 5, &[]).is_some());
        assert!(can_hear_transmission(&radios, 5, 100.0, Modulation::Intercom, 0, false, 6, &[]).is_none());
        assert!(can_hear_transmission(&radios, 5, 100.0, Modulation::Intercom, 0, false, 0, &[]).is_none());
        radios[0].intercom_unit_id = 6;
        assert!(can_hear_transmission(&radios, 5, 100.0, Modulation::Intercom, 0, false, 6, &[]).is_some());
        // Server variant: unit 0 == sending unit 0 is refused by sending > 0 anyway,
        // but unit 0 vs sending 0 differs between variants only when sending > 0 is false.
        assert!(
            can_hear_transmission_ext(&radios, 0, 100.0, Modulation::Intercom, 0, false, 0, &[], true)
                .is_none()
        );
    }

    #[test]
    fn encryption_rules() {
        let mut radios = [radio(251_000_000.0, Modulation::Am)];
        // Clear radio, encrypted transmission: tuned but not decryptable.
        let r = can_hear_transmission(&radios, 1, 251_000_000.0, Modulation::Am, 3, false, 2, &[]).unwrap();
        assert!(!r.decryptable);
        // Encrypted radio, clear transmission, non-strict: audible.
        radios[0].enc = true;
        radios[0].enc_key = 3;
        assert!(
            can_hear_transmission(&radios, 1, 251_000_000.0, Modulation::Am, 0, false, 2, &[])
                .unwrap()
                .decryptable
        );
        // Strict: not decryptable.
        assert!(
            !can_hear_transmission(&radios, 1, 251_000_000.0, Modulation::Am, 0, true, 2, &[])
                .unwrap()
                .decryptable
        );
        // Matching key: decryptable.
        assert!(
            can_hear_transmission(&radios, 1, 251_000_000.0, Modulation::Am, 3, true, 2, &[])
                .unwrap()
                .decryptable
        );
    }

    #[test]
    fn guard_frequency_is_secondary_and_ignores_modulation() {
        let mut r = radio(251_000_000.0, Modulation::Am);
        r.sec_freq = 243_000_000.0;
        let res = can_hear_transmission(&[r], 1, 243_000_000.0, Modulation::Fm, 0, false, 2, &[]).unwrap();
        assert!(res.is_secondary);
    }

    #[test]
    fn blocked_radio_is_reported_as_tuned_but_not_returned_first() {
        let radios = [
            radio(251_000_000.0, Modulation::Am),
            radio(251_000_000.0, Modulation::Am),
        ];
        let r = can_hear_transmission(&radios, 1, 251_000_000.0, Modulation::Am, 0, false, 2, &[0]).unwrap();
        assert_eq!(r.radio_index, 1);
        let r =
            can_hear_transmission(&radios, 1, 251_000_000.0, Modulation::Am, 0, false, 2, &[0, 1]).unwrap();
        assert_eq!(r.radio_index, 1);
        assert!(r.decryptable);
    }

    #[test]
    fn scoring() {
        assert_eq!(candidate_score(true, true, true), 28);
        assert_eq!(candidate_score(false, true, true), 12);
        assert!(candidate_score(true, false, false) > candidate_score(false, true, true));
    }
}
