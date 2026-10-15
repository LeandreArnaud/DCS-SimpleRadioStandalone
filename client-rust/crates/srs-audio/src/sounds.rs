//! UI sounds (`Sounds.cs`: `beep-connected.wav` / `beep-disconnected.wav`).
//! The C# plays them with `SoundPlayer` on the default device; here they go
//! through the SRS output stream as a one-shot stereo overlay.

use std::path::Path;

use srs_protocol::constants::OUTPUT_SAMPLE_RATE;

/// Decoded, resampled to 48 kHz, interleaved stereo.
#[derive(Debug, Clone, Default)]
pub struct Sound {
    pub stereo: std::sync::Arc<Vec<f32>>,
}

impl Sound {
    pub fn loaded(&self) -> bool {
        !self.stereo.is_empty()
    }

    /// Any PCM WAV (8/16/24/32-bit int or float, any rate, 1..n channels).
    pub fn load(path: &Path) -> Self {
        let Ok(reader) = hound::WavReader::open(path) else {
            log::info!("Sound not found: {}", path.display());
            return Self::default();
        };
        let spec = reader.spec();
        let channels = spec.channels.max(1) as usize;
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.into_samples::<f32>().filter_map(Result::ok).collect(),
            hound::SampleFormat::Int => {
                let scale = 1.0 / (1u64 << (spec.bits_per_sample.max(1) - 1)) as f32;
                reader
                    .into_samples::<i32>()
                    .filter_map(Result::ok)
                    .map(|s| s as f32 * scale)
                    .collect()
            }
        };
        let frames = samples.len() / channels;
        if frames == 0 {
            return Self::default();
        }
        // Linear resample to 48 kHz, stereo out (mono duplicated, extra channels dropped).
        let ratio = f64::from(spec.sample_rate) / f64::from(OUTPUT_SAMPLE_RATE);
        let out_frames = (frames as f64 / ratio).floor() as usize;
        let mut out = Vec::with_capacity(out_frames * 2);
        for i in 0..out_frames {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let next = (idx + 1).min(frames - 1);
            for ch in 0..2 {
                let c = ch.min(channels - 1);
                let a = samples[idx * channels + c];
                let b = samples[next * channels + c];
                out.push(a + (b - a) * frac);
            }
        }
        Self {
            stereo: std::sync::Arc::new(out),
        }
    }
}

impl Sound {
    /// `SoundPlayer.Play`: play once on the default output device, on its
    /// own short-lived stream (independent of the SRS output stream, which
    /// may be stopped when a disconnect beep is due).
    pub fn play_detached(&self) {
        if !self.loaded() {
            return;
        }
        let samples = self.stereo.clone();
        std::thread::Builder::new()
            .name("srs-ui-sound".into())
            .spawn(move || {
                if let Err(e) = play_blocking(&samples) {
                    log::warn!("Cannot play UI sound: {e}");
                }
            })
            .ok();
    }
}

fn play_blocking(stereo48: &[f32]) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no output device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let rate = config.sample_rate();
    let channels = usize::from(config.channels()).max(1);
    // Resample 48 kHz stereo → device rate, then map to the device channel count.
    let frames = stereo48.len() / 2;
    let ratio = f64::from(OUTPUT_SAMPLE_RATE) / f64::from(rate);
    let out_frames = (frames as f64 / ratio) as usize;
    let mut buf: Vec<f32> = Vec::with_capacity(out_frames * channels);
    for i in 0..out_frames {
        let pos = i as f64 * ratio;
        let idx = (pos as usize).min(frames - 1);
        let next = (idx + 1).min(frames - 1);
        let frac = (pos - idx as f64) as f32;
        for ch in 0..channels {
            let c = ch.min(1);
            let a = stereo48[idx * 2 + c];
            let b = stereo48[next * 2 + c];
            buf.push(a + (b - a) * frac);
        }
    }
    let total = buf.len();
    let pos = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let pos2 = pos.clone();
    let data = std::sync::Arc::new(buf);
    let stream = device
        .build_output_stream(
            config.into(),
            move |out: &mut [f32], _| {
                let start = pos2.load(std::sync::atomic::Ordering::Relaxed);
                for (i, s) in out.iter_mut().enumerate() {
                    *s = data.get(start + i).copied().unwrap_or(0.0);
                }
                pos2.store(start + out.len(), std::sync::atomic::Ordering::Relaxed);
            },
            |e| log::warn!("UI sound stream error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    while pos.load(std::sync::atomic::Ordering::Relaxed) < total {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    Ok(())
}

/// `Sounds`: the two connection beeps, loaded from `<base>/AudioEffects/`.
#[derive(Debug, Clone, Default)]
pub struct UiSounds {
    pub connected: Sound,
    pub disconnected: Sound,
}

impl UiSounds {
    pub fn load(base: &Path) -> Self {
        let dir = base.join("AudioEffects");
        Self {
            connected: Sound::load(&dir.join("beep-connected.wav")),
            disconnected: Sound::load(&dir.join("beep-disconnected.wav")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_resamples_shipped_beeps() {
        let base = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../DCS-SR-Client"));
        if !base.join("AudioEffects/beep-connected.wav").exists() {
            return;
        }
        let s = UiSounds::load(base);
        assert!(s.connected.loaded() && s.disconnected.loaded());
        assert_eq!(s.connected.stereo.len() % 2, 0);
        assert!(s.connected.stereo.iter().any(|v| v.abs() > 0.01));
        assert!(s.connected.stereo.iter().all(|v| v.abs() <= 1.0));
    }

    #[test]
    fn missing_file_is_silent() {
        assert!(!Sound::load(Path::new("/nonexistent/x.wav")).loaded());
    }
}
