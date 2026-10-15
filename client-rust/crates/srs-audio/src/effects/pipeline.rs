//! Per-remote-client transmission effects: port of
//! `Common/Audio/Providers/ClientTransmissionPipelineProvider.cs`.
//!
//! Order (wet path, ordinary radios): clipping (±4000/32768) → background
//! noise (+ NATO/HAVEQUICK tone) → radio model TX effect (+ encryption
//! effect) → clip ±1. Intercom-like modulations (INTERCOM, SATCOM, MIDS)
//! only get the intercom model. Output = dry·(1−ratio) + wet·ratio, then
//! the radio volume. `NoAudioEffects` transmissions are returned untouched
//! (the C# code also skips the volume in that case).

use std::collections::HashMap;
use std::sync::Arc;

use srs_protocol::constants::OUTPUT_SAMPLE_RATE;
use srs_protocol::Modulation;

use super::cache::EffectCache;
use super::dsp::{self, Filter, FirstOrder, PinkNoise, WhiteNoise};
use super::model::{RadioModelFactory, TxModel};
use super::tones::LoopingTone;
use super::EffectSettings;

/// `RadioFilter.CLIPPING_MAX`.
pub const CLIPPING_MAX: f32 = 4000.0 / 32768.0;
/// Below (inclusive) → pink "HF" noise, above → white noise.
pub const HF_NOISE_CUTOFF_HZ: f64 = 25e6;

/// What the pipeline needs to know about one transmission segment.
#[derive(Debug, Clone)]
pub struct TxInput<'a> {
    pub modulation: Modulation,
    pub frequency: f64,
    pub encryption: u8,
    pub no_audio_effects: bool,
    /// Linear radio volume.
    pub volume: f32,
    /// Sender's radio `Model` (empty = default ARC-210).
    pub model_name: &'a str,
}

pub struct TxPipeline {
    factory: Arc<RadioModelFactory>,
    models: HashMap<String, TxModel>,
    /// `None` = not looked up yet, `Some(None)` = no `hfnoise` model.
    hf_noise: Option<Option<TxModel>>,
    nato: LoopingTone,
    hq: LoopingTone,
    white: WhiteNoise,
    pink: PinkNoise,
    noise_lp: Vec<Box<dyn Filter>>,
    wet: Vec<f32>,
    noise: Vec<f32>,
    tone: Vec<f32>,
}

fn is_intercom_like(m: Modulation) -> bool {
    matches!(m, Modulation::Mids | Modulation::Satcom | Modulation::Intercom)
}

impl TxPipeline {
    pub fn new(factory: Arc<RadioModelFactory>, cache: &EffectCache, seed: u64) -> Self {
        Self {
            factory,
            models: HashMap::new(),
            hf_noise: None,
            nato: LoopingTone::new(&cache.nato_tone),
            hq: LoopingTone::new(&cache.havequick_tone),
            white: WhiteNoise::new(seed),
            pink: PinkNoise::new(seed ^ 0x5DEE_CE66),
            noise_lp: vec![Box::new(FirstOrder::low_pass(
                f64::from(OUTPUT_SAMPLE_RATE),
                800.0,
            ))],
            wet: Vec::new(),
            noise: Vec::new(),
            tone: Vec::new(),
        }
    }

    /// Drop the cached models (the C# code does this after an exception).
    pub fn reset_models(&mut self) {
        self.models.clear();
        self.hf_noise = None;
    }

