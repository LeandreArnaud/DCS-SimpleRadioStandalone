//! Looping tones (`CachedEffectProvider`) and the per-radio one-shot effect
//! queue (`RadioMixingProvider.effectsBuffer`, a 5 s circular buffer).

use std::collections::VecDeque;
use std::sync::Arc;

use srs_protocol::constants::OUTPUT_SAMPLE_RATE;

use super::cache::CachedEffect;

/// `CachedEffectProvider`: plays the effect in a loop, keeping its position.
#[derive(Debug, Clone)]
pub struct LoopingTone {
    samples: Arc<Vec<f32>>,
    pos: usize,
}

impl LoopingTone {
    pub fn new(effect: &CachedEffect) -> Self {
        Self {
            samples: effect.samples.clone(),
            pos: 0,
        }
    }

    pub fn loaded(&self) -> bool {
        !self.samples.is_empty()
    }

    /// Fill `out` with the next samples (no-op when nothing is loaded).
    pub fn read(&mut self, out: &mut [f32]) {
        if self.samples.is_empty() {
            out.iter_mut().for_each(|s| *s = 0.0);
            return;
        }
        let mut done = 0;
        while done < out.len() {
            let n = (self.samples.len() - self.pos).min(out.len() - done);
            out[done..done + n].copy_from_slice(&self.samples[self.pos..self.pos + n]);
            self.pos += n;
            if self.pos == self.samples.len() {
                self.pos = 0;
            }
            done += n;
        }
    }
}

/// One-shot start/end tones queued for a radio; at most 5 s buffered.
#[derive(Debug)]
pub struct EffectQueue {
    buf: VecDeque<f32>,
}

impl Default for EffectQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectQueue {
    pub const CAPACITY: usize = OUTPUT_SAMPLE_RATE as usize * 5;

    pub fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(OUTPUT_SAMPLE_RATE as usize),
        }
    }

    /// `effectsBuffer.Write`: whatever does not fit is dropped.
    pub fn write(&mut self, samples: &[f32]) {
        let room = Self::CAPACITY - self.buf.len();
        self.buf.extend(samples.iter().take(room).copied());
    }

    pub fn write_effect(&mut self, effect: Option<&CachedEffect>) {
        if let Some(e) = effect.filter(|e| e.loaded()) {
            self.write(&e.samples);
        }
    }

    pub fn reset(&mut self) {
        self.buf.clear();
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// `ReadEffects`: drain up to `out.len()` samples, returns the count.
    pub fn read(&mut self, out: &mut [f32]) -> usize {
        let n = out.len().min(self.buf.len());
        for s in out.iter_mut().take(n) {
            *s = self.buf.pop_front().unwrap_or(0.0);
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looping_tone_wraps() {
        let e = CachedEffect {
            file_name: "t".into(),
            samples: Arc::new(vec![1.0, 2.0, 3.0]),
            rms_db: 0.0,
        };
        let mut t = LoopingTone::new(&e);
        let mut out = [0f32; 7];
        t.read(&mut out);
        assert_eq!(out, [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0]);
        t.read(&mut out[..2]);
        assert_eq!(&out[..2], &[2.0, 3.0]);
    }

    #[test]
    fn queue_reads_in_order_and_caps() {
        let mut q = EffectQueue::new();
        q.write(&[1.0, 2.0]);
        q.write(&[3.0]);
        let mut out = [0f32; 2];
        assert_eq!(q.read(&mut out), 2);
        assert_eq!(out, [1.0, 2.0]);
        assert_eq!(q.read(&mut out), 1);
        assert_eq!(out[0], 3.0);
        assert_eq!(q.read(&mut out), 0);
        q.write(&vec![0.5; EffectQueue::CAPACITY + 10]);
        assert_eq!(q.len(), EffectQueue::CAPACITY);
        q.reset();
        assert!(q.is_empty());
    }
}
