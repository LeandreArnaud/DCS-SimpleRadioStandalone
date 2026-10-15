//! Safe wrapper over the speexdsp preprocessor, mirroring
//! `Common/Audio/Utility/Speex/Preprocessor.cs`.

use std::os::raw::{c_int, c_void};

use speexdsp_preprocess_sys as ffi;

pub struct SpeexPreprocessor {
    st: *mut ffi::SpeexPreprocessState,
    frame_size: usize,
}

// The state is only ever touched from the thread that owns the wrapper.
unsafe impl Send for SpeexPreprocessor {}

impl SpeexPreprocessor {
    /// `frame_size` samples per `process` call at `sample_rate` Hz. The C#
    /// client uses (640, 16000) for the microphone and (480, 48000) for
    /// incoming audio.
    pub fn new(frame_size: usize, sample_rate: u32) -> Self {
        let st = unsafe { ffi::speex_preprocess_state_init(frame_size as c_int, sample_rate as c_int) };
        assert!(!st.is_null(), "speex_preprocess_state_init failed");
        Self { st, frame_size }
    }

    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    fn ctl_int(&mut self, request: c_int, value: i32) {
        let mut v: c_int = value;
        let r = unsafe { ffi::speex_preprocess_ctl(self.st, request, &mut v as *mut c_int as *mut c_void) };
        debug_assert_eq!(r, 0, "speex_preprocess_ctl({request}) failed");
    }

    fn get_int(&mut self, request: c_int) -> i32 {
        let mut v: c_int = 0;
        let r = unsafe { ffi::speex_preprocess_ctl(self.st, request, &mut v as *mut c_int as *mut c_void) };
        debug_assert_eq!(r, 0, "speex_preprocess_ctl({request}) failed");
        v
    }

    fn ctl_float(&mut self, request: c_int, value: f32) {
        let mut v: f32 = value;
        let r = unsafe { ffi::speex_preprocess_ctl(self.st, request, &mut v as *mut f32 as *mut c_void) };
        debug_assert_eq!(r, 0, "speex_preprocess_ctl({request}) failed");
    }

    pub fn set_denoise(&mut self, on: bool) {
        self.ctl_int(ffi::SPEEX_PREPROCESS_SET_DENOISE, on as i32);
    }

    pub fn denoise(&mut self) -> bool {
        self.get_int(ffi::SPEEX_PREPROCESS_GET_DENOISE) != 0
    }

    /// Maximum attenuation in dB (negative), e.g. `-30`.
    pub fn set_noise_suppress(&mut self, db: i32) {
        self.ctl_int(ffi::SPEEX_PREPROCESS_SET_NOISE_SUPPRESS, db);
    }

    pub fn set_agc(&mut self, on: bool) {
        self.ctl_int(ffi::SPEEX_PREPROCESS_SET_AGC, on as i32);
    }

    pub fn agc(&mut self) -> bool {
        self.get_int(ffi::SPEEX_PREPROCESS_GET_AGC) != 0
    }

    /// `SPEEX_PREPROCESS_SET_AGC_TARGET` (int). The C# client uses 14000.
    pub fn set_agc_target(&mut self, target: i32) {
        self.ctl_int(ffi::SPEEX_PREPROCESS_SET_AGC_TARGET, target);
    }

    /// `SPEEX_PREPROCESS_SET_AGC_LEVEL` (float, 1..32768).
    pub fn set_agc_level(&mut self, level: f32) {
        self.ctl_float(ffi::SPEEX_PREPROCESS_SET_AGC_LEVEL, level);
    }

    pub fn set_agc_increment(&mut self, db_per_second: i32) {
        self.ctl_int(ffi::SPEEX_PREPROCESS_SET_AGC_INCREMENT, db_per_second);
    }

    /// Negative dB per second, e.g. `-60`.
    pub fn set_agc_decrement(&mut self, db_per_second: i32) {
        self.ctl_int(ffi::SPEEX_PREPROCESS_SET_AGC_DECREMENT, db_per_second);
    }

    /// dB, e.g. `40`.
    pub fn set_agc_max_gain(&mut self, db: i32) {
        self.ctl_int(ffi::SPEEX_PREPROCESS_SET_AGC_MAX_GAIN, db);
    }

    /// Process one frame in place. Returns the VAD decision (only meaningful
    /// when VAD is enabled, which SRS never does).
    pub fn process(&mut self, frame: &mut [i16]) -> bool {
        assert_eq!(frame.len(), self.frame_size, "speex frame size mismatch");
        unsafe { ffi::speex_preprocess_run(self.st, frame.as_mut_ptr()) != 0 }
    }
}

impl Drop for SpeexPreprocessor {
    fn drop(&mut self) {
        unsafe { ffi::speex_preprocess_state_destroy(self.st) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mic_configuration_like_csharp() {
        let mut p = SpeexPreprocessor::new(640, 16000);
        p.set_agc(false);
        p.set_denoise(true);
        p.set_noise_suppress(-30);
        assert!(!p.agc());
        assert!(p.denoise());
        let mut frame = [0i16; 640];
        for (i, s) in frame.iter_mut().enumerate() {
            *s = ((i as f32 * 0.7).sin() * 3000.0) as i16;
        }
        p.process(&mut frame);
    }

    #[test]
    fn incoming_configuration_like_csharp() {
        let mut p = SpeexPreprocessor::new(480, 48000);
        p.set_agc(true);
        p.set_agc_target(14000);
        p.set_agc_decrement(-60);
        p.set_agc_max_gain(40);
        p.set_denoise(true);
        p.set_noise_suppress(-30);
        assert!(p.agc());
        let mut frame = [100i16; 480];
        p.process(&mut frame);
    }
}
