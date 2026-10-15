//! DSP primitives used by the radio effect chains, ported from
//! `Common/Audio/Dsp/*`, `Common/Audio/Codecs/CVSD.cs` and the NAudio classes
//! the C# code relies on (`BiQuadFilter`, `SimpleCompressor`,
//! `EnvelopeDetector`, `SignalGenerator` pink noise).

use std::f64::consts::PI;

pub fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

pub fn linear_to_db(lin: f64) -> f64 {
    20.0 * lin.log10()
}

/// `Dsp.IFilter`: one sample in, one sample out.
pub trait Filter: Send {
    fn transform(&mut self, input: f32) -> f32;
}

/// NAudio `BiQuadFilter` (RBJ cookbook), with its exact coefficient layout.
#[derive(Debug, Clone)]
pub struct BiQuad {
    a0: f64,
    a1: f64,
    a2: f64,
    a3: f64,
    a4: f64,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiQuad {
    fn from_coefficients(aa0: f64, aa1: f64, aa2: f64, b0: f64, b1: f64, b2: f64) -> Self {
        Self {
            a0: b0 / aa0,
            a1: b1 / aa0,
            a2: b2 / aa0,
            a3: aa1 / aa0,
            a4: aa2 / aa0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn low_pass(sample_rate: f64, cutoff: f64, q: f64) -> Self {
        let w0 = 2.0 * PI * cutoff / sample_rate;
        let cosw0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);
        Self::from_coefficients(
            1.0 + alpha,
            -2.0 * cosw0,
            1.0 - alpha,
            (1.0 - cosw0) / 2.0,
            1.0 - cosw0,
            (1.0 - cosw0) / 2.0,
        )
    }

    pub fn high_pass(sample_rate: f64, cutoff: f64, q: f64) -> Self {
        let w0 = 2.0 * PI * cutoff / sample_rate;
        let cosw0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);
        Self::from_coefficients(
            1.0 + alpha,
            -2.0 * cosw0,
            1.0 - alpha,
            (1.0 + cosw0) / 2.0,
            -(1.0 + cosw0),
            (1.0 + cosw0) / 2.0,
        )
    }

    pub fn peaking_eq(sample_rate: f64, centre: f64, q: f64, db_gain: f64) -> Self {
        let w0 = 2.0 * PI * centre / sample_rate;
        let cosw0 = w0.cos();
        let sinw0 = w0.sin();
        let alpha = sinw0 / (2.0 * q);
        let a = 10f64.powf(db_gain / 40.0);
        Self::from_coefficients(
            1.0 + alpha / a,
            -2.0 * cosw0,
            1.0 - alpha / a,
            1.0 + alpha * a,
            -2.0 * cosw0,
            1.0 - alpha * a,
        )
    }
}

impl Filter for BiQuad {
    fn transform(&mut self, input: f32) -> f32 {
        let result = self.a0 * f64::from(input) + self.a1 * f64::from(self.x1) + self.a2 * f64::from(self.x2)
            - self.a3 * f64::from(self.y1)
            - self.a4 * f64::from(self.y2);
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = result as f32;
        self.y1
    }
}

/// `Dsp.FirstOrderFilter` (bilinear first-order low/high pass).
#[derive(Debug, Clone)]
pub struct FirstOrder {
    b0: f64,
    b1: f64,
    a1: f64,
    x_n1: f32,
    y_n1: f32,
}

impl FirstOrder {
    fn from_coefficients(aa0: f64, bb0: f64, bb1: f64, aa1: f64) -> Self {
        Self {
            b0: bb0 / aa0,
            b1: bb1 / aa0,
            a1: aa1 / aa0,
            x_n1: 0.0,
            y_n1: 0.0,
        }
    }

    pub fn low_pass(sample_rate: f64, cutoff: f64) -> Self {
        let w0 = 2.0 * PI * cutoff / sample_rate;
        let (sinw0, cosw0) = w0.sin_cos();
        Self::from_coefficients(sinw0 + 1.0 + cosw0, sinw0, sinw0, sinw0 - 1.0 - cosw0)
    }

