//! Per-remote-client receive chain: port of
//! `Common/Audio/Providers/ClientAudioProvider.cs` (+ `AudioProvider.cs`).
//!
//! `add_samples` (decode thread): Opus decode at 48 kHz, reset the decoder
//! after 400 ms of silence, push into the jitter buffer of the receiving
//! radio. `read` (audio thread): drain the jitter buffer, Speex AGC/denoise,
//! then LOS/distance attenuation + cockpit ambient, or the encryption
//! failure noise, then the transmission effects pipeline
//! (`ClientTransmissionPipelineProvider` → [`TxPipeline`]).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use srs_protocol::constants::{MAX_RADIOS, OUTPUT_SAMPLE_RATE};
use srs_protocol::{Ambient, Modulation, ShortGuid};

use crate::effects::{EffectCache, EffectSettings, RadioModelFactory, TxInput, TxPipeline};
use crate::jitter_iface::{JitterAudio, JitterInterface, TransmissionMeta, JITTER_MS};
use crate::opus::OpusDecoder48k;
use crate::speex::SpeexPreprocessor;

/// Decoded-packet descriptor built by the decode thread (`ClientAudio`).
#[derive(Debug, Clone)]
pub struct ClientAudio {
    pub encoded: Vec<u8>,
    pub client_guid: ShortGuid,
    pub received_radio: usize,
    pub frequency: f64,
    pub modulation: Modulation,
    /// Linear gain already converted from the radio volume slider.
    pub volume: f32,
    pub unit_id: u32,
    pub encryption: u8,
    pub decryptable: bool,
    /// 0 = no loss, >= 1 = total loss (distance).
    pub receiving_power: f64,
    /// 0 = no loss, >= 1 = total loss (terrain).
    pub line_of_sight_loss: f32,
    pub packet_number: u64,
    pub original_client_guid: ShortGuid,
    pub is_secondary: bool,
    pub no_audio_effects: bool,
    pub ambient: Ambient,
    /// Sender's radio `Model` for the per-radio model effect ("" = default).
    pub model_name: String,
}

/// Settings polled every 3 s by the C# code (`ReLoadSettings`).
#[derive(Debug, Clone, PartialEq)]
pub struct IncomingAudioSettings {
    pub agc: bool,
    pub agc_target: i32,
    pub agc_decrement: i32,
    pub agc_max_gain: i32,
    pub denoise: bool,
    pub denoise_attenuation: i32,
    pub ambient_cockpit_effect: bool,
    /// Already converted with `radio_volume_slider(.., true)`.
    pub ambient_cockpit_effect_volume: f32,
    pub ambient_cockpit_intercom_effect: bool,
}

impl Default for IncomingAudioSettings {
    fn default() -> Self {
        Self {
            agc: true,
            agc_target: 14_000,
            agc_decrement: -60,
            agc_max_gain: 40,
            denoise: true,
            denoise_attenuation: -30,
            ambient_cockpit_effect: true,
            ambient_cockpit_effect_volume: 1.0,
            ambient_cockpit_intercom_effect: false,
        }
    }
}

/// 10 ms at 48 kHz: the incoming Speex frame.
const SPEEX_FRAME: usize = (OUTPUT_SAMPLE_RATE / 100) as usize;

pub struct RemoteClientAudio {
    decoder: OpusDecoder48k,
    last_update: Option<Instant>,
    jitter: Vec<JitterInterface>,
    preprocess: SpeexPreprocessor,
    applied: IncomingAudioSettings,
    decode_buf: Vec<f32>,
    speex_frame: [i16; SPEEX_FRAME],
    rng: u64,
    cache: Arc<EffectCache>,
    pipeline: TxPipeline,
    /// `ambientEffectProgress`: per receiving radio, per ambient type.
    ambient_progress: Vec<HashMap<String, usize>>,
}

impl RemoteClientAudio {
    pub fn new(
        cache: Arc<EffectCache>,
        factory: Arc<RadioModelFactory>,
    ) -> Result<Self, crate::opus::OpusError> {
        let mut preprocess = SpeexPreprocessor::new(SPEEX_FRAME, OUTPUT_SAMPLE_RATE);
        let applied = IncomingAudioSettings::default();
        apply_settings(&mut preprocess, &applied);
        let seed = Instant::now().elapsed().as_nanos() as u64 ^ (&preprocess as *const _ as u64);
        Ok(Self {
            decoder: OpusDecoder48k::new()?,
            last_update: None,
            jitter: (0..MAX_RADIOS).map(|_| JitterInterface::new()).collect(),
            preprocess,
            applied,
            decode_buf: vec![0.0; OpusDecoder48k::MAX_FRAME_SAMPLES],
            speex_frame: [0; SPEEX_FRAME],
            rng: 0x9E37_79B9_7F4A_7C15 ^ seed,
            pipeline: TxPipeline::new(factory, &cache, seed),
            cache,
            ambient_progress: (0..MAX_RADIOS).map(|_| HashMap::new()).collect(),
        })
    }

