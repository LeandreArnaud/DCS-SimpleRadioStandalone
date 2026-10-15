//! WAV effect cache: `CachedAudioEffect` / `CachedAudioEffectProvider`.
//! Files live in `<exe dir>/AudioEffects/` (and `Ambient/`), 48 kHz, 16-bit,
//! mono; anything else is ignored like the C# code does.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use srs_protocol::constants::OUTPUT_SAMPLE_RATE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    RadioTransStart,
    RadioTransEnd,
    Ky58Tx,
    Ky58Rx,
    NatoTone,
    MidsTx,
    MidsTxEnd,
    HaveQuickTone,
    IntercomTransStart,
    IntercomTransEnd,
    AmbientCockpit,
}

impl EffectType {
    /// File stem as the C# enum name.
    pub const fn stem(self) -> &'static str {
        match self {
            EffectType::RadioTransStart => "RADIO_TRANS_START",
            EffectType::RadioTransEnd => "RADIO_TRANS_END",
            EffectType::Ky58Tx => "KY_58_TX",
            EffectType::Ky58Rx => "KY_58_RX",
            EffectType::NatoTone => "NATO_TONE",
            EffectType::MidsTx => "MIDS_TX",
            EffectType::MidsTxEnd => "MIDS_TX_END",
            EffectType::HaveQuickTone => "HAVEQUICK_TONE",
            EffectType::IntercomTransStart => "INTERCOM_TRANS_START",
            EffectType::IntercomTransEnd => "INTERCOM_TRANS_END",
            EffectType::AmbientCockpit => "AMBIENT_COCKPIT",
        }
    }
}

/// A loaded effect (`CachedAudioEffect`). `samples` is empty when the file
/// is missing or has the wrong format (`Loaded == false`).
#[derive(Debug, Clone, Default)]
pub struct CachedEffect {
    pub file_name: String,
    pub samples: std::sync::Arc<Vec<f32>>,
    /// RMS in dBFS (`-96.6` for silence).
    pub rms_db: f64,
}

impl CachedEffect {
    pub fn loaded(&self) -> bool {
        !self.samples.is_empty()
    }

    pub fn load(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let mut effect = Self {
            file_name,
            samples: Default::default(),
            rms_db: -96.6,
        };
        let reader = match hound::WavReader::open(path) {
            Ok(r) => r,
            Err(e) => {
                log::info!("Unable to find file for effect {}: {e}", path.display());
                return effect;
            }
        };
        let spec = reader.spec();
        if spec.sample_rate != OUTPUT_SAMPLE_RATE || spec.channels != 1 || spec.bits_per_sample != 16 {
            log::info!(
                "Unable to read Effect {} - {} Hz {} ch {} bit is not 48000 Hz 1 ch 16 bit",
                path.display(),
                spec.sample_rate,
                spec.channels,
                spec.bits_per_sample
            );
            return effect;
        }
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .filter_map(Result::ok)
            .map(|s| f32::from(s) / 32768.0)
            .collect();
        effect.rms_db = crate::volume::rms_db_f32(&samples);
        effect.samples = std::sync::Arc::new(samples);
        log::info!(
            "Read Effect {} ({} samples)",
            path.display(),
            effect.samples.len()
        );
        effect
    }
}

/// All effects known to the client (`CachedAudioEffectProvider`).
#[derive(Debug, Default)]
pub struct EffectCache {
    pub radio_start: Vec<CachedEffect>,
    pub radio_end: Vec<CachedEffect>,
    pub intercom_start: Vec<CachedEffect>,
    pub intercom_end: Vec<CachedEffect>,
    pub ky58_tx: CachedEffect,
    pub ky58_rx: CachedEffect,
    pub nato_tone: CachedEffect,
    pub mids_tx: CachedEffect,
    pub mids_end: CachedEffect,
    pub havequick_tone: CachedEffect,
    ambient: HashMap<String, CachedEffect>,
}

