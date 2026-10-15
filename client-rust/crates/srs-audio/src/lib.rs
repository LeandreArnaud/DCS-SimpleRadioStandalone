//! Audio engine for the SRS client.
//!
//! M0 scope: native codec/DSP wrappers (Opus, Speex preprocessor, WebRTC
//! VAD) linked statically, with the exact parameters the C# client uses.

pub mod devices;
pub mod effects;
pub mod engine;
pub mod jitter;
pub mod jitter_iface;
pub mod master;
pub mod mic_pipeline;
pub mod opus;
pub mod radio_mixer;
pub mod remote_client;
pub mod resample;
pub mod sounds;
pub mod speex;
pub mod vad;
pub mod volume;

pub use effects::{EffectCache, EffectSettings, RadioModelFactory};
pub use engine::{shared_engine, AudioEngine, SharedEngine};
pub use jitter::JitterBuffer;
pub use mic_pipeline::{MicFrame, MicPipeline, MicSettings};
pub use opus::{OpusDecoder48k, OpusEncoder16k};
pub use remote_client::{ClientAudio, IncomingAudioSettings, RemoteClientAudio};
pub use sounds::{Sound, UiSounds};
pub use speex::SpeexPreprocessor;
pub use vad::VoiceDetector;