    /// Push the current settings (cheap when unchanged).
    pub fn set_settings(&mut self, settings: &IncomingAudioSettings) {
        if *settings != self.applied {
            apply_settings(&mut self.preprocess, settings);
            self.applied = settings.clone();
        }
    }

    /// `LikelyNewTransmission`: more than `JITTER_MS` since the last packet.
    fn likely_new_transmission(&self) -> bool {
        match self.last_update {
            None => true,
            Some(t) => t.elapsed() > Duration::from_millis(JITTER_MS),
        }
    }

    /// `AddClientAudioSamples`: returns the number of decoded samples (0 on
    /// decode failure).
    pub fn add_samples(&mut self, audio: &ClientAudio) -> usize {
        if self.likely_new_transmission() {
            let _ = self.decoder.reset();
        }
        let decoded = match self.decoder.decode_float(&audio.encoded, &mut self.decode_buf) {
            Ok(n) if n > 0 => n,
            Ok(_) => {
                log::info!("Failed to decode audio from Packet for client");
                return 0;
            }
            Err(e) => {
                log::warn!("Error decoding audio packet: {e}");
                return 0;
            }
        };
        self.last_update = Some(Instant::now());
        let radio = audio.received_radio.min(MAX_RADIOS - 1);
        self.jitter[radio].add_samples(JitterAudio {
            audio: self.decode_buf[..decoded].to_vec(),
            packet_number: audio.packet_number,
            received_radio: radio,
            modulation: audio.modulation,
            decryptable: audio.decryptable,
            volume: audio.volume,
            is_secondary: audio.is_secondary,
            frequency: audio.frequency,
            no_audio_effects: audio.no_audio_effects,
            guid: audio.client_guid,
            original_client_guid: audio.original_client_guid,
            encryption: audio.encryption,
            receiving_power: audio.receiving_power,
            line_of_sight_loss: audio.line_of_sight_loss,
            ambient: audio.ambient.clone(),
            model_name: audio.model_name.clone(),
        });
        decoded
    }

    /// Any audio queued for this radio?
    pub fn available(&self, radio: usize) -> bool {
        self.jitter.get(radio).is_some_and(JitterInterface::available)
    }

    /// `Read(radioId, desired)`: fill `out` with processed mono audio for
    /// that radio. Returns the sample count (0 = nothing to play) and the
    /// metadata of the transmission.
    pub fn read(
        &mut self,
        effects: &EffectSettings,
        radio: usize,
        out: &mut [f32],
    ) -> (usize, TransmissionMeta) {
        let Some(j) = self.jitter.get_mut(radio) else {
            return (0, TransmissionMeta::default());
        };
        let (n, meta) = j.read(out);
        let meta = meta.clone();
        if n == 0 {
            return (0, meta);
        }
        let audio = &mut out[..n];
        self.speex_in_place(audio);
        if meta.decryptable {
            adjust_volume_for_loss(
                meta.modulation,
                meta.receiving_power,
                meta.line_of_sight_loss,
                audio,
            );
            self.add_cockpit_ambient(meta.received_radio, meta.modulation, &meta.ambient, audio);
        } else {
            self.encryption_failure_noise(audio);
        }
        let input = TxInput {
            modulation: meta.modulation,
            frequency: meta.frequency,
            encryption: meta.encryption,
            no_audio_effects: meta.no_audio_effects,
            volume: meta.volume,
            model_name: &meta.model_name,
        };
        self.pipeline.process(effects, &input, audio);
        (n, meta)
    }

