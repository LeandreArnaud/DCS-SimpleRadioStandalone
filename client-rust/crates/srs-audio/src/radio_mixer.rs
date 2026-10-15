//! Per-local-radio mixer: port of `Common/Audio/Providers/RadioMixingProvider.cs`
//! and `ClientEffectsPipeline.ProcessSegments` (RX side).
//!
//! Each call produces stereo (interleaved) audio for one radio: queued
//! start/end tones first, then the mixdown of every remote client that has
//! audio on this radio (FM capture when the server enables
//! `IRL_RADIO_RX_INTERFERENCE`), passed through the RX radio model
//! (dry/wet ratio), clipped, then balanced left/right.

use std::collections::HashMap;
use std::sync::Arc;

use srs_protocol::constants::OUTPUT_SEGMENT_FRAMES;
use srs_protocol::{Modulation, ShortGuid};

use crate::effects::tones::EffectQueue;
use crate::effects::{EffectCache, EffectSettings, RadioModelFactory, RxModel};
use crate::remote_client::RemoteClientAudio;

pub struct RadioMixer {
    radio_id: usize,
    /// Left/right balance, -1 (left) .. 1 (right), from `Radio{N}Channel`.
    pub balance: f32,
    mix_buffer: Vec<f32>,
    available: usize,
    scratch: Vec<f32>,
    mono: Vec<f32>,
    is_receiving: bool,
    /// Only updated by the transmit tone calls, like the C# `lastModulation`.
    last_modulation: Modulation,
    effects: EffectQueue,
    cache: Arc<EffectCache>,
    factory: Arc<RadioModelFactory>,
    rx_models: HashMap<String, RxModel>,
    wet: Vec<f32>,
    /// FM capture candidate (`capturedFMSegment`).
    fm: Vec<f32>,
    fm_len: usize,
    fm_power: f64,
}

impl RadioMixer {
    pub fn new(radio_id: usize, cache: Arc<EffectCache>, factory: Arc<RadioModelFactory>) -> Self {
        Self {
            radio_id,
            balance: 0.0,
            mix_buffer: Vec::with_capacity(OUTPUT_SEGMENT_FRAMES * 2),
            available: 0,
            scratch: vec![0.0; OUTPUT_SEGMENT_FRAMES],
            mono: vec![0.0; OUTPUT_SEGMENT_FRAMES],
            is_receiving: false,
            last_modulation: Modulation::Disabled,
            effects: EffectQueue::new(),
            cache,
            factory,
            rx_models: HashMap::new(),
            wet: Vec::new(),
            fm: vec![0.0; OUTPUT_SEGMENT_FRAMES],
            fm_len: 0,
            fm_power: 0.0,
        }
    }

    pub fn radio_id(&self) -> usize {
        self.radio_id
    }

    pub fn is_receiving(&self) -> bool {
        self.is_receiving
    }

    fn read_mix_buffer(&mut self, out: &mut [f32]) -> usize {
        let n = out.len().min(self.available);
        if n > 0 {
            out[..n].copy_from_slice(&self.mix_buffer[..n]);
            self.mix_buffer.copy_within(n..self.available, 0);
            self.available -= n;
        }
        n
    }