    pub fn high_pass(sample_rate: f64, cutoff: f64) -> Self {
        let w0 = 2.0 * PI * cutoff / sample_rate;
        let (sinw0, cosw0) = w0.sin_cos();
        Self::from_coefficients(
            sinw0 + 1.0 + cosw0,
            1.0 + cosw0,
            -1.0 - cosw0,
            sinw0 - 1.0 - cosw0,
        )
    }
}

impl Filter for FirstOrder {
    fn transform(&mut self, input: f32) -> f32 {
        self.y_n1 = (self.b0 * f64::from(input) + self.b1 * f64::from(self.x_n1)
            - self.a1 * f64::from(self.y_n1)) as f32;
        self.x_n1 = input;
        self.y_n1
    }
}

/// `FiltersProvider`: each filter in turn, skipping exact zeros.
pub fn apply_filters(filters: &mut [Box<dyn Filter>], buf: &mut [f32]) {
    for f in filters.iter_mut() {
        for s in buf.iter_mut() {
            if *s != 0.0 {
                *s = f.transform(*s);
            }
        }
    }
}

/// NAudio `EnvelopeDetector`.
#[derive(Debug, Clone)]
pub struct EnvelopeDetector {
    coeff: f64,
}

impl EnvelopeDetector {
    pub fn new(ms: f64, sample_rate: f64) -> Self {
        Self {
            coeff: (-1.0 / (0.001 * ms * sample_rate)).exp(),
        }
    }

    pub fn run(&self, input: f64, state: f64) -> f64 {
        input + self.coeff * (state - input)
    }
}

/// NAudio `AttRelEnvelope`.
#[derive(Debug, Clone)]
pub struct AttRelEnvelope {
    attack: EnvelopeDetector,
    release: EnvelopeDetector,
}

pub const DC_OFFSET: f64 = 1.0e-25;

impl AttRelEnvelope {
    pub fn new(attack_ms: f64, release_ms: f64, sample_rate: f64) -> Self {
        Self {
            attack: EnvelopeDetector::new(attack_ms, sample_rate),
            release: EnvelopeDetector::new(release_ms, sample_rate),
        }
    }

    pub fn run(&self, input: f64, state: f64) -> f64 {
        if input > state {
            self.attack.run(input, state)
        } else {
            self.release.run(input, state)
        }
    }
}

/// `Dsp.SidechainCompressor` (and NAudio `SimpleCompressor` when the
/// sidechain is the signal itself). Chunkware convention: gain change in
/// dB = over·(ratio − 1), so a ratio below 1 compresses and the shipped
/// 1.18 actually expands slightly. Kept as is.
#[derive(Debug, Clone)]
pub struct Compressor {
    env: AttRelEnvelope,
    envelope_db: f64,
    pub make_up_gain: f64,
    pub threshold: f64,
    pub ratio: f64,
}

impl Compressor {
    pub fn new(
        attack_ms: f64,
        release_ms: f64,
        sample_rate: f64,
        threshold: f64,
        ratio: f64,
        make_up_gain: f64,
    ) -> Self {
        Self {
            env: AttRelEnvelope::new(attack_ms, release_ms, sample_rate),
            envelope_db: DC_OFFSET,
            make_up_gain,
            threshold,
            ratio,
        }
    }

    /// Gain-reduce `signal` according to `side` (`SidechainCompressor.Process`).
    pub fn process(&mut self, side: f64, signal: f64) -> f64 {
        let rect = side.abs() + DC_OFFSET;
        let key_db = linear_to_db(rect);
        let mut over_db = key_db - self.threshold;
        if over_db < 0.0 {
            over_db = 0.0;
        }
        over_db += DC_OFFSET;
        self.envelope_db = self.env.run(over_db, self.envelope_db);
        over_db = self.envelope_db - DC_OFFSET;
        let gr = over_db * (self.ratio - 1.0);
        let gr = db_to_linear(gr) * db_to_linear(self.make_up_gain);
        signal * gr
    }
}

/// `Codecs.CVSD`: continuously variable slope delta "encryption" effect.
#[derive(Debug, Clone)]
pub struct Cvsd {
    reference: f32,
    bitref: u32,
    delta: f32,
}

impl Default for Cvsd {
    fn default() -> Self {
        Self::new()
    }
}

impl Cvsd {
    const BIT_MASK: u32 = 0b111;
    const ZETA: f32 = 1.5;
    const DELTA_MIN: f32 = 0.01;
    const DELTA_MAX: f32 = 1.0;