    /// `Process`: in place.
    pub fn process(&mut self, s: &EffectSettings, t: &TxInput<'_>, audio: &mut [f32]) {
        if t.no_audio_effects {
            return;
        }
        let ratio = s.radio_effects_ratio.clamp(0.0, 1.0);
        self.wet.clear();
        self.wet.extend_from_slice(audio);
        let encrypted = s.radio_encryption_effects && (t.modulation == Modulation::Mids || t.encryption > 0);
        if ratio > 0.0 {
            let Self {
                factory,
                models,
                hf_noise,
                nato,
                hq,
                white,
                pink,
                noise_lp,
                wet,
                noise,
                tone,
            } = self;
            if is_intercom_like(t.modulation) {
                let model = models
                    .entry("intercom".to_string())
                    .or_insert_with(|| factory.tx_or_default_intercom("intercom"));
                model.process(wet, encrypted);
            } else {
                let name = if s.per_radio_model_effects {
                    t.model_name
                } else {
                    ""
                };
                let model = match models.get_mut(name) {
                    Some(m) => m,
                    None => models
                        .entry(name.to_string())
                        .or_insert_with(|| factory.tx_or_default_radio(name)),
                };
                // BuildRadioEffectsChain
                if s.clipping {
                    dsp::clip(wet, -CLIPPING_MAX, CLIPPING_MAX);
                }
                if s.background_noise {
                    let hf = hf_noise.get_or_insert_with(|| factory.tx("hfnoise"));
                    background_noise(
                        s,
                        t,
                        model.noise_gain,
                        hf,
                        encrypted,
                        white,
                        pink,
                        noise_lp,
                        nato,
                        hq,
                        noise,
                        tone,
                        wet,
                    );
                }
                model.process(wet, encrypted);
                dsp::clip(wet, -1.0, 1.0);
            }
        }
        let dry_gain = (1.0 - ratio).max(0.0);
        for (a, w) in audio.iter_mut().zip(&self.wet) {
            *a = (*a * dry_gain + *w * ratio) * t.volume;
        }
    }
}

