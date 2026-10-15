//! Port of `Common/Audio/Providers/JitterBufferProviderInterface.cs`: one
//! jitter buffer per (remote client, local radio), fed with decoded 48 kHz
//! mono frames and drained in fixed-size reads.
//!
//! Timestamp = `packet_number * samples_in_packet`, span = samples in packet.
//! A `Missing` packet (lost) yields nothing (the read stops short → silence
//! is filled by the caller), an `Insertion` yields silence for its span.

use srs_protocol::constants::OUTPUT_SEGMENT_FRAMES;
use srs_protocol::{Ambient, Modulation, ShortGuid};

use crate::jitter::{JitterBuffer, Packet, Status};

/// `JITTER_MS`: also the decoder reset threshold.
pub const JITTER_MS: u64 = 400;

/// Decoded audio plus the metadata that travels with it (`JitterBufferAudio`).
#[derive(Debug, Clone)]
pub struct JitterAudio {
    pub audio: Vec<f32>,
    pub packet_number: u64,
    pub received_radio: usize,
    pub modulation: Modulation,
    pub decryptable: bool,
    pub volume: f32,
    pub is_secondary: bool,
    pub frequency: f64,
    pub no_audio_effects: bool,
    pub guid: ShortGuid,
    pub original_client_guid: ShortGuid,
    pub encryption: u8,
    pub receiving_power: f64,
    pub line_of_sight_loss: f32,
    pub ambient: Ambient,
    /// Sender's radio `Model` (resolved by the decode thread, "" = default).
    pub model_name: String,
}

/// Metadata of the last packet read (`DeJitteredTransmission` without the
/// PCM, which the caller owns).
#[derive(Debug, Clone, Default)]
pub struct TransmissionMeta {
    pub received_radio: usize,
    pub modulation: Modulation,
    pub decryptable: bool,
    pub encryption: u8,
    pub volume: f32,
    pub is_secondary: bool,
    pub frequency: f64,
    pub no_audio_effects: bool,
    pub guid: Option<ShortGuid>,
    pub original_client_guid: Option<ShortGuid>,
    pub receiving_power: f64,
    pub line_of_sight_loss: f32,
    pub ambient: Ambient,
    pub model_name: String,
}

pub struct JitterInterface {
    jitter: JitterBuffer<JitterAudio>,
    /// Packet currently being drained (`_packet`).
    current: Option<Packet<JitterAudio>>,
    last: TransmissionMeta,
}

impl Default for JitterInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl JitterInterface {
    pub fn new() -> Self {
        Self {
            jitter: JitterBuffer::new(OUTPUT_SEGMENT_FRAMES as i64),
            current: None,
            last: TransmissionMeta::default(),
        }
    }

    /// `AddSamples`.
    pub fn add_samples(&mut self, audio: JitterAudio) {
        let span = audio.audio.len() as i64;
        let timestamp = audio.packet_number as i64 * span;
        self.jitter.put(Packet {
            data: Some(audio),
            timestamp,
            span,
        });
    }

    fn hydrate(&mut self) -> bool {
        while self.current.is_none() && self.jitter.available() {
            let (status, packet) = self.jitter.get(OUTPUT_SEGMENT_FRAMES as i64);
            match status {
                Status::Ok => {
                    if let Some(d) = &packet.data {
                        self.last = TransmissionMeta {
                            received_radio: d.received_radio,
                            modulation: d.modulation,
                            decryptable: d.decryptable,
                            encryption: d.encryption,
                            volume: d.volume,
                            is_secondary: d.is_secondary,
                            frequency: d.frequency,
                            no_audio_effects: d.no_audio_effects,
                            guid: Some(d.guid),
                            original_client_guid: Some(d.original_client_guid),
                            receiving_power: d.receiving_power,
                            line_of_sight_loss: d.line_of_sight_loss,
                            ambient: d.ambient.clone(),
                            model_name: d.model_name.clone(),
                        };
                    }
                    self.current = Some(packet);
                }
                Status::Missing => self.current = None,
                Status::Insertion => self.current = Some(packet),
            }
        }
        self.current.is_some()
    }

    fn read_from_current(&mut self, buffer: &mut [f32]) -> usize {
        let Some(packet) = &mut self.current else { return 0 };
        let from_buffer = (buffer.len() as i64).min(packet.span) as usize;
        if from_buffer == 0 {
            return 0;
        }
        match &packet.data {
            Some(d) => {
                let start = d.audio.len() - packet.span as usize;
                buffer[..from_buffer].copy_from_slice(&d.audio[start..start + from_buffer]);
            }
            None => {
                // Insertion: silence.
                buffer[..from_buffer].iter_mut().for_each(|s| *s = 0.0);
            }
        }
        packet.span -= from_buffer as i64;
        if packet.span == 0 {
            self.current = None;
        }
        from_buffer
    }

    /// `Read(count)`: fill `out` (cleared first) and return how many samples
    /// were produced, with the metadata of the transmission being played.
    /// Fewer than `out.len()` means the buffer starved.
    pub fn read(&mut self, out: &mut [f32]) -> (usize, &TransmissionMeta) {
        out.iter_mut().for_each(|s| *s = 0.0);
        let mut read = 0;
        while read < out.len() {
            read += self.read_from_current(&mut out[read..]);
            if !self.hydrate() {
                break;
            }
        }
        // Tick.
        let remaining = self.current.as_ref().map(|p| p.span).unwrap_or(0);
        self.jitter.remaining_span(remaining);
        self.jitter.tick();
        (read, &self.last)
    }

    pub fn available(&self) -> bool {
        self.current.is_some() || self.jitter.available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(n: u64, value: f32) -> JitterAudio {
        JitterAudio {
            audio: vec![value; OUTPUT_SEGMENT_FRAMES],
            packet_number: n,
            received_radio: 1,
            modulation: Modulation::Am,
            decryptable: true,
            volume: 1.0,
            is_secondary: false,
            frequency: 251e6,
            no_audio_effects: false,
            guid: ShortGuid::new_random(),
            original_client_guid: ShortGuid::new_random(),
            encryption: 0,
            receiving_power: 0.0,
            line_of_sight_loss: 0.0,
            ambient: Ambient::default(),
            model_name: String::new(),
        }
    }

    #[test]
    fn reads_whole_frames_in_order_and_starves_cleanly() {
        let mut j = JitterInterface::new();
        j.add_samples(frame(1, 0.1));
        j.add_samples(frame(2, 0.2));
        let mut out = vec![0f32; OUTPUT_SEGMENT_FRAMES];
        let (n, meta) = j.read(&mut out);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES);
        assert_eq!(out[0], 0.1);
        assert_eq!(meta.received_radio, 1);
        let (n, _) = j.read(&mut out);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES);
        assert_eq!(out[0], 0.2);
        let (n, _) = j.read(&mut out);
        assert_eq!(n, 0, "starved");
        assert!(!j.available());
    }

    #[test]
    fn partial_reads_split_a_packet() {
        let mut j = JitterInterface::new();
        j.add_samples(frame(1, 0.5));
        let mut half = vec![0f32; OUTPUT_SEGMENT_FRAMES / 2];
        let (n, _) = j.read(&mut half);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES / 2);
        let (n, _) = j.read(&mut half);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES / 2);
        let (n, _) = j.read(&mut half);
        assert_eq!(n, 0);
    }
}
