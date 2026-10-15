//! Transmit side: which radios a frame goes out on (`PTTPressed`) and the
//! packet construction of `UDPClientAudioProcessor.Send`. Pure functions
//! over [`RadioState`]; the encoder thread (`encoder.rs`) drives them.

use std::time::{Duration, Instant};

use srs_dcs::DcsRadio;
use srs_protocol::{FrequencySegment, Modulation, ShortGuid, VoicePacket};

use crate::radio_state::RadioState;

/// Inputs that come from outside the radio state.
#[derive(Debug, Clone, Copy)]
pub struct TxInputs {
    /// Main PTT (binding / HOTAS / DCS), after start/release delays.
    pub ptt: bool,
    /// `IntercomPTT` binding.
    pub intercom_ptt: bool,
    /// VOX decision for this frame (`true` when VOX is disabled).
    pub voice: bool,
    /// `VOXMinimumTime` (ms): hot-mic hold after the last voice frame.
    pub vox_minimum_time_ms: u64,
    /// VAICOM `InhibitTX` received less than 5 s ago.
    pub inhibit_tx: bool,
}

/// Mutable bits the C# code keeps between frames.
#[derive(Debug, Default)]
pub struct TxMemory {
    /// `_lastVOXSend`.
    pub last_vox_send: Option<Instant>,
    /// `_packetNumber`, starts at 1.
    pub packet_number: u64,
}

impl TxMemory {
    pub fn new() -> Self {
        Self {
            last_vox_send: None,
            packet_number: 1,
        }
    }
}

/// Radios a frame is transmitted on, and the index shown as "sending on".
#[derive(Debug, Clone, PartialEq)]
pub struct TxSelection {
    pub radios: Vec<usize>,
    pub sending_on: Option<usize>,
}

fn can_transmit(r: &DcsRadio) -> bool {
    r.modulation != Modulation::Disabled
        && (r.freq > 100.0 || r.modulation == Modulation::Intercom)
        && !r.rx_only
}

/// Port of `PTTPressed`.
pub fn ptt_pressed(state: &RadioState, inputs: &TxInputs, mem: &mut TxMemory) -> TxSelection {
    let none = TxSelection {
        radios: Vec::new(),
        sending_on: None,
    };
    if inputs.inhibit_tx {
        return none;
    }
    let info = &state.dcs;
    let dcs_ptt = info.ptt;

    // Hot intercom (Gazelle etc.) or explicit intercom PTT.
    if ((info.intercom_hot_mic && info.selected != 0 && !inputs.ptt && !dcs_ptt) || inputs.intercom_ptt)
        && info
            .radios
            .first()
            .is_some_and(|r| r.modulation == Modulation::Intercom)
    {
        let intercom = TxSelection {
            radios: vec![0],
            sending_on: Some(0),
        };
        if info.intercom_hot_mic && inputs.voice {
            mem.last_vox_send = Some(Instant::now());
            return intercom;
        }
        if info.intercom_hot_mic && !inputs.voice {
            let hold = Duration::from_millis(inputs.vox_minimum_time_ms);
            return if mem.last_vox_send.is_some_and(|t| t.elapsed() < hold) {
                intercom
            } else {
                none
            };
        }
        return intercom;
    }

    let mut radios = Vec::new();
    let mut sending_on = None;
    if inputs.ptt || dcs_ptt {
        let selected = info.selected_index();
        let mut selected_radio: Option<&DcsRadio> = None;
        if let Some(i) = selected {
            let r = &info.radios[i];
            selected_radio = Some(r);
            if can_transmit(r) {
                sending_on = Some(i);
                radios.push(i);
            }
        }
        if info.simultaneous_transmission {
            // AWACS on the intercom: never spray every radio.
            if selected == Some(0)
                && selected_radio.is_some_and(|r| r.modulation == Modulation::Intercom)
                && !info.in_aircraft
            {
                return TxSelection {
                    radios: vec![0],
                    sending_on: Some(0),
                };
            }
            for (i, r) in info.radios.iter().enumerate() {
                if r.simul && can_transmit(r) && !radios.contains(&i) {
                    if sending_on.is_none() {
                        sending_on = Some(i);
                    }
                    radios.push(i);
                }
            }
        }
    }
    TxSelection { radios, sending_on }
}

