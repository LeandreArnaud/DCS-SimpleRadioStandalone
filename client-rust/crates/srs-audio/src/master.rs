//! Final mixdown: port of `SRSMixingSampleProvider` (sum of the 11 radio
//! mixers, clamp, non-finite → 0, always a full buffer) followed by
//! `VolumeSampleProviderWithPeak` (speaker boost, clamp, peak meter).

use std::collections::HashMap;
use std::sync::Arc;

use srs_protocol::constants::MAX_RADIOS;
use srs_protocol::ShortGuid;

use crate::effects::{EffectCache, EffectSettings, RadioModelFactory};
use crate::radio_mixer::RadioMixer;
use crate::remote_client::RemoteClientAudio;

pub struct MasterMixer {
    pub radios: Vec<RadioMixer>,
    /// `SpeakerBoost` (linear).
    pub volume: f32,
    /// Peak of the last block, linear 0..1.
    pub peak: f32,
    scratch: Vec<f32>,
    /// One-shot UI sounds (interleaved stereo), mixed after the radios,
    /// not affected by the speaker boost.
    system: std::collections::VecDeque<f32>,
}

impl Default for MasterMixer {
    fn default() -> Self {
        Self::new(
            Arc::new(EffectCache::default()),
            Arc::new(RadioModelFactory::empty()),
        )
    }
}

impl MasterMixer {
    pub fn new(cache: Arc<EffectCache>, factory: Arc<RadioModelFactory>) -> Self {
        Self {
            radios: (0..MAX_RADIOS)
                .map(|i| RadioMixer::new(i, cache.clone(), factory.clone()))
                .collect(),
            volume: 1.0,
            peak: 0.0,
            scratch: vec![0.0; 1920 * 2],
            system: std::collections::VecDeque::new(),
        }
    }

    /// Queue a UI sound (interleaved stereo at 48 kHz).
    pub fn play_system(&mut self, stereo: &[f32]) {
        self.system.extend(stereo.iter().copied());
    }

    /// Render interleaved stereo at 48 kHz into `out`.
    pub fn read(
        &mut self,
        settings: &EffectSettings,
        sources: &mut HashMap<ShortGuid, RemoteClientAudio>,
        out: &mut [f32],
    ) {
        if self.scratch.len() < out.len() {
            self.scratch.resize(out.len(), 0.0);
        }
        let n = out.len();
        out.iter_mut().for_each(|s| *s = 0.0);
        let mut scratch = std::mem::take(&mut self.scratch);
        for radio in &mut self.radios {
            radio.read(settings, sources, &mut scratch[..n]);
            for (o, s) in out.iter_mut().zip(&scratch[..n]) {
                *o += *s;
            }
        }
        self.scratch = scratch;
        let mut peak = 0f32;
        for s in out.iter_mut() {
            let v = if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 };
            let v = (v * self.volume).clamp(-1.0, 1.0);
            peak = peak.max(v.abs());
            *s = v;
        }
        self.peak = peak;
        if !self.system.is_empty() {
            for s in out.iter_mut() {
                let Some(v) = self.system.pop_front() else { break };
                *s = (*s + v).clamp(-1.0, 1.0);
            }
        }
    }
}