    /// `AddCockpitAmbientAudio`: loop the aircraft's ambient WAV under the
    /// voice, scaled to the DCS `vol` against the file's RMS; samples that
    /// would leave [-1, 1] keep the original voice sample.
    fn add_cockpit_ambient(
        &mut self,
        radio: usize,
        modulation: Modulation,
        ambient: &Ambient,
        audio: &mut [f32],
    ) {
        let s = &self.applied;
        if !s.ambient_cockpit_effect
            || (modulation == Modulation::Intercom && !s.ambient_cockpit_intercom_effect)
            || modulation == Modulation::Mids
        {
            return;
        }
        if ambient.ab_type.is_empty() {
            return;
        }
        let Some(effect) = self.cache.ambient(&ambient.ab_type).filter(|e| e.loaded()) else {
            return;
        };
        let vol = crate::volume::decibels_to_linear(crate::volume::target_db(
            effect.rms_db,
            f64::from(ambient.vol),
        )) as f32
            * s.ambient_cockpit_effect_volume;
        let Some(progress_map) = self.ambient_progress.get_mut(radio) else {
            return;
        };
        let progress = match progress_map.get_mut(&ambient.ab_type) {
            Some(p) => p,
            None => progress_map.entry(ambient.ab_type.clone()).or_insert(0),
        };
        let samples = &effect.samples;
        if *progress >= samples.len() {
            *progress = 0;
        }
        for s in audio.iter_mut() {
            let mixed = *s + samples[*progress] * vol;
            if mixed > -1.0 && mixed < 1.0 {
                *s = mixed;
            }
            *progress += 1;
            if *progress >= samples.len() {
                *progress = 0;
            }
        }
    }

    /// `SpeexPreprocessorProvider.Read`: whole 10 ms frames only.
    fn speex_in_place(&mut self, audio: &mut [f32]) {
        let frames = audio.len() / SPEEX_FRAME;
        for f in 0..frames {
            let slice = &mut audio[f * SPEEX_FRAME..(f + 1) * SPEEX_FRAME];
            for (dst, &src) in self.speex_frame.iter_mut().zip(slice.iter()) {
                *dst = (src.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            }
            self.preprocess.process(&mut self.speex_frame);
            for (dst, &src) in slice.iter_mut().zip(self.speex_frame.iter()) {
                *dst = f32::from(src) / (i16::MAX as f32 + 1.0);
            }
        }
    }

    /// `AddEncryptionFailureEffect`: uniform noise in [-1/8, 1/8).
    fn encryption_failure_noise(&mut self, audio: &mut [f32]) {
        for s in audio {
            // xorshift64*
            self.rng ^= self.rng >> 12;
            self.rng ^= self.rng << 25;
            self.rng ^= self.rng >> 27;
            let r = (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 48) as i32; // 0..65535
            let v = (r - 32768) / 8; // -4096..4095
            *s = (v as f32 / 32768.0).clamp(-1.0, 1.0);
        }
    }
}

fn apply_settings(p: &mut SpeexPreprocessor, s: &IncomingAudioSettings) {
    p.set_agc(s.agc);
    p.set_agc_target(s.agc_target);
    p.set_agc_decrement(s.agc_decrement);
    p.set_agc_max_gain(s.agc_max_gain);
    p.set_denoise(s.denoise);
    p.set_noise_suppress(s.denoise_attenuation);
}

/// `AdjustVolumeForLoss`: distance loss only beyond 85 % of the max range,
/// LOS loss whenever > 0. MIDS, SATCOM and INTERCOM are exempt.
pub fn adjust_volume_for_loss(
    modulation: Modulation,
    receiving_power: f64,
    los_loss: f32,
    audio: &mut [f32],
) {
    if matches!(
        modulation,
        Modulation::Mids | Modulation::Satcom | Modulation::Intercom
    ) {
        return;
    }
    let mut factor = 1.0f32;
    if receiving_power > 0.85 {
        factor *= 1.0 - receiving_power as f32;
    }
    if los_loss > 0.0 {
        factor *= 1.0 - los_loss;
    }
    if factor != 1.0 {
        for s in audio {
            *s *= factor;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opus::OpusEncoder16k;
    use srs_protocol::constants::{MAX_OPUS_FRAME_BYTES, MIC_SEGMENT_FRAMES, OUTPUT_SEGMENT_FRAMES};

    fn remote() -> RemoteClientAudio {
        RemoteClientAudio::new(
            Arc::new(EffectCache::default()),
            Arc::new(RadioModelFactory::empty()),
        )
        .unwrap()
    }

    fn no_effects() -> EffectSettings {
        EffectSettings {
            radio_effects_ratio: 0.0,
            ..Default::default()
        }
    }

    fn packet(enc: &mut OpusEncoder16k, n: u64, decryptable: bool) -> ClientAudio {
        let mut pcm = [0i16; MIC_SEGMENT_FRAMES];
        for (i, s) in pcm.iter_mut().enumerate() {
            *s = ((i as f32 * 2.0 * std::f32::consts::PI * 440.0 / 16000.0).sin() * 12000.0) as i16;
        }
        let mut buf = [0u8; MAX_OPUS_FRAME_BYTES];
        let len = enc.encode(&pcm, &mut buf).unwrap();
        ClientAudio {
            encoded: buf[..len].to_vec(),
            client_guid: ShortGuid::new_random(),
            received_radio: 1,
            frequency: 251e6,
            modulation: Modulation::Am,
            volume: 1.0,
            unit_id: 1,
            encryption: 0,
            decryptable,
            receiving_power: 0.0,
            line_of_sight_loss: 0.0,
            packet_number: n,
            original_client_guid: ShortGuid::new_random(),
            is_secondary: false,
            no_audio_effects: false,
            ambient: Ambient::default(),
            model_name: String::new(),
        }
    }

    #[test]
    fn decode_jitter_read_chain() {
        let mut enc = OpusEncoder16k::new().unwrap();
        let mut rc = remote();
        let fx = no_effects();
        for n in 1..=3 {
            assert_eq!(rc.add_samples(&packet(&mut enc, n, true)), OUTPUT_SEGMENT_FRAMES);
        }
        assert!(rc.available(1));
        assert!(!rc.available(2));
        let mut out = vec![0f32; OUTPUT_SEGMENT_FRAMES];
        let (n, meta) = rc.read(&fx, 1, &mut out);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES);
        assert_eq!(meta.received_radio, 1);
        assert!(meta.decryptable);
        // Opus priming: the first frame is mostly silent, the second one is not.
        let (n, _) = rc.read(&fx, 1, &mut out);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES);
        let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "peak {peak}");
        let (n, _) = rc.read(&fx, 2, &mut out);
        assert_eq!(n, 0);
    }

