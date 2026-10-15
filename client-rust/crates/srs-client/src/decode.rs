//! Decode thread: port of `UDPClientAudioProcessor.UdpAudioDecode`,
//! `HasLineOfSight` and `InRange`. Takes raw voice datagrams, decides which
//! local radio (if any) receives them, and hands the packets to the audio
//! engine. Also ports `CurrentlyBlockedRadios` (IRL_RADIO_TX) and
//! `RetransmitAudio` (relay radios).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

use srs_audio::{ClientAudio, SharedEngine};
use srs_dcs::model::DcsPlayerRadioInfo;
use srs_net::{SharedState, UdpHandle};
use srs_protocol::radio_calc::{
    distance_haversine, friis_maximum_transmission_range, HF_FREQUENCY_LOS_IGNORED_HZ,
};
use srs_protocol::voice_packet::FrequencySegment;
use srs_protocol::voice_packet::MIN_VOICE_PACKET_LEN;
use srs_protocol::{candidate_score, HearResult, Modulation, ServerSettingsKey, ShortGuid, VoicePacket};

use crate::encoder::TxControls;
use crate::radio_state::{ReceivingState, SharedRadioState};

/// Settings read by the decode thread that the UI may change.
pub struct DecodePrefs {
    /// `ShowTransmitterName` client setting.
    pub show_transmitter_name: AtomicBool,
}

struct Candidate {
    hear: HearResult,
    decryptable: bool,
    encryption: u8,
    frequency: f64,
    modulation: Modulation,
    los_loss: f32,
    power_loss: f64,
    volume: f32,
}

/// Everything the decode thread shares with the rest of the client.
pub struct DecodeCtx {
    pub my_guid: ShortGuid,
    pub shared: Arc<SharedState>,
    pub radio_state: SharedRadioState,
    pub engine: SharedEngine,
    pub prefs: Arc<DecodePrefs>,
    pub controls: Arc<TxControls>,
    pub udp: UdpHandle,
}

pub fn run_decode(rx: mpsc::Receiver<Vec<u8>>, ctx: DecodeCtx) {
    let my_guid = ctx.my_guid;
    let shared = &ctx.shared;
    log::info!("UDP Audio Decode - Starting");
    while let Ok(datagram) = rx.recv() {
        if datagram.len() < MIN_VOICE_PACKET_LEN {
            continue;
        }
        // Only decode once the server echoed our own client record and DCS data is current.
        if !shared.contains(my_guid.as_str().as_ref()) {
            continue;
        }
        let packet = match VoicePacket::decode(&datagram) {
            Ok(p) => p,
            Err(e) => {
                log::error!("Unable to decode UDP Voice Packet: {e}");
                continue;
            }
        };
        handle_packet(packet, &ctx);
    }
    log::info!("UDP Audio Decode - Stopping.");
}