    pub fn new() -> Self {
        Self {
            reference: 0.0,
            bitref: 0,
            delta: 0.0,
        }
    }

    pub fn transform(&mut self, sample: f32) -> f32 {
        let bit = if self.reference > sample { 0 } else { 1 };
        self.bitref <<= 1;
        self.bitref |= bit;
        self.bitref &= Self::BIT_MASK;
        if self.bitref == 0 || self.bitref == Self::BIT_MASK {
            self.delta *= Self::ZETA;
        } else {
            self.delta /= Self::ZETA;
        }
        self.delta = self.delta.clamp(Self::DELTA_MIN, Self::DELTA_MAX);
        self.reference += if bit != 0 { self.delta } else { -self.delta };
        self.reference = self.reference.clamp(-1.0, 1.0);
        self.reference
    }
}

/// `SaturationProvider`: tanh-style soft clipping above a threshold.
pub fn saturate(buf: &mut [f32], gain_db: f32, threshold_db: f32) {
    let gain = db_to_linear(f64::from(gain_db)) as f32;
    let threshold = db_to_linear(f64::from(threshold_db)) as f32;
    for s in buf.iter_mut() {
        let g = *s * gain;
        if g.abs() > threshold {
            let e = (2.0 * g).exp();
            *s = (e - 1.0) / (e + 1.0);
        } else {
            *s = g;
        }
    }
}

pub fn clip(buf: &mut [f32], min: f32, max: f32) {
    for s in buf.iter_mut() {
        *s = s.clamp(min, max);
    }
}

pub fn gain(buf: &mut [f32], linear: f32) {
    if linear != 1.0 {
        for s in buf.iter_mut() {
            *s *= linear;
        }
    }
}

/// Small xorshift RNG (no allocation, deterministic per instance).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `GaussianWhiteNoise`: Box-Muller normal samples.
#[derive(Debug, Clone)]
pub struct WhiteNoise {
    rng: Rng,
}

impl WhiteNoise {
    pub fn new(seed: u64) -> Self {
        Self { rng: Rng::new(seed) }
    }

    pub fn fill(&mut self, buf: &mut [f32]) {
        let mut i = 0;
        while i + 1 < buf.len() {
            let (z0, z1) = self.pair();
            buf[i] = z0 as f32;
            buf[i + 1] = z1 as f32;
            i += 2;
        }
        if i < buf.len() {
            buf[i] = self.pair().0 as f32;
        }
    }