    /// Fill `out` (interleaved stereo) and return the number of samples
    /// written (always `out.len()`, silence-padded).
    pub fn read(
        &mut self,
        settings: &EffectSettings,
        sources: &mut HashMap<ShortGuid, RemoteClientAudio>,
        out: &mut [f32],
    ) -> usize {
        let mono_len = out.len() / 2;
        if self.mono.len() < mono_len {
            self.mono.resize(mono_len, 0.0);
        }
        self.mono[..mono_len].iter_mut().for_each(|s| *s = 0.0);

        let mut tmp = std::mem::take(&mut self.mono);
        let mut mono_offset = self.effects.read(&mut tmp[..mono_len]);
        mono_offset += self.read_mix_buffer(&mut tmp[mono_offset..mono_len]);

        if mono_offset < mono_len {
            let desired = mono_len - mono_offset;
            if self.scratch.len() < desired {
                self.scratch.resize(desired, 0.0);
            }
            if self.fm.len() < desired {
                self.fm.resize(desired, 0.0);
            }
            let mut longest = 0usize;
            let mut any = false;
            let mut ky58 = false;
            self.fm_len = 0;
            // Accumulate every source's segment into mix_buffer[available..] (dry sum).
            let need = self.available + desired;
            if self.mix_buffer.len() < need {
                self.mix_buffer.resize(need, 0.0);
            }
            self.mix_buffer[self.available..need]
                .iter_mut()
                .for_each(|s| *s = 0.0);
            for src in sources.values_mut() {
                let (n, meta) = src.read(settings, self.radio_id, &mut self.scratch[..desired]);
                if n == 0 {
                    continue;
                }
                any = true;
                longest = longest.max(n);
                if meta.decryptable && meta.encryption > 0 {
                    ky58 = true;
                }
                if settings.irl_radio_rx_interference
                    && !meta.no_audio_effects
                    && meta.modulation == Modulation::Fm
                {
                    // FM capture: keep the segment with the highest ReceivingPower (sic).
                    if self.fm_len == 0 || self.fm_power < meta.receiving_power {
                        self.fm[..n].copy_from_slice(&self.scratch[..n]);
                        self.fm_len = n;
                        self.fm_power = meta.receiving_power;
                    }
                    continue;
                }
                let dst = &mut self.mix_buffer[self.available..self.available + n];
                for (d, s) in dst.iter_mut().zip(&self.scratch[..n]) {
                    *d += *s;
                }
            }
            if self.fm_len > 0 {
                let dst = &mut self.mix_buffer[self.available..self.available + self.fm_len];
                for (d, s) in dst.iter_mut().zip(&self.fm[..self.fm_len]) {
                    *d += *s;
                }
            }
            let active = any || self.available > 0;
            mono_offset +=
                self.handle_start_end_tones(settings, active, ky58, &mut tmp[mono_offset..mono_len]);
            if any {
                let start = self.available;
                self.process_segments(settings, start, longest);
                // ClipArray on the newly mixed region.
                for s in &mut self.mix_buffer[start..start + longest] {
                    *s = s.clamp(-1.0, 1.0);
                }
                self.available += longest;
                mono_offset += self.read_mix_buffer(&mut tmp[mono_offset..mono_len]);
            }
        }

        if self.available == 0 && self.mix_buffer.len() > OUTPUT_SEGMENT_FRAMES {
            self.mix_buffer.truncate(OUTPUT_SEGMENT_FRAMES);
        }

        // Mono → balanced stereo (`CreateBalancedMix`).
        let balance = self.balance.clamp(-1.0, 1.0);
        let left = (1.0 - balance) / 2.0;
        let right = 1.0 - left;
        for i in 0..mono_len {
            let s = if i < mono_offset { tmp[i] } else { 0.0 };
            out[2 * i] = s * left;
            out[2 * i + 1] = s * right;
        }
        self.mono = tmp;
        out.len()
    }

    /// `ClientEffectsPipeline.ProcessSegments` on `mix_buffer[start..start+len]`
    /// (already the dry sum): RX model wet/dry mix and optional clipping.
    fn process_segments(&mut self, settings: &EffectSettings, start: usize, len: usize) {
        let ratio = settings.radio_effects_ratio.clamp(0.0, 1.0);
        // `modelName` is always null in the C# caller → "" → default intercom RX model.
        let model = match self.rx_models.get_mut("") {
            Some(m) => m,
            None => self
                .rx_models
                .entry(String::new())
                .or_insert_with(|| self.factory.rx_or_default_intercom("")),
        };
        let dry = &mut self.mix_buffer[start..start + len];
        self.wet.clear();
        self.wet.extend_from_slice(dry);
        model.rx_effect.process(&mut self.wet);
        for (d, w) in dry.iter_mut().zip(&self.wet) {
            *d = *d * (1.0 - ratio) + *w * ratio;
        }
        if settings.clipping && ratio > 0.0 {
            for s in dry.iter_mut() {
                *s = s.clamp(-1.0, 1.0);
            }
        }
    }

    fn handle_start_end_tones(
        &mut self,
        settings: &EffectSettings,
        active: bool,
        encrypted: bool,
        out: &mut [f32],
    ) -> usize {
        if active != self.is_receiving {
            if active {
                self.effects.reset();
                self.play_start_receive(settings, encrypted, self.last_modulation);
                self.is_receiving = true;
            } else {
                self.is_receiving = false;
                self.play_end_receive(settings, self.last_modulation);
            }
        }
        self.effects.read(out)
    }

    fn play_end_receive(&mut self, s: &EffectSettings, modulation: Modulation) {
        if !s.rx_effects_end {
            return;
        }
        if self.radio_id == 0 {
            self.effects
                .write_effect(self.cache.intercom_end(&s.intercom_end_selection));
        } else if modulation == Modulation::Mids && s.mids_radio_effect {
            self.effects.write_effect(Some(&self.cache.mids_end));
        } else {
            self.effects
                .write_effect(self.cache.radio_end(&s.radio_end_selection));
        }
    }

    pub fn play_start_receive(&mut self, s: &EffectSettings, encrypted: bool, modulation: Modulation) {
        if !s.rx_effects_start {
            return;
        }
        if modulation == Modulation::Mids && s.mids_radio_effect {
            return;
        }
        if self.radio_id == 0 {
            self.effects
                .write_effect(self.cache.intercom_start(&s.intercom_start_selection));
        } else if encrypted && s.radio_encryption_effects {
            self.effects.write_effect(Some(&self.cache.ky58_rx));
        } else {
            self.effects
                .write_effect(self.cache.radio_start(&s.radio_start_selection));
        }
    }