/// `BuildBackgroundNoiseEffect`: adds noise (and the modulation's tone) to `voice`.
#[allow(clippy::too_many_arguments)]
fn background_noise(
    s: &EffectSettings,
    t: &TxInput<'_>,
    model_noise_gain: f32,
    hf_noise: &mut Option<TxModel>,
    encrypted: bool,
    white: &mut WhiteNoise,
    pink: &mut PinkNoise,
    noise_lp: &mut [Box<dyn Filter>],
    nato: &mut LoopingTone,
    hq: &mut LoopingTone,
    noise: &mut Vec<f32>,
    tone: &mut Vec<f32>,
    voice: &mut [f32],
) {
    let is_hf = t.frequency <= HF_NOISE_CUTOFF_HZ;
    let mut noise_gain_db = -(t.frequency / 1e6).ln() * 10.0 / 2.0;
    noise_gain_db += f64::from(if is_hf {
        s.hf_noise_gain_db
    } else {
        s.noise_gain_db
    });
    noise_gain_db += f64::from(model_noise_gain);
    let generator_gain_db = if is_hf { 0.0 } else { noise_gain_db };

    noise.resize(voice.len(), 0.0);
    if is_hf {
        pink.fill(noise, 1.0);
    } else {
        white.fill(noise);
        dsp::gain(noise, dsp::db_to_linear(generator_gain_db) as f32);
    }
    dsp::apply_filters(noise_lp, noise);
    if is_hf {
        if let Some(m) = hf_noise {
            m.process(noise, encrypted);
        }
        dsp::gain(noise, dsp::db_to_linear(noise_gain_db) as f32);
    }
    for (v, n) in voice.iter_mut().zip(noise.iter()) {
        *v += *n;
    }

    let (tone_src, enabled, volume) = match t.modulation {
        Modulation::Fm | Modulation::Sincgars => (nato, s.nato_tone, s.nato_tone_volume),
        Modulation::HaveQuick => (hq, s.havequick_tone, s.hq_tone_volume),
        _ => return,
    };
    if enabled && tone_src.loaded() {
        tone.resize(voice.len(), 0.0);
        tone_src.read(tone);
        for (v, n) in voice.iter_mut().zip(tone.iter()) {
            *v += *n * volume;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::cache::CachedEffect;

    fn pipeline() -> TxPipeline {
        let mut cache = EffectCache::default();
        // 1 kHz tone (a DC "tone" would be removed by the model's high-pass).
        let tone: Vec<f32> = (0..480)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI / 48.0).sin() * 0.2)
            .collect();
        cache.nato_tone = CachedEffect {
            file_name: "NATO_TONE.wav".into(),
            samples: Arc::new(tone),
            rms_db: -17.0,
        };
        TxPipeline::new(Arc::new(RadioModelFactory::empty()), &cache, 42)
    }

    fn voice() -> Vec<f32> {
        (0..1920).map(|i| (i as f32 * 0.07).sin() * 0.3).collect()
    }

    fn input(modulation: Modulation, frequency: f64) -> TxInput<'static> {
        TxInput {
            modulation,
            frequency,
            encryption: 0,
            no_audio_effects: false,
            volume: 1.0,
            model_name: "",
        }
    }

    #[test]
    fn no_effects_passthrough_ignores_volume() {
        let mut p = pipeline();
        let mut a = voice();
        let orig = a.clone();
        let mut t = input(Modulation::Am, 251e6);
        t.no_audio_effects = true;
        t.volume = 0.5;
        p.process(&EffectSettings::default(), &t, &mut a);
        assert_eq!(a, orig);
    }

    #[test]
    fn ratio_zero_is_dry_times_volume() {
        let mut p = pipeline();
        let mut a = voice();
        let orig = a.clone();
        let mut t = input(Modulation::Am, 251e6);
        t.volume = 0.5;
        let s = EffectSettings {
            radio_effects_ratio: 0.0,
            ..Default::default()
        };
        p.process(&s, &t, &mut a);
        for (x, o) in a.iter().zip(&orig) {
            assert!((x - o * 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn uhf_adds_noise_and_fm_adds_tone() {
        let mut p = pipeline();
        let s = EffectSettings::default();
        let mut silence = vec![0f32; 1920];
        p.process(&s, &input(Modulation::Am, 251e6), &mut silence);
        let rms = (silence.iter().map(|x| x * x).sum::<f32>() / 1920.0).sqrt();
        assert!(rms > 1e-4 && rms < 0.2, "noise rms {rms}");
        let mut fm = vec![0f32; 1920];
        let mut no_tone = vec![0f32; 1920];
        p.process(&s, &input(Modulation::Fm, 30e6), &mut fm);
        let s2 = EffectSettings {
            nato_tone: false,
            ..Default::default()
        };
        p.process(&s2, &input(Modulation::Fm, 30e6), &mut no_tone);
        let e = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>();
        assert!(e(&fm) > e(&no_tone) * 1.5, "{} vs {}", e(&fm), e(&no_tone));
    }

    #[test]
    fn hf_uses_pink_noise_and_intercom_skips_noise() {
        let mut p = pipeline();
        let s = EffectSettings::default();
        let mut hf = vec![0f32; 1920];
        p.process(&s, &input(Modulation::Am, 5e6), &mut hf);
        assert!(hf.iter().any(|x| x.abs() > 1e-5));
        assert!(hf.iter().all(|x| x.abs() <= 1.0));
        let mut ic = vec![0f32; 1920];
        p.process(&s, &input(Modulation::Intercom, 100.0), &mut ic);
        assert!(ic.iter().all(|x| *x == 0.0), "intercom silence stays silent");
    }

    #[test]
    fn encryption_effect_applies_cvsd() {
        let mut p = pipeline();
        let s = EffectSettings {
            background_noise: false,
            ..Default::default()
        };
        let mut a = voice();
        let mut b = voice();
        let mut t = input(Modulation::Am, 251e6);
        p.process(&s, &t, &mut a);
        t.encryption = 1;
        let mut p2 = pipeline();
        p2.process(&s, &t, &mut b);
        assert!(a != b);
        assert!(b.iter().all(|x| x.abs() <= 1.0));
    }
}