impl EffectCache {
    /// Load from `<base>/AudioEffects/`.
    pub fn load(base: &Path) -> Self {
        let folder = base.join("AudioEffects");
        let mut c = Self::default();
        if let Ok(entries) = std::fs::read_dir(&folder) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            for path in files {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let target = if name.starts_with("radio_trans_start") {
                    &mut c.radio_start
                } else if name.starts_with("radio_trans_end") {
                    &mut c.radio_end
                } else if name.starts_with("intercom_trans_start") {
                    &mut c.intercom_start
                } else if name.starts_with("intercom_trans_end") {
                    &mut c.intercom_end
                } else {
                    continue;
                };
                let e = CachedEffect::load(&path);
                if e.loaded() {
                    target.push(e);
                }
            }
        } else {
            log::warn!("AudioEffects folder not found at {}", folder.display());
        }
        let single = |t: EffectType| CachedEffect::load(&folder.join(format!("{}.wav", t.stem())));
        c.ky58_tx = single(EffectType::Ky58Tx);
        c.ky58_rx = single(EffectType::Ky58Rx);
        c.nato_tone = single(EffectType::NatoTone);
        c.mids_tx = single(EffectType::MidsTx);
        c.mids_end = single(EffectType::MidsTxEnd);
        c.havequick_tone = single(EffectType::HaveQuickTone);
        // `GetAmbientEffect` loads lazily from the audio thread in C#; here every
        // `Ambient/*.wav` is read once at startup so the render thread never touches disk.
        if let Ok(entries) = std::fs::read_dir(folder.join("Ambient")) {
            for path in entries.flatten().map(|e| e.path()).filter(|p| p.is_file()) {
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("wav"))
                    != Some(true)
                {
                    continue;
                }
                let e = CachedEffect::load(&path);
                if e.loaded() {
                    c.ambient.insert(stem.to_ascii_lowercase(), e);
                }
            }
        }
        c
    }

    /// First effect whose file name matches (case-insensitive), else the
    /// first loaded one (C# falls back to the first list entry).
    fn select<'a>(list: &'a [CachedEffect], file_name: &str) -> Option<&'a CachedEffect> {
        list.iter()
            .find(|e| e.file_name.eq_ignore_ascii_case(file_name))
            .or_else(|| list.first())
    }

    /// `SelectedRadioTransmissionStartEffect` for the profile's file name.
    pub fn radio_start(&self, selection: &str) -> Option<&CachedEffect> {
        Self::select(&self.radio_start, selection)
    }

    pub fn radio_end(&self, selection: &str) -> Option<&CachedEffect> {
        Self::select(&self.radio_end, selection)
    }

    pub fn intercom_start(&self, selection: &str) -> Option<&CachedEffect> {
        Self::select(&self.intercom_start, selection)
    }

    pub fn intercom_end(&self, selection: &str) -> Option<&CachedEffect> {
        Self::select(&self.intercom_end, selection)
    }

    /// Test hook: register an in-memory ambient loop.
    pub fn insert_ambient_for_test(&mut self, ab_type: &str, samples: Vec<f32>) {
        let rms_db = crate::volume::rms_db_f32(&samples);
        self.ambient.insert(
            ab_type.to_ascii_lowercase(),
            CachedEffect {
                file_name: format!("{ab_type}.wav"),
                samples: std::sync::Arc::new(samples),
                rms_db,
            },
        );
    }

    /// `GetAmbientEffect`: `AudioEffects/Ambient/<abType>.wav` (case-insensitive).
    pub fn ambient(&self, ab_type: &str) -> Option<&CachedEffect> {
        self.ambient
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(ab_type))
            .map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_shipped_effects() {
        let base = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../DCS-SR-Client"));
        if !base.join("AudioEffects").exists() {
            eprintln!("AudioEffects not found, skipping");
            return;
        }
        let c = EffectCache::load(base);
        assert!(c.nato_tone.loaded(), "NATO_TONE.wav");
        assert!(c.havequick_tone.loaded());
        assert!(c.ky58_tx.loaded());
        assert!(c
            .radio_start("RADIO_TRANS_START.wav")
            .is_some_and(CachedEffect::loaded));
        assert_eq!(
            c.radio_start("RADIO_TRANS_START_ALTERNATE.wav")
                .unwrap()
                .file_name,
            "RADIO_TRANS_START_ALTERNATE.wav"
        );
        assert_eq!(
            c.radio_start("nope.wav").unwrap().file_name,
            c.radio_start[0].file_name
        );
        assert!(c.radio_start.len() >= 2);
        assert_eq!(
            c.ambient("uh1").is_some(),
            base.join("AudioEffects/Ambient/uh1.wav").exists()
        );
        assert!(
            c.nato_tone.rms_db < 0.0 && c.nato_tone.rms_db > -60.0,
            "{}",
            c.nato_tone.rms_db
        );
    }
}