/// Port of the packet building in `Send`: dedupe (freq, modulation, key),
/// `UnitId` overridden by `IntercomUnitId` for instructor mode. Returns the
/// packet and the first (frequency, modulation, key) for local effects.
pub fn build_packet(
    state: &RadioState,
    selection: &TxSelection,
    opus: &[u8],
    guid: ShortGuid,
    mem: &mut TxMemory,
) -> Option<VoicePacket> {
    let mut freqs: Vec<FrequencySegment> = Vec::with_capacity(selection.radios.len());
    let mut group_override = 0u32;
    for &i in &selection.radios {
        let r = &state.dcs.radios[i];
        let key = if r.enc { r.enc_key } else { 0 };
        let seg = FrequencySegment {
            freq_hz: r.freq,
            modulation: r.modulation.as_u8(),
            encryption: key,
        };
        if freqs.iter().any(|f| {
            f.freq_hz == seg.freq_hz && f.modulation == seg.modulation && f.encryption == seg.encryption
        }) {
            continue;
        }
        if r.modulation == Modulation::Intercom && r.intercom_unit_id > 0 {
            group_override = r.intercom_unit_id;
        }
        freqs.push(seg);
    }
    if freqs.is_empty() {
        return None;
    }
    let packet = VoicePacket {
        audio: opus.to_vec(),
        frequencies: freqs,
        unit_id: if group_override > 0 {
            group_override
        } else {
            state.dcs.unit_id
        },
        packet_number: mem.packet_number,
        retransmission_count: 0,
        original_client_guid: guid,
        client_guid: guid,
    };
    mem.packet_number += 1;
    Some(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(radios: Vec<DcsRadio>, selected: i16) -> RadioState {
        let mut s = RadioState::new();
        s.dcs.radios = radios;
        s.dcs.radios.resize_with(11, DcsRadio::no_radio);
        s.dcs.selected = selected;
        s.dcs.unit_id = 42;
        s
    }

    fn am(freq: f64) -> DcsRadio {
        DcsRadio {
            modulation: Modulation::Am,
            freq,
            ..Default::default()
        }
    }

    #[test]
    fn selected_radio_transmits_on_ptt() {
        let s = state_with(
            vec![
                DcsRadio {
                    modulation: Modulation::Intercom,
                    freq: 100.0,
                    ..Default::default()
                },
                am(251e6),
            ],
            1,
        );
        let mut mem = TxMemory::new();
        let inputs = TxInputs {
            ptt: true,
            intercom_ptt: false,
            voice: true,
            vox_minimum_time_ms: 300,
            inhibit_tx: false,
        };
        let sel = ptt_pressed(&s, &inputs, &mut mem);
        assert_eq!(
            sel,
            TxSelection {
                radios: vec![1],
                sending_on: Some(1)
            }
        );
        let p = build_packet(&s, &sel, &[1, 2, 3], ShortGuid::new_random(), &mut mem).unwrap();
        assert_eq!(p.frequencies.len(), 1);
        assert_eq!(p.frequencies[0].freq_hz, 251e6);
        assert_eq!(p.unit_id, 42);
        assert_eq!(p.packet_number, 1);
        assert_eq!(mem.packet_number, 2);
        let no = ptt_pressed(&s, &TxInputs { ptt: false, ..inputs }, &mut mem);
        assert!(no.radios.is_empty());
    }

    #[test]
    fn simultaneous_adds_simul_radios_and_dedupes() {
        let mut r2 = am(251e6);
        r2.simul = true;
        let mut r3 = am(305e6);
        r3.simul = true;
        let mut s = state_with(vec![DcsRadio::no_radio(), am(251e6), r2, r3], 1);
        s.dcs.simultaneous_transmission = true;
        let mut mem = TxMemory::new();
        let inputs = TxInputs {
            ptt: true,
            intercom_ptt: false,
            voice: true,
            vox_minimum_time_ms: 300,
            inhibit_tx: false,
        };
        let sel = ptt_pressed(&s, &inputs, &mut mem);
        assert_eq!(sel.radios, vec![1, 2, 3]);
        let p = build_packet(&s, &sel, &[0], ShortGuid::new_random(), &mut mem).unwrap();
        assert_eq!(p.frequencies.len(), 2, "251 MHz AM clear appears once");
    }

    #[test]
    fn intercom_ptt_and_instructor_override() {
        let mut ic = DcsRadio {
            modulation: Modulation::Intercom,
            freq: 100.0,
            ..Default::default()
        };
        ic.intercom_unit_id = 7;
        let s = state_with(vec![ic, am(251e6)], 1);
        let mut mem = TxMemory::new();
        let inputs = TxInputs {
            ptt: false,
            intercom_ptt: true,
            voice: true,
            vox_minimum_time_ms: 300,
            inhibit_tx: false,
        };
        let sel = ptt_pressed(&s, &inputs, &mut mem);
        assert_eq!(sel.sending_on, Some(0));
        let p = build_packet(&s, &sel, &[0], ShortGuid::new_random(), &mut mem).unwrap();
        assert_eq!(p.unit_id, 7, "IntercomUnitId overrides the unit id");
    }

    #[test]
    fn hot_mic_holds_after_voice_stops() {
        let mut s = state_with(
            vec![
                DcsRadio {
                    modulation: Modulation::Intercom,
                    freq: 100.0,
                    ..Default::default()
                },
                am(251e6),
            ],
            1,
        );
        s.dcs.intercom_hot_mic = true;
        let mut mem = TxMemory::new();
        let voice = TxInputs {
            ptt: false,
            intercom_ptt: false,
            voice: true,
            vox_minimum_time_ms: 300,
            inhibit_tx: false,
        };
        assert_eq!(ptt_pressed(&s, &voice, &mut mem).radios, vec![0]);
        let silent = TxInputs {
            voice: false,
            ..voice
        };
        assert_eq!(
            ptt_pressed(&s, &silent, &mut mem).radios,
            vec![0],
            "held for VOXMinimumTime"
        );
        mem.last_vox_send = Some(Instant::now() - Duration::from_secs(1));
        assert!(ptt_pressed(&s, &silent, &mut mem).radios.is_empty());
    }

    #[test]
    fn inhibit_blocks_everything() {
        let s = state_with(vec![DcsRadio::no_radio(), am(251e6)], 1);
        let mut mem = TxMemory::new();
        let inputs = TxInputs {
            ptt: true,
            intercom_ptt: true,
            voice: true,
            vox_minimum_time_ms: 300,
            inhibit_tx: true,
        };
        assert!(ptt_pressed(&s, &inputs, &mut mem).radios.is_empty());
    }
}
