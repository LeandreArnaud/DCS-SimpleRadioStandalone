//! Raw FFI to libopus (the subset used by SRS: `Common/Audio/Opus/API.cs`).
#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_int};

#[repr(C)]
pub struct OpusEncoder {
    _private: [u8; 0],
}

#[repr(C)]
pub struct OpusDecoder {
    _private: [u8; 0],
}

pub const OPUS_OK: c_int = 0;
pub const OPUS_BAD_ARG: c_int = -1;
pub const OPUS_BUFFER_TOO_SMALL: c_int = -2;
pub const OPUS_INTERNAL_ERROR: c_int = -3;
pub const OPUS_INVALID_PACKET: c_int = -4;
pub const OPUS_UNIMPLEMENTED: c_int = -5;
pub const OPUS_INVALID_STATE: c_int = -6;
pub const OPUS_ALLOC_FAIL: c_int = -7;

pub const OPUS_APPLICATION_VOIP: c_int = 2048;
pub const OPUS_APPLICATION_AUDIO: c_int = 2049;
pub const OPUS_APPLICATION_RESTRICTED_LOWDELAY: c_int = 2051;

pub const OPUS_SET_BITRATE_REQUEST: c_int = 4002;
pub const OPUS_GET_BITRATE_REQUEST: c_int = 4003;
pub const OPUS_SET_COMPLEXITY_REQUEST: c_int = 4010;
pub const OPUS_GET_COMPLEXITY_REQUEST: c_int = 4011;
pub const OPUS_SET_INBAND_FEC_REQUEST: c_int = 4012;
pub const OPUS_GET_INBAND_FEC_REQUEST: c_int = 4013;
pub const OPUS_SET_PACKET_LOSS_PERC_REQUEST: c_int = 4014;
pub const OPUS_GET_PACKET_LOSS_PERC_REQUEST: c_int = 4015;
pub const OPUS_RESET_STATE: c_int = 4028;
pub const OPUS_AUTO: c_int = -1000;
pub const OPUS_BITRATE_MAX: c_int = -1;

extern "C" {
    pub fn opus_get_version_string() -> *const c_char;
    pub fn opus_strerror(error: c_int) -> *const c_char;

    pub fn opus_encoder_create(
        fs: i32,
        channels: c_int,
        application: c_int,
        error: *mut c_int,
    ) -> *mut OpusEncoder;
    pub fn opus_encoder_destroy(st: *mut OpusEncoder);
    pub fn opus_encode(
        st: *mut OpusEncoder,
        pcm: *const i16,
        frame_size: c_int,
        data: *mut u8,
        max_data_bytes: i32,
    ) -> i32;
    pub fn opus_encode_float(
        st: *mut OpusEncoder,
        pcm: *const f32,
        frame_size: c_int,
        data: *mut u8,
        max_data_bytes: i32,
    ) -> i32;
    pub fn opus_encoder_ctl(st: *mut OpusEncoder, request: c_int, ...) -> c_int;

    pub fn opus_decoder_create(fs: i32, channels: c_int, error: *mut c_int) -> *mut OpusDecoder;
    pub fn opus_decoder_destroy(st: *mut OpusDecoder);
    pub fn opus_decode(
        st: *mut OpusDecoder,
        data: *const u8,
        len: i32,
        pcm: *mut i16,
        frame_size: c_int,
        decode_fec: c_int,
    ) -> c_int;
    pub fn opus_decode_float(
        st: *mut OpusDecoder,
        data: *const u8,
        len: i32,
        pcm: *mut f32,
        frame_size: c_int,
        decode_fec: c_int,
    ) -> c_int;
    pub fn opus_decoder_ctl(st: *mut OpusDecoder, request: c_int, ...) -> c_int;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string() {
        let v = unsafe { std::ffi::CStr::from_ptr(opus_get_version_string()) }
            .to_str()
            .unwrap();
        assert!(v.starts_with("libopus 1.5.2"), "{v}");
    }
}
