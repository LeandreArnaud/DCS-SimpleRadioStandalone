//! Raw FFI to the speexdsp preprocessor. Statically linked, no DLL.
//!
//! Only the functions and control codes used by the SRS client are exposed
//! (`Common/Audio/Utility/Speex/DspNative.cs` and `Preprocessor.cs`).
#![allow(non_camel_case_types)]

use std::os::raw::{c_int, c_void};

pub type spx_int16_t = i16;
pub type spx_int32_t = i32;

/// Opaque `SpeexPreprocessState`.
#[repr(C)]
pub struct SpeexPreprocessState {
    _private: [u8; 0],
}

// Control codes from `speex/speex_preprocess.h`.
pub const SPEEX_PREPROCESS_SET_DENOISE: c_int = 0;
pub const SPEEX_PREPROCESS_GET_DENOISE: c_int = 1;
pub const SPEEX_PREPROCESS_SET_AGC: c_int = 2;
pub const SPEEX_PREPROCESS_GET_AGC: c_int = 3;
pub const SPEEX_PREPROCESS_SET_VAD: c_int = 4;
pub const SPEEX_PREPROCESS_GET_VAD: c_int = 5;
pub const SPEEX_PREPROCESS_SET_AGC_LEVEL: c_int = 6;
pub const SPEEX_PREPROCESS_GET_AGC_LEVEL: c_int = 7;
pub const SPEEX_PREPROCESS_SET_DEREVERB: c_int = 8;
pub const SPEEX_PREPROCESS_GET_DEREVERB: c_int = 9;
pub const SPEEX_PREPROCESS_SET_PROB_START: c_int = 14;
pub const SPEEX_PREPROCESS_GET_PROB_START: c_int = 15;
pub const SPEEX_PREPROCESS_SET_PROB_CONTINUE: c_int = 16;
pub const SPEEX_PREPROCESS_GET_PROB_CONTINUE: c_int = 17;
pub const SPEEX_PREPROCESS_SET_NOISE_SUPPRESS: c_int = 18;
pub const SPEEX_PREPROCESS_GET_NOISE_SUPPRESS: c_int = 19;
pub const SPEEX_PREPROCESS_SET_ECHO_SUPPRESS: c_int = 20;
pub const SPEEX_PREPROCESS_GET_ECHO_SUPPRESS: c_int = 21;
pub const SPEEX_PREPROCESS_SET_ECHO_SUPPRESS_ACTIVE: c_int = 22;
pub const SPEEX_PREPROCESS_GET_ECHO_SUPPRESS_ACTIVE: c_int = 23;
pub const SPEEX_PREPROCESS_SET_ECHO_STATE: c_int = 24;
pub const SPEEX_PREPROCESS_GET_ECHO_STATE: c_int = 25;
pub const SPEEX_PREPROCESS_SET_AGC_INCREMENT: c_int = 26;
pub const SPEEX_PREPROCESS_GET_AGC_INCREMENT: c_int = 27;
pub const SPEEX_PREPROCESS_SET_AGC_DECREMENT: c_int = 28;
pub const SPEEX_PREPROCESS_GET_AGC_DECREMENT: c_int = 29;
pub const SPEEX_PREPROCESS_SET_AGC_MAX_GAIN: c_int = 30;
pub const SPEEX_PREPROCESS_GET_AGC_MAX_GAIN: c_int = 31;
pub const SPEEX_PREPROCESS_GET_AGC_LOUDNESS: c_int = 33;
pub const SPEEX_PREPROCESS_GET_AGC_GAIN: c_int = 35;
pub const SPEEX_PREPROCESS_GET_PSD_SIZE: c_int = 37;
pub const SPEEX_PREPROCESS_GET_PSD: c_int = 39;
pub const SPEEX_PREPROCESS_GET_NOISE_PSD_SIZE: c_int = 41;
pub const SPEEX_PREPROCESS_GET_NOISE_PSD: c_int = 43;
pub const SPEEX_PREPROCESS_GET_PROB: c_int = 45;
pub const SPEEX_PREPROCESS_SET_AGC_TARGET: c_int = 46;
pub const SPEEX_PREPROCESS_GET_AGC_TARGET: c_int = 47;

extern "C" {
    pub fn speex_preprocess_state_init(frame_size: c_int, sampling_rate: c_int) -> *mut SpeexPreprocessState;
    pub fn speex_preprocess_state_destroy(st: *mut SpeexPreprocessState);
    /// Returns 1 when speech is detected (only meaningful with VAD on).
    pub fn speex_preprocess_run(st: *mut SpeexPreprocessState, x: *mut spx_int16_t) -> c_int;
    pub fn speex_preprocess_ctl(st: *mut SpeexPreprocessState, request: c_int, ptr: *mut c_void) -> c_int;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_run_destroy() {
        unsafe {
            let st = speex_preprocess_state_init(640, 16000);
            assert!(!st.is_null());
            let mut on: c_int = 1;
            assert_eq!(
                speex_preprocess_ctl(st, SPEEX_PREPROCESS_SET_DENOISE, &mut on as *mut _ as *mut c_void),
                0
            );
            let mut att: c_int = -30;
            assert_eq!(
                speex_preprocess_ctl(
                    st,
                    SPEEX_PREPROCESS_SET_NOISE_SUPPRESS,
                    &mut att as *mut _ as *mut c_void
                ),
                0
            );
            let mut frame = [0i16; 640];
            for (i, s) in frame.iter_mut().enumerate() {
                *s = ((i as f32 * 0.3).sin() * 8000.0) as i16;
            }
            speex_preprocess_run(st, frame.as_mut_ptr());
            speex_preprocess_state_destroy(st);
        }
    }
}
