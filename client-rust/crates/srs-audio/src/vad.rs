//! WebRTC voice activity detection, as used for VOX (`AudioManager.cs`):
//! 16 kHz, 20 ms frames, operating mode from the `VOXMode` setting (0..3,
//! default 3 = most aggressive).

use webrtc_vad::{SampleRate, Vad, VadMode};

pub struct VoiceDetector {
    vad: Vad,
}

impl VoiceDetector {
    /// Samples in one 20 ms frame at 16 kHz.
    pub const FRAME_20MS: usize = 320;

    pub fn new(mode: u8) -> Self {
        let mode = match mode {
            0 => VadMode::Quality,
            1 => VadMode::LowBitrate,
            2 => VadMode::Aggressive,
            _ => VadMode::VeryAggressive,
        };
        Self {
            vad: Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, mode),
        }
    }

    /// Speech detected in this 20 ms frame?
    pub fn has_speech_20ms(&mut self, frame: &[i16; Self::FRAME_20MS]) -> bool {
        self.vad.is_voice_segment(frame).unwrap_or(false)
    }

    /// The C# client checks both halves of a 40 ms frame and ORs them.
    pub fn has_speech_40ms(&mut self, frame: &[i16; 640]) -> bool {
        let (a, b) = frame.split_at(Self::FRAME_20MS);
        let a: &[i16; Self::FRAME_20MS] = a.try_into().unwrap();
        let b: &[i16; Self::FRAME_20MS] = b.try_into().unwrap();
        let first = self.has_speech_20ms(a);
        let second = self.has_speech_20ms(b);
        first || second
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_not_speech() {
        let mut v = VoiceDetector::new(3);
        assert!(!v.has_speech_40ms(&[0i16; 640]));
    }
}
