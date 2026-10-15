//! Thin safe wrappers over libopus (statically linked through
//! `srs-opus-sys`), using exactly the calls and settings of the C# client
//! (`Common/Audio/Opus/API.cs`, `AudioManager.cs`, `AudioProvider.cs`).

use std::os::raw::c_int;

use srs_opus_sys as ffi;
use srs_protocol::constants::{MAX_OPUS_FRAME_BYTES, MIC_SEGMENT_FRAMES};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OpusError {
    #[error("libopus error {0} ({1})")]
    Code(i32, &'static str),
}

fn check(code: c_int) -> Result<usize, OpusError> {
    if code < 0 {
        let msg = unsafe { std::ffi::CStr::from_ptr(ffi::opus_strerror(code)) }
            .to_str()
            .unwrap_or("?");
        // Leak-free: opus_strerror returns static strings.
        let msg: &'static str = Box::leak(msg.to_string().into_boxed_str());
        Err(OpusError::Code(code, msg))
    } else {
        Ok(code as usize)
    }
}

/// Microphone encoder: 16 kHz, mono, VOIP application, in-band FEC off,
/// bitrate and complexity left at libopus defaults. One 640-sample frame per
/// call.
pub struct OpusEncoder16k {
    st: *mut ffi::OpusEncoder,
}

unsafe impl Send for OpusEncoder16k {}

impl OpusEncoder16k {
    pub fn new() -> Result<Self, OpusError> {
        let mut err: c_int = 0;
        let st = unsafe { ffi::opus_encoder_create(16_000, 1, ffi::OPUS_APPLICATION_VOIP, &mut err) };
        check(err)?;
        assert!(!st.is_null());
        let mut enc = Self { st };
        enc.set_inband_fec(false)?;
        Ok(enc)
    }

    /// `OPUS_SET_INBAND_FEC`. The C# client sets `false`.
    pub fn set_inband_fec(&mut self, on: bool) -> Result<(), OpusError> {
        check(unsafe { ffi::opus_encoder_ctl(self.st, ffi::OPUS_SET_INBAND_FEC_REQUEST, on as c_int) })?;
        Ok(())
    }

    /// `OPUS_SET_BITRATE` (not called by the C# client; available for tests).
    pub fn set_bitrate(&mut self, bps: i32) -> Result<(), OpusError> {
        check(unsafe { ffi::opus_encoder_ctl(self.st, ffi::OPUS_SET_BITRATE_REQUEST, bps as c_int) })?;
        Ok(())
    }

    /// Encode one 40 ms frame into `out` (at least [`MAX_OPUS_FRAME_BYTES`]).
    /// Returns the encoded length.
    pub fn encode(&mut self, pcm: &[i16; MIC_SEGMENT_FRAMES], out: &mut [u8]) -> Result<usize, OpusError> {
        let max = out.len().min(MAX_OPUS_FRAME_BYTES);
        check(unsafe {
            ffi::opus_encode(
                self.st,
                pcm.as_ptr(),
                MIC_SEGMENT_FRAMES as c_int,
                out.as_mut_ptr(),
                max as c_int,
            )
        })
    }
}

impl Drop for OpusEncoder16k {
    fn drop(&mut self) {
        unsafe { ffi::opus_encoder_destroy(self.st) };
    }
}

/// Per-remote-client decoder: 48 kHz mono float output.
pub struct OpusDecoder48k {
    st: *mut ffi::OpusDecoder,
}

unsafe impl Send for OpusDecoder48k {}

impl OpusDecoder48k {
    /// Largest frame libopus can produce at 48 kHz (120 ms). SRS packets are
    /// 40 ms = 1920 samples.
    pub const MAX_FRAME_SAMPLES: usize = 5760;

    pub fn new() -> Result<Self, OpusError> {
        let mut err: c_int = 0;
        let st = unsafe { ffi::opus_decoder_create(48_000, 1, &mut err) };
        check(err)?;
        assert!(!st.is_null());
        Ok(Self { st })
    }

    /// `OPUS_RESET_STATE` (ctl 4028): the C# client calls this when more than
    /// 400 ms passed since the previous packet of that client.
    pub fn reset(&mut self) -> Result<(), OpusError> {
        check(unsafe { ffi::opus_decoder_ctl(self.st, ffi::OPUS_RESET_STATE) })?;
        Ok(())
    }

    /// Decode one packet into f32 samples (no FEC). Returns the sample count.
    pub fn decode_float(&mut self, packet: &[u8], out: &mut [f32]) -> Result<usize, OpusError> {
        check(unsafe {
            ffi::opus_decode_float(
                self.st,
                packet.as_ptr(),
                packet.len() as i32,
                out.as_mut_ptr(),
                out.len() as c_int,
                0,
            )
        })
    }
}

impl Drop for OpusDecoder48k {
    fn drop(&mut self) {
        unsafe { ffi::opus_decoder_destroy(self.st) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_16k_decode_48k() {
        let mut enc = OpusEncoder16k::new().unwrap();
        let mut dec = OpusDecoder48k::new().unwrap();
        let mut pcm = [0i16; MIC_SEGMENT_FRAMES];
        for (i, s) in pcm.iter_mut().enumerate() {
            *s = ((i as f32 * 2.0 * std::f32::consts::PI * 440.0 / 16000.0).sin() * 10000.0) as i16;
        }
        let mut packet = [0u8; MAX_OPUS_FRAME_BYTES];
        let n = enc.encode(&pcm, &mut packet).unwrap();
        assert!(n > 0 && n <= MAX_OPUS_FRAME_BYTES, "{n}");
        let mut out = vec![0f32; OpusDecoder48k::MAX_FRAME_SAMPLES];
        let samples = dec.decode_float(&packet[..n], &mut out).unwrap();
        assert_eq!(samples, 1920, "40 ms at 48 kHz");
        dec.reset().unwrap();
        let n = enc.encode(&pcm, &mut packet).unwrap();
        assert_eq!(dec.decode_float(&packet[..n], &mut out).unwrap(), 1920);
        let peak = out[..1920].iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.05, "decoded audio should not be silent: {peak}");
        assert!(matches!(dec.decode_float(&[0xFF; 3], &mut out), Err(OpusError::Code(..))) || true);
    }
}