    /// `PlaySoundEffectStartTransmit`.
    pub fn play_start_transmit(&mut self, s: &EffectSettings, encrypted: bool, modulation: Modulation) {
        self.last_modulation = modulation;
        if !s.tx_effects_start {
            return;
        }
        if self.radio_id == 0 {
            self.effects
                .write_effect(self.cache.intercom_start(&s.intercom_start_selection));
        } else if encrypted && s.radio_encryption_effects {
            self.effects.write_effect(Some(&self.cache.ky58_tx));
        } else if modulation == Modulation::Mids && s.mids_radio_effect {
            self.effects.write_effect(Some(&self.cache.mids_tx));
        } else {
            self.effects
                .write_effect(self.cache.radio_start(&s.radio_start_selection));
        }
    }

    /// `PlaySoundEffectEndTransmit`.
    pub fn play_end_transmit(&mut self, s: &EffectSettings, modulation: Modulation) {
        self.last_modulation = modulation;
        if !s.tx_effects_end {
            return;
        }
        if self.radio_id == 0 {
            self.effects
                .write_effect(self.cache.intercom_end(&s.intercom_end_selection));
        } else if modulation == Modulation::Mids && s.mids_radio_effect {
            self.effects.write_effect(Some(&self.cache.mids_end));
        } else {
            self.effects
                .write_effect(self.cache.radio_end(&s.radio_end_selection));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::cache::CachedEffect;

    fn cache_with_tones() -> Arc<EffectCache> {
        let mut c = EffectCache::default();
        c.radio_start.push(CachedEffect {
            file_name: "RADIO_TRANS_START.wav".into(),
            samples: Arc::new(vec![0.5; 100]),
            rms_db: -6.0,
        });
        c.radio_end.push(CachedEffect {
            file_name: "RADIO_TRANS_END.wav".into(),
            samples: Arc::new(vec![-0.5; 50]),
            rms_db: -6.0,
        });
        c.ky58_tx = CachedEffect {
            file_name: "KY_58_TX.wav".into(),
            samples: Arc::new(vec![0.25; 10]),
            rms_db: -12.0,
        };
        Arc::new(c)
    }

    #[test]
    fn tx_tones_are_played_through_read() {
        let mut m = RadioMixer::new(1, cache_with_tones(), Arc::new(RadioModelFactory::empty()));
        let s = EffectSettings::default();
        m.play_start_transmit(&s, false, Modulation::Am);
        let mut sources = HashMap::new();
        let mut out = vec![0f32; 2 * 120];
        m.read(&s, &mut sources, &mut out);
        // 100 samples of 0.5 split evenly left/right (balance 0 → 0.25 each side), then silence.
        assert!((out[0] - 0.25).abs() < 1e-6);
        assert!((out[1] - 0.25).abs() < 1e-6);
        assert!((out[2 * 99] - 0.25).abs() < 1e-6);
        assert_eq!(out[2 * 100], 0.0);
        m.play_end_transmit(&s, Modulation::Am);
        m.read(&s, &mut sources, &mut out);
        assert!((out[0] + 0.25).abs() < 1e-6);
        assert_eq!(out[2 * 50], 0.0);
        // Encrypted start on a radio → KY-58 tone; tone disabled → nothing.
        m.play_start_transmit(&s, true, Modulation::Am);
        m.read(&s, &mut sources, &mut out);
        assert!((out[0] - 0.125).abs() < 1e-6);
        let off = EffectSettings {
            tx_effects_start: false,
            ..Default::default()
        };
        m.play_start_transmit(&off, false, Modulation::Am);
        m.read(&s, &mut sources, &mut out);
        assert!(out.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn balance_and_rx_model_are_applied() {
        let mut m = RadioMixer::new(
            2,
            Arc::new(EffectCache::default()),
            Arc::new(RadioModelFactory::empty()),
        );
        m.balance = 1.0;
        let s = EffectSettings {
            radio_effects_ratio: 0.0,
            ..Default::default()
        };
        m.mix_buffer = vec![0.4; 10];
        m.available = 10;
        let mut sources = HashMap::new();
        let mut out = vec![0f32; 20];
        m.read(&s, &mut sources, &mut out);
        assert_eq!(out[0], 0.0);
        assert!((out[1] - 0.4).abs() < 1e-6);
        // RX model (HP 270 / LP 4500) on a DC block attenuates it.
        let mut dc = vec![0.5f32; 480];
        m.mix_buffer = dc.clone();
        m.process_segments(&EffectSettings::default(), 0, 480);
        dc.copy_from_slice(&m.mix_buffer[..480]);
        assert!(dc[479].abs() < 0.3, "{}", dc[479]);
    }
}