    #[test]
    fn full_effects_chain_runs() {
        let mut enc = OpusEncoder16k::new().unwrap();
        let mut rc = remote();
        let fx = EffectSettings::default();
        for n in 1..=3 {
            rc.add_samples(&packet(&mut enc, n, true));
        }
        let mut out = vec![0f32; OUTPUT_SEGMENT_FRAMES];
        rc.read(&fx, 1, &mut out);
        let (n, _) = rc.read(&fx, 1, &mut out);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES);
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(out.iter().any(|s| s.abs() > 0.01));
    }

    #[test]
    fn undecryptable_is_noise() {
        let mut enc = OpusEncoder16k::new().unwrap();
        let mut rc = remote();
        rc.add_samples(&packet(&mut enc, 1, false));
        let mut out = vec![0f32; OUTPUT_SEGMENT_FRAMES];
        let (n, meta) = rc.read(&no_effects(), 1, &mut out);
        assert_eq!(n, OUTPUT_SEGMENT_FRAMES);
        assert!(!meta.decryptable);
        assert!(out.iter().all(|s| s.abs() <= 0.125));
        assert!(out.iter().any(|s| s.abs() > 0.01));
    }

    #[test]
    fn cockpit_ambient_is_mixed_in() {
        let mut cache = EffectCache::default();
        // RMS 0.5 → −6 dB → `GetTargetdB(rms, 1.0)` = −6 dB → ×0.5 (that is what the C# does).
        cache.insert_ambient_for_test("uh1", vec![0.5; 4800]);
        let mut rc = RemoteClientAudio::new(Arc::new(cache), Arc::new(RadioModelFactory::empty())).unwrap();
        let mut a = vec![0f32; 960];
        let amb = Ambient {
            ab_type: "UH1".into(),
            vol: 1.0,
        };
        rc.add_cockpit_ambient(1, Modulation::Am, &amb, &mut a);
        assert!(a.iter().all(|s| (*s - 0.25).abs() < 1e-5), "{}", a[0]);
        let mut b = vec![0f32; 960];
        rc.add_cockpit_ambient(1, Modulation::Intercom, &amb, &mut b);
        assert!(b.iter().all(|s| *s == 0.0), "intercom opt-in");
        let mut c = vec![0f32; 960];
        rc.add_cockpit_ambient(
            1,
            Modulation::Am,
            &Ambient {
                ab_type: "unknown".into(),
                vol: 1.0,
            },
            &mut c,
        );
        assert!(c.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn loss_attenuation() {
        let mut a = vec![1.0f32; 4];
        adjust_volume_for_loss(Modulation::Am, 0.9, 0.5, &mut a);
        assert!((a[0] - 0.05).abs() < 1e-6);
        let mut b = vec![1.0f32; 4];
        adjust_volume_for_loss(Modulation::Am, 0.5, 0.0, &mut b);
        assert_eq!(b[0], 1.0, "below 85 % no distance loss");
        let mut c = vec![1.0f32; 4];
        adjust_volume_for_loss(Modulation::Intercom, 0.99, 0.9, &mut c);
        assert_eq!(c[0], 1.0);
    }
}