fn handle_packet(mut packet: VoicePacket, ctx: &DecodeCtx) {
    let shared = &*ctx.shared;
    let radio_state = &ctx.radio_state;
    let engine = &ctx.engine;
    let prefs = &*ctx.prefs;
    let (
        global_frequencies,
        strict_encryption,
        los_enabled,
        distance_enabled,
        interference,
        effect_override,
        show_name_server,
        irl_radio_tx,
        node_limit,
    ) = {
        let s = shared.server_settings.read().expect("server settings lock");
        (
            s.global_frequencies_hz.clone(),
            s.get_bool(ServerSettingsKey::STRICT_RADIO_ENCRYPTION),
            s.get_bool(ServerSettingsKey::LOS_ENABLED),
            s.get_bool(ServerSettingsKey::DISTANCE_ENABLED),
            s.get_bool(ServerSettingsKey::IRL_RADIO_RX_INTERFERENCE),
            s.get_bool(ServerSettingsKey::RADIO_EFFECT_OVERRIDE),
            s.get_bool(ServerSettingsKey::SHOW_TRANSMITTER_NAME),
            s.get_bool(ServerSettingsKey::IRL_RADIO_TX),
            s.retransmit_node_limit,
        )
    };
    let transmitter = shared.get(packet.client_guid.as_str().as_ref());

    let mut candidates: Vec<Candidate> = Vec::with_capacity(packet.frequencies.len());
    {
        let state = radio_state.lock();
        if !state.dcs.is_current() {
            return;
        }
        let my_pos = state.side.position;
        let ptt = ctx.controls.ptt.load(Ordering::Relaxed) || ctx.controls.ui_ptt.load(Ordering::Relaxed);
        let blocked = currently_blocked_radios(irl_radio_tx, ptt, &state.dcs);
        // HF exemption looks at every frequency of the packet (C# quirk).
        let hf_exempt = packet
            .frequencies
            .iter()
            .any(|f| f.modulation == Modulation::Am.as_u8() && f.freq_hz <= HF_FREQUENCY_LOS_IGNORED_HZ);
        for seg in packet.frequencies.iter_mut() {
            // C# compares with exact double equality here.
            let global = global_frequencies.contains(&seg.freq_hz);
            if global {
                seg.encryption = 0;
            }
            let modulation = Modulation::from_u8(seg.modulation).unwrap_or(Modulation::Disabled);
            let Some(hear) = state.dcs.can_hear_transmission(
                seg.freq_hz,
                modulation,
                seg.encryption,
                strict_encryption,
                packet.unit_id,
                &blocked,
            ) else {
                continue;
            };
            let radio = &state.dcs.radios[hear.radio_index];
            let mut los_loss = 0.0f32;
            let mut power_loss = 0.0f64;
            let exempt = matches!(radio.modulation, Modulation::Intercom | Modulation::Mids) || global;
            let audible = exempt
                || (has_line_of_sight(
                    los_enabled,
                    hf_exempt,
                    transmitter.as_ref(),
                    &my_pos,
                    &mut los_loss,
                ) && in_range(
                    distance_enabled,
                    transmitter.as_ref(),
                    &my_pos,
                    seg.freq_hz,
                    &mut power_loss,
                ) && !blocked.contains(&hear.radio_index));
            if !audible {
                continue;
            }
            candidates.push(Candidate {
                hear,
                decryptable: hear.decryptable,
                encryption: seg.encryption,
                frequency: seg.freq_hz,
                modulation,
                los_loss,
                power_loss,
                volume: srs_audio::volume::radio_volume_slider(radio.volume, false),
            });
        }
        if candidates.is_empty() {
            return;
        }
        let selected = state.dcs.selected_index();
        candidates.sort_by_key(|c| {
            let selected_bonus = selected == Some(c.hear.radio_index);
            std::cmp::Reverse(candidate_score(c.decryptable, selected_bonus, c.volume > 0.0))
        });
    }

    let transmitter_name = match &transmitter {
        Some(t) if show_name_server && prefs.show_transmitter_name.load(Ordering::Relaxed) => {
            t.client.name.clone()
        }
        _ => String::new(),
    };
    let ambient = transmitter
        .as_ref()
        .and_then(|t| t.client.radio_info.as_ref().map(|r| r.ambient.clone()))
        .unwrap_or_default();
    if transmitter.as_ref().is_some_and(|t| t.local.muted) {
        return;
    }

    let now = Instant::now();
    let mut state = radio_state.lock();
    for (i, c) in candidates.iter().enumerate() {
        let is_simultaneous = candidates.len() > 1 && i > 0;
        state.receiving[c.hear.radio_index] = ReceivingState {
            is_secondary: c.hear.is_secondary,
            is_simultaneous,
            last_received_at: Some(now),
            received_on: c.hear.radio_index,
            sent_by: transmitter_name.clone(),
        };
        if i == 0 || interference {
            // `GetRadioModel`: the sender's radio on this frequency/modulation.
            let model_name = transmitter
                .as_ref()
                .and_then(|t| t.client.radio_info.as_ref())
                .and_then(|r| {
                    r.radios.iter().find(|radio| {
                        radio.modulation == c.modulation
                            && srs_protocol::hearing::freq_close_enough(c.frequency, radio.freq)
                    })
                })
                .map(|r| r.model.clone())
                .unwrap_or_default();
            let audio = ClientAudio {
                encoded: packet.audio.clone(),
                client_guid: packet.client_guid,
                received_radio: c.hear.radio_index,
                frequency: c.frequency,
                modulation: c.modulation,
                volume: c.volume,
                unit_id: packet.unit_id,
                encryption: c.encryption,
                decryptable: c.decryptable,
                receiving_power: c.power_loss,
                line_of_sight_loss: c.los_loss,
                packet_number: packet.packet_number,
                original_client_guid: packet.original_client_guid,
                is_secondary: c.hear.is_secondary,
                no_audio_effects: effect_override && global_frequencies.contains(&c.frequency),
                ambient: ambient.clone(),
                model_name,
            };
            engine.lock().add_client_audio(&audio);
        }
    }
    retransmit_audio(
        &packet,
        &candidates,
        &state.dcs,
        &ctx.my_guid,
        node_limit,
        &global_frequencies,
        &ctx.udp,
    );
}

