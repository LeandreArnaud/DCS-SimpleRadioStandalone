//! Microphone downsampler: any device rate / channel count → 16 kHz mono
//! i16, replacing the DMO / WDL resamplers of the C# client.
//!
//! Anti-aliasing windowed-sinc FIR at the input rate (cutoff just below
//! 8 kHz), then fractional-step linear interpolation on the filtered signal.
//! Adequate for narrowband voice; a polyphase design can replace it later.

use std::collections::VecDeque;

use srs_protocol::constants::MIC_SAMPLE_RATE;

pub struct Downsampler {
    in_rate: u32,
    channels: usize,
    taps: Vec<f32>,
    history: VecDeque<f32>,
    step: f64,
    phase: f64,
    filtered: VecDeque<f32>,
    out: VecDeque<i16>,
}

impl Downsampler {
    pub fn new(in_rate: u32, channels: u16) -> Self {
        let channels = channels.max(1) as usize;
        let taps = if in_rate > MIC_SAMPLE_RATE {
            design_lowpass(in_rate, 7_000.0, 63)
        } else {
            vec![1.0]
        };
        let mut history = VecDeque::with_capacity(taps.len());
        history.extend(std::iter::repeat_n(0.0f32, taps.len()));
        Self {
            in_rate,
            channels,
            taps,
            history,
            step: f64::from(in_rate) / f64::from(MIC_SAMPLE_RATE),
            phase: 0.0,
            filtered: VecDeque::with_capacity(4096),
            out: VecDeque::with_capacity(4096),
        }
    }

    pub fn in_rate(&self) -> u32 {
        self.in_rate
    }

    /// Push interleaved f32 samples from the device.
    pub fn push(&mut self, interleaved: &[f32]) {
        for frame in interleaved.chunks(self.channels) {
            let mono = frame.iter().sum::<f32>() / self.channels as f32;
            self.history.pop_front();
            self.history.push_back(mono);
            let mut acc = 0.0f32;
            for (h, t) in self.history.iter().zip(&self.taps) {
                acc += h * t;
            }
            self.filtered.push_back(acc);
        }
        // Interpolate while we have a next sample.
        while (self.phase as usize) + 1 < self.filtered.len() {
            let idx = self.phase as usize;
            let frac = (self.phase - idx as f64) as f32;
            let a = self.filtered[idx];
            let b = self.filtered[idx + 1];
            let v = a + (b - a) * frac;
            self.out.push_back((v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
            self.phase += self.step;
        }
        let consumed = self.phase as usize;
        if consumed > 0 {
            self.filtered.drain(..consumed.min(self.filtered.len()));
            self.phase -= consumed as f64;
        }
    }

    pub fn available(&self) -> usize {
        self.out.len()
    }

    /// Pop exactly `frame.len()` samples when available.
    pub fn pop_frame(&mut self, frame: &mut [i16]) -> bool {
        if self.out.len() < frame.len() {
            return false;
        }
        for s in frame.iter_mut() {
            *s = self.out.pop_front().unwrap_or(0);
        }
        true
    }
}

/// Hamming-windowed sinc low-pass, odd tap count, unity DC gain.
fn design_lowpass(rate: u32, cutoff_hz: f64, taps: usize) -> Vec<f32> {
    let taps = taps | 1;
    let fc = cutoff_hz / f64::from(rate);
    let m = (taps - 1) as f64;
    let mut h: Vec<f64> = (0..taps)
        .map(|n| {
            let x = n as f64 - m / 2.0;
            let sinc = if x == 0.0 {
                2.0 * fc
            } else {
                (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x)
            };
            let w = 0.54 - 0.46 * (2.0 * std::f64::consts::PI * n as f64 / m).cos();
            sinc * w
        })
        .collect();
    let sum: f64 = h.iter().sum();
    for v in &mut h {
        *v /= sum;
    }
    h.into_iter().map(|v| v as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(in_rate: u32, freq: f32, channels: u16) -> Vec<i16> {
        let mut d = Downsampler::new(in_rate, channels);
        let n = in_rate as usize; // one second
        let mut buf = Vec::with_capacity(n * channels as usize);
        for i in 0..n {
            let v = (i as f32 * 2.0 * std::f32::consts::PI * freq / in_rate as f32).sin() * 0.5;
            for _ in 0..channels {
                buf.push(v);
            }
        }
        d.push(&buf);
        let mut out = Vec::new();
        let mut frame = [0i16; 640];
        while d.pop_frame(&mut frame) {
            out.extend_from_slice(&frame);
        }
        out
    }

    fn rms(v: &[i16]) -> f64 {
        (v.iter().map(|&s| (f64::from(s) / 32768.0).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
    }

    #[test]
    fn rate_conversion_lengths() {
        for rate in [48_000u32, 44_100, 96_000, 16_000] {
            let out = run(rate, 440.0, 2);
            let expected = 16_000;
            assert!((out.len() as i64 - expected).abs() < 700, "{rate}: {}", out.len());
        }
    }

    #[test]
    fn passband_kept_stopband_removed() {
        let tone = run(48_000, 1_000.0, 1);
        let r = rms(&tone[3200..]);
        assert!((r - 0.5 / 2f64.sqrt()).abs() < 0.05, "1 kHz rms {r}");
        let alias = run(48_000, 15_000.0, 1);
        let r2 = rms(&alias[3200..]);
        assert!(r2 < 0.02, "15 kHz should be filtered out, rms {r2}");
    }
}
