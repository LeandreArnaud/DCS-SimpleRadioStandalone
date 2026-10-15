//! Mirror of `Common/Constants.cs`.

/// Microphone pipeline sample rate (Hz). Opus encoder input.
pub const MIC_SAMPLE_RATE: u32 = 16_000;
/// Length of one microphone frame (ms).
pub const MIC_INPUT_AUDIO_LENGTH_MS: u32 = 40;
/// Samples per microphone frame: 16 kHz * 40 ms.
pub const MIC_SEGMENT_FRAMES: usize = (MIC_SAMPLE_RATE / 1000 * MIC_INPUT_AUDIO_LENGTH_MS) as usize;

/// Output / decoding sample rate (Hz).
pub const OUTPUT_SAMPLE_RATE: u32 = 48_000;
/// Length of one output segment (ms).
pub const OUTPUT_AUDIO_LENGTH_MS: u32 = 40;
/// Samples per output segment: 48 kHz * 40 ms. Also the jitter buffer step.
pub const OUTPUT_SEGMENT_FRAMES: usize = (OUTPUT_SAMPLE_RATE / 1000 * OUTPUT_AUDIO_LENGTH_MS) as usize;

/// Declared in C# but unused there as well (ms).
pub const JITTER_BUFFER_MS: u32 = 50;

/// Ten radios plus the intercom. `RadioInfo.radios` always has this length.
pub const MAX_RADIOS: usize = 11;

/// Resend the full client state even without change after this many seconds.
pub const CLIENT_UPDATE_INTERVAL_LIMIT_SECS: u64 = 180;

/// Default SRS server port (TCP and UDP).
pub const DEFAULT_SERVER_PORT: u16 = 5002;

/// Maximum size of one encoded Opus frame the C# client passes to the encoder.
pub const MAX_OPUS_FRAME_BYTES: usize = 1280;