/// `CurrentlyBlockedRadios`: with `IRL_RADIO_TX`, radios I am transmitting
/// on cannot receive (AM/FM/SINCGARS/MIDS/HAVEQUICK only; SATCOM and the
/// intercom never block).
pub fn currently_blocked_radios(irl_radio_tx: bool, ptt: bool, info: &DcsPlayerRadioInfo) -> Vec<usize> {
    let mut blocked = Vec::new();
    if !irl_radio_tx || (!ptt && !info.ptt) {
        return blocked;
    }
    let selected = usize::try_from(info.selected).unwrap_or(0);
    if let Some(current) = info.radios.get(selected) {
        if matches!(
            current.modulation,
            Modulation::Fm | Modulation::Sincgars | Modulation::Am | Modulation::Mids | Modulation::HaveQuick
        ) {
            blocked.push(selected);
        }
    }
    if info.simultaneous_transmission {
        for (i, radio) in info.radios.iter().enumerate().skip(1) {
            if matches!(
                radio.modulation,
                Modulation::Fm | Modulation::Sincgars | Modulation::Am
            ) && radio.simul
                && i != selected
            {
                blocked.push(i);
            }
        }
    }
    blocked
}

/// `RetransmitAudio`: relay a received transmission on my other
/// `retransmit` radios, unless the hop limit is reached.
fn retransmit_audio(
    packet: &VoicePacket,
    candidates: &[Candidate],
    info: &DcsPlayerRadioInfo,
    my_guid: &ShortGuid,
    node_limit: i32,
    global_frequencies: &[f64],
    udp: &UdpHandle,
) {
    if packet.client_guid == *my_guid {
        return;
    }
    if node_limit < i32::from(packet.retransmission_count) {
        return;
    }
    let close = srs_protocol::hearing::freq_close_enough;
    // Radios I received on that have retransmit enabled (not guard, not global).
    let receiving: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| {
            let radio = &info.radios[c.hear.radio_index];
            (c.decryptable || c.encryption == 0)
                && radio.retransmit
                && !global_frequencies.iter().any(|g| close(radio.freq, *g))
                && !c.hear.is_secondary
        })
        .collect();
    if receiving.is_empty() {
        return;
    }
    // My retransmit radios that are not on one of the received frequencies.
    let targets: Vec<&srs_dcs::model::DcsRadio> = info
        .radios
        .iter()
        .filter(|r| r.retransmit)
        .filter(|r| !receiving.iter().any(|c| close(r.freq, c.frequency)))
        .collect();
    if targets.is_empty() {
        return;
    }
    let relayed = VoicePacket {
        audio: packet.audio.clone(),
        frequencies: targets
            .iter()
            .map(|r| FrequencySegment {
                freq_hz: r.freq,
                modulation: r.modulation.as_u8(),
                encryption: if r.enc { r.enc_key } else { 0 },
            })
            .collect(),
        unit_id: info.unit_id,
        packet_number: packet.packet_number,
        retransmission_count: packet.retransmission_count.wrapping_add(1),
        original_client_guid: packet.original_client_guid,
        client_guid: *my_guid,
    };
    let mut bytes = Vec::with_capacity(relayed.encoded_len());
    if relayed.encode_into(&mut bytes).is_ok() {
        udp.send(bytes);
    }
}

/// `HasLineOfSight`: AM at or below 30 MHz always passes (HF skywave).
fn has_line_of_sight(
    los_enabled: bool,
    hf_exempt: bool,
    transmitter: Option<&srs_net::shared::ConnectedClient>,
    my_pos: &srs_protocol::LatLng,
    los_loss: &mut f32,
) -> bool {
    *los_loss = 0.0;
    if !los_enabled {
        return true;
    }
    if hf_exempt {
        return true;
    }
    match transmitter {
        Some(t) => {
            let Some(their_pos) = t.client.lat_lng_position else {
                return true;
            };
            if !my_pos.is_valid() || !their_pos.is_valid() {
                return true;
            }
            *los_loss = t.local.line_of_sight_loss;
            t.local.line_of_sight_loss < 1.0
        }
        None => false,
    }
}

/// `InRange`: Haversine + altitude vs the Friis maximum range.
fn in_range(
    distance_enabled: bool,
    transmitter: Option<&srs_net::shared::ConnectedClient>,
    my_pos: &srs_protocol::LatLng,
    frequency: f64,
    signal_strength: &mut f64,
) -> bool {
    *signal_strength = 0.0;
    if !distance_enabled {
        return true;
    }
    match transmitter {
        Some(t) => {
            let Some(their_pos) = t.client.lat_lng_position else {
                return true;
            };
            if !my_pos.is_valid() || !their_pos.is_valid() {
                return true;
            }
            let dist = distance_haversine(my_pos, &their_pos);
            let max = friis_maximum_transmission_range(frequency);
            *signal_strength = dist / max;
            max > dist
        }
        None => false,
    }
}