    fn pair(&mut self) -> (f64, f64) {
        let u1 = self.rng.next_f64().max(1e-9);
        let u2 = self.rng.next_f64().max(1e-9);
        let radius = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * PI * u2;
        let (z1, z0) = theta.sin_cos();
        (z0 * radius, z1 * radius)
    }
}

/// NAudio `SignalGenerator` pink noise (Paul Kellet's filter), including the
/// `/5` scaling.
#[derive(Debug, Clone)]
pub struct PinkNoise {
    rng: Rng,
    b: [f64; 7],
}

impl PinkNoise {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            b: [0.0; 7],
        }
    }

    pub fn fill(&mut self, buf: &mut [f32], gain: f64) {
        for s in buf.iter_mut() {
            let white = 2.0 * self.rng.next_f64() - 1.0;
            let b = &mut self.b;
            b[0] = 0.99886 * b[0] + white * 0.0555179;
            b[1] = 0.99332 * b[1] + white * 0.0750759;
            b[2] = 0.96900 * b[2] + white * 0.1538520;
            b[3] = 0.86650 * b[3] + white * 0.3104856;
            b[4] = 0.55000 * b[4] + white * 0.5329522;
            b[5] = -0.7616 * b[5] - white * 0.0168980;
            let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
            b[6] = white * 0.115926;
            *s = (gain * (pink / 5.0)) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(v: &[f32]) -> f64 {
        (v.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>() / v.len() as f64).sqrt()
    }

    fn tone(freq: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f64 * 2.0 * PI * freq / 48000.0).sin() as f32)
            .collect()
    }

    #[test]
    fn biquad_low_pass_attenuates_highs() {
        let mut lp = BiQuad::low_pass(48000.0, 1000.0, 0.707);
        let mut lo = tone(200.0, 9600);
        let mut hi = tone(8000.0, 9600);
        for s in lo.iter_mut() {
            *s = lp.transform(*s);
        }
        let mut lp2 = BiQuad::low_pass(48000.0, 1000.0, 0.707);
        for s in hi.iter_mut() {
            *s = lp2.transform(*s);
        }
        assert!(rms(&lo[4800..]) > 0.6, "{}", rms(&lo[4800..]));
        assert!(rms(&hi[4800..]) < 0.05, "{}", rms(&hi[4800..]));
    }

    #[test]
    fn first_order_high_pass_and_peaking() {
        let mut hp = FirstOrder::high_pass(48000.0, 2000.0);
        let mut lo = tone(100.0, 9600);
        for s in lo.iter_mut() {
            *s = hp.transform(*s);
        }
        assert!(rms(&lo[4800..]) < 0.1);
        let mut pk = BiQuad::peaking_eq(48000.0, 2801.0, 0.5, 5.0);
        let mut t = tone(2801.0, 9600);
        for s in t.iter_mut() {
            *s = pk.transform(*s);
        }
        let boost = linear_to_db(rms(&t[4800..]) / (1.0 / 2f64.sqrt()));
        assert!((boost - 5.0).abs() < 0.5, "boost {boost}");
    }

    #[test]
    fn compressor_reduces_loud_signal() {
        let mut c = Compressor::new(10.0, 200.0, 48000.0, -20.0, 0.25, 0.0);
        let mut out = 0.0;
        for _ in 0..48000 {
            out = c.process(1.0, 1.0);
        }
        // 20 dB over threshold, ratio 0.25 → 20·(0.25−1) = −15 dB.
        let reduction = -linear_to_db(out);
        assert!((reduction - 15.0).abs() < 0.5, "reduction {reduction}");
        let mut e = Compressor::new(10.0, 200.0, 48000.0, -20.0, 1.18, 0.0);
        for _ in 0..48000 {
            out = e.process(1.0, 1.0);
        }
        assert!(
            (linear_to_db(out) - 3.6).abs() < 0.2,
            "shipped ratio expands: {}",
            linear_to_db(out)
        );
        let mut q = Compressor::new(10.0, 200.0, 48000.0, -20.0, 4.0, 6.0);
        let quiet = q.process(0.01, 0.01);
        assert!((quiet / 0.01 - db_to_linear(6.0)).abs() < 1e-3, "make-up only");
    }

    #[test]
    fn cvsd_tracks_input_roughly() {
        let mut c = Cvsd::new();
        let input = tone(300.0, 4800);
        let out: Vec<f32> = input.iter().map(|&s| c.transform(s)).collect();
        let err: f64 = input
            .iter()
            .zip(&out)
            .skip(480)
            .map(|(a, b)| f64::from((a - b).abs()))
            .sum::<f64>()
            / 4320.0;
        assert!(err < 0.2, "mean error {err}");
    }

    #[test]
    fn saturation_and_clipping() {
        let mut b = vec![0.5f32, -0.9, 0.01];
        saturate(&mut b, 0.0, -20.0);
        assert!(b[0] < 0.5 && b[0] > 0.4);
        assert!(b[1] > -0.9 && b[1] < -0.6);
        assert!((b[2] - 0.01).abs() < 1e-6);
        let mut c = vec![2.0f32, -2.0, 0.1];
        clip(&mut c, -1.0, 1.0);
        assert_eq!(c, vec![1.0, -1.0, 0.1]);
    }

    #[test]
    fn noise_levels() {
        let mut w = WhiteNoise::new(7);
        let mut buf = vec![0f32; 48000];
        w.fill(&mut buf);
        let r = rms(&buf);
        assert!((r - 1.0).abs() < 0.05, "white rms {r}");
        let mut p = PinkNoise::new(7);
        p.fill(&mut buf, 1.0);
        let r = rms(&buf);
        assert!(r > 0.05 && r < 0.5, "pink rms {r}");
    }
}
