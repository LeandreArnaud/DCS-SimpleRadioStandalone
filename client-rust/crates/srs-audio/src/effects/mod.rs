//! Radio effects (M5): DSP primitives, radio models, cached WAV effects, the
//! per-transmission TX pipeline and the settings that drive them.

pub mod cache;
pub mod dsp;
pub mod model;
pub mod pipeline;
pub mod tones;

pub use cache::{CachedEffect, EffectCache, EffectType};
pub use model::{RadioModelFactory, RadioModelSpec, RxModel, TxModel};
pub use pipeline::{TxInput, TxPipeline};

/// Profile/server settings read every 3 s by the C# providers
/// (`ClientTransmissionPipelineProvider`, `ClientEffectsPipeline`,
/// `RadioMixingProvider`). Defaults are the profile defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectSettings {
    pub per_radio_model_effects: bool,
    /// `RadioEffectsRatio` (0..1, clamped on use).
    pub radio_effects_ratio: f32,
    pub radio_encryption_effects: bool,
    pub clipping: bool,
    pub background_noise: bool,
    pub noise_gain_db: f32,
    pub hf_noise_gain_db: f32,
    pub nato_tone: bool,
    pub nato_tone_volume: f32,
    pub havequick_tone: bool,
    pub hq_tone_volume: f32,
    pub rx_effects_start: bool,
    pub rx_effects_end: bool,
    pub tx_effects_start: bool,
    pub tx_effects_end: bool,
    pub mids_radio_effect: bool,
    pub radio_start_selection: String,
    pub radio_end_selection: String,
    pub intercom_start_selection: String,
    pub intercom_end_selection: String,
    /// Server setting `IRL_RADIO_RX_INTERFERENCE` (FM capture).
    pub irl_radio_rx_interference: bool,
}

impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            per_radio_model_effects: true,
            radio_effects_ratio: 1.0,
            radio_encryption_effects: true,
            clipping: false,
            background_noise: true,
            noise_gain_db: 0.0,
            hf_noise_gain_db: 0.0,
            nato_tone: true,
            nato_tone_volume: 1.2,
            havequick_tone: true,
            hq_tone_volume: 0.3,
            rx_effects_start: true,
            rx_effects_end: true,
            tx_effects_start: true,
            tx_effects_end: true,
            mids_radio_effect: true,
            radio_start_selection: "RADIO_TRANS_START.wav".into(),
            radio_end_selection: "RADIO_TRANS_END.wav".into(),
            intercom_start_selection: String::new(),
            intercom_end_selection: String::new(),
            irl_radio_rx_interference: false,
        }
    }
}
