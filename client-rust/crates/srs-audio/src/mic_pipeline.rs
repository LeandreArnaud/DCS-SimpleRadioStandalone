//! Microphone pipeline (`AudioManager.WasapiCaptureOnDataAvailable`):
//! native capture → 16 kHz mono i16 → 640-sample frames → VAD (VOX) →
//! Speex denoise → Opus.

use srs_protocol::constants::{MAX_OPUS_FRAME_BYTES, MIC_SAMPLE_RATE, MIC_SEGMENT_FRAMES};

use crate::opus::{OpusEncoder16k, OpusError};
use crate::resample::Downsampler;
use crate::speex::SpeexPreprocessor;
use crate::vad::VoiceDetector;
use crate::volume::rms_db_i16;

/// Settings the C# code polls (`SpeexProcessor.RefreshSettings` every 5 s,
/// VOX settings every frame).
#[derive(Debug, Clone, PartialEq)]
pub struct MicSettings {
    pub denoise: bool,
    pub denoise_attenuation: i32,
    pub vox: bool,
    pub vox_mode: u8,
    pub vox_minimum_db: f64,
}

impl Default for MicSettings {
    fn default() -> Self {
        Self {
            denoise: true,
            denoise_attenuation: -30,
            vox: false,
            vox_mode: 3,
            vox_minimum_db: -59.0,
        }
    }
}

pub struct MicFrame {
    pub opus: [u8; MAX_OPUS_FRAME_BYTES],
    pub len: usize,
    /// VOX decision (`true` when VOX is off).
    pub voice: bool,
    /// Post-Speex RMS in dBFS (`MicMax`).
    pub rms_db: f32,
}

pub struct MicPipeline {
    down: Downsampler,
    frame: [i16; MIC_SEGMENT_FRAMES],
    vad: VoiceDetector,
    vad_mode: u8,
    speex: SpeexPreprocessor,
    encoder: OpusEncoder16k,
    settings: MicSettings,
    applied_denoise: (bool, i32),
}

impl MicPipeline {
    pub fn new(in_rate: u32, channels: u16) -> Result<Self, OpusError> {
        let settings = MicSettings::default();
        let mut speex = SpeexPreprocessor::new(MIC_SEGMENT_FRAMES, MIC_SAMPLE_RATE);
        speex.set_agc(false);
        speex.set_denoise(settings.denoise);
        speex.set_noise_suppress(settings.denoise_attenuation);
        Ok(Self {
            down: Downsampler::new(in_rate, channels),
            frame: [0; MIC_SEGMENT_FRAMES],
            vad: VoiceDetector::new(settings.vox_mode),
            vad_mode: settings.vox_mode,
            speex,
            encoder: OpusEncoder16k::new()?,
            applied_denoise: (settings.denoise, settings.denoise_attenuation),
            settings,
        })
    }

    pub fn set_settings(&mut self, s: &MicSettings) {
        if self.applied_denoise != (s.denoise, s.denoise_attenuation) {
            self.speex.set_denoise(s.denoise);
            self.speex.set_noise_suppress(s.denoise_attenuation);
            self.applied_denoise = (s.denoise, s.denoise_attenuation);
        }
        if self.vad_mode != s.vox_mode {
            self.vad = VoiceDetector::new(s.vox_mode);
            self.vad_mode = s.vox_mode;
        }
        self.settings = s.clone();
    }

    /// Push interleaved f32 samples from the capture callback.
    pub fn push_native(&mut self, interleaved: &[f32]) {
        self.down.push(interleaved);
    }

    /// Next 40 ms frame, encoded. `None` when fewer than 640 samples queued.
    pub fn next_frame(&mut self) -> Option<MicFrame> {
        if !self.down.pop_frame(&mut self.frame) {
            return None;
        }
        // VOX runs on the raw frame, before Speex (C# order).
        let voice = if self.settings.vox {
            self.vad.has_speech_40ms(&self.frame) && rms_db_i16(&self.frame) > self.settings.vox_minimum_db
        } else {
            true
        };
        self.speex.process(&mut self.frame);
        let rms_db = rms_db_i16(&self.frame) as f32;
        let mut opus = [0u8; MAX_OPUS_FRAME_BYTES];
        let len = match self.encoder.encode(&self.frame, &mut opus) {
            Ok(n) => n,
            Err(e) => {
                log::error!("Error encoding Opus! {e}");
                return None;
            }
        };
        Some(MicFrame {
            opus,
            len,
            voice,
            rms_db,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_frames_from_native_audio() {
        let mut p = MicPipeline::new(48_000, 2).unwrap();
        let mut buf = Vec::new();
        for i in 0..4800 {
            let v = (i as f32 * 2.0 * std::f32::consts::PI * 300.0 / 48_000.0).sin() * 0.5;
            buf.push(v);
            buf.push(v);
        }
        p.push_native(&buf);
        let mut frames = 0;
        while let Some(f) = p.next_frame() {
            assert!(f.len > 0 && f.len <= MAX_OPUS_FRAME_BYTES);
            assert!(f.voice);
            frames += 1;
        }
        assert_eq!(frames, 2, "100 ms of audio = 2 frames of 40 ms");
    }
}
