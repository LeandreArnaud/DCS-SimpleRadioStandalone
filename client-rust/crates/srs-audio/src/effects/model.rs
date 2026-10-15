//! Radio models: the JSON description (`RadioModels/*.json`, `$type`
//! discriminated effects), the built-in ARC-210 and intercom defaults, and
//! the runtime effect chains built from them (`Common/Audio/Models/RadioModel.cs`,
//! `JsonConverter.cs`).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use srs_protocol::constants::OUTPUT_SAMPLE_RATE;

use super::dsp::{self, BiQuad, Compressor, Cvsd, Filter, FirstOrder};

const SR: f64 = OUTPUT_SAMPLE_RATE as f64;

/// A filter entry: `q` present → biquad, absent → first order; `peak`
/// requires `q` and `gain`.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterSpec {
    LowPass { frequency: f32, q: Option<f32> },
    HighPass { frequency: f32, q: Option<f32> },
    Peak { frequency: f32, q: f32, gain: f32 },
}

impl FilterSpec {
    pub fn build(&self) -> Box<dyn Filter> {
        match *self {
            FilterSpec::LowPass {
                frequency,
                q: Some(q),
            } => Box::new(BiQuad::low_pass(SR, f64::from(frequency), f64::from(q))),
            FilterSpec::LowPass { frequency, q: None } => {
                Box::new(FirstOrder::low_pass(SR, f64::from(frequency)))
            }
            FilterSpec::HighPass {
                frequency,
                q: Some(q),
            } => Box::new(BiQuad::high_pass(SR, f64::from(frequency), f64::from(q))),
            FilterSpec::HighPass { frequency, q: None } => {
                Box::new(FirstOrder::high_pass(SR, f64::from(frequency)))
            }
            FilterSpec::Peak { frequency, q, gain } => Box::new(BiQuad::peaking_eq(
                SR,
                f64::from(frequency),
                f64::from(q),
                f64::from(gain),
            )),
        }
    }
}

impl<'de> Deserialize<'de> for FilterSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            #[serde(rename = "$type")]
            kind: String,
            frequency: Option<f32>,
            q: Option<f32>,
            gain: Option<f32>,
        }
        let r = Raw::deserialize(d)?;
        let frequency = r
            .frequency
            .ok_or_else(|| serde::de::Error::custom("Filter must have a frequency set."))?;
        match r.kind.as_str() {
            "lowpass" => Ok(FilterSpec::LowPass { frequency, q: r.q }),
            "highpass" => Ok(FilterSpec::HighPass { frequency, q: r.q }),
            "peak" => match (r.q, r.gain) {
                (Some(q), Some(gain)) => Ok(FilterSpec::Peak { frequency, q, gain }),
                (None, _) => Err(serde::de::Error::custom("peak must have a q property set.")),
                (_, None) => Err(serde::de::Error::custom("peak must have a gain property set.")),
            },
            other => Err(serde::de::Error::custom(format!("unknown filter type {other}"))),
        }
    }
}

/// `IEffect` tree.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "$type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EffectSpec {
    Chain {
        effects: Vec<EffectSpec>,
    },
    Filters {
        filters: Vec<FilterSpec>,
    },
    Saturation {
        gain: f32,
        threshold: f32,
    },
    Compressor {
        attack: f32,
        make_up: f32,
        release: f32,
        threshold: f32,
        ratio: f32,
    },
    SidechainCompressor {
        attack: f32,
        make_up: f32,
        release: f32,
        threshold: f32,
        ratio: f32,
        sidechain_effect: Box<EffectSpec>,
    },
    Gain {
        gain: f32,
    },
    Cvsd,
}

/// `RadioModel` DTO (camelCase JSON).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioModelSpec {
    pub version: i32,
    pub noise_gain: f32,
    pub tx_effect: EffectSpec,
    pub rx_effect: EffectSpec,
    #[serde(default)]
    pub encryption_effect: Option<EffectSpec>,
}

/// A stateful, instantiated effect.
pub enum Effect {
    Chain(Vec<Effect>),
    Filters(Vec<Box<dyn Filter>>),
    Saturation {
        gain_db: f32,
        threshold_db: f32,
    },
    /// NAudio `SimpleCompressorEffect`: sidechain = the signal itself.
    Compressor(Compressor),
    SidechainCompressor {
        compressor: Compressor,
        sidechain: Box<Effect>,
        scratch: Vec<f32>,
    },
    Gain(f32),
    Cvsd(Cvsd),
}

impl Effect {
    pub fn from_spec(spec: &EffectSpec) -> Self {
        match spec {
            EffectSpec::Chain { effects } => Effect::Chain(effects.iter().map(Effect::from_spec).collect()),
            EffectSpec::Filters { filters } => {
                Effect::Filters(filters.iter().map(FilterSpec::build).collect())
            }
            EffectSpec::Saturation { gain, threshold } => Effect::Saturation {
                gain_db: *gain,
                threshold_db: *threshold,
            },
            EffectSpec::Compressor {
                attack,
                make_up,
                release,
                threshold,
                ratio,
            } => Effect::Compressor(Compressor::new(
                f64::from(*attack) * 1000.0,
                f64::from(*release) * 1000.0,
                SR,
                f64::from(*threshold),
                f64::from(*ratio),
                f64::from(*make_up),
            )),
            EffectSpec::SidechainCompressor {
                attack,
                make_up,
                release,
                threshold,
                ratio,
                sidechain_effect,
            } => Effect::SidechainCompressor {
                compressor: Compressor::new(
                    f64::from(*attack) * 1000.0,
                    f64::from(*release) * 1000.0,
                    SR,
                    f64::from(*threshold),
                    f64::from(*ratio),
                    f64::from(*make_up),
                ),
                sidechain: Box::new(Effect::from_spec(sidechain_effect)),
                scratch: Vec::new(),
            },
            EffectSpec::Gain { gain } => Effect::Gain(dsp::db_to_linear(f64::from(*gain)) as f32),
            EffectSpec::Cvsd => Effect::Cvsd(Cvsd::new()),
        }
    }

    /// Process in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        match self {
            Effect::Chain(effects) => {
                for e in effects {
                    e.process(buf);
                }
            }
            Effect::Filters(filters) => dsp::apply_filters(filters, buf),
            Effect::Saturation {
                gain_db,
                threshold_db,
            } => dsp::saturate(buf, *gain_db, *threshold_db),
            Effect::Compressor(c) => {
                for s in buf.iter_mut() {
                    *s = c.process(f64::from(*s), f64::from(*s)) as f32;
                }
            }
            Effect::SidechainCompressor {
                compressor,
                sidechain,
                scratch,
            } => {
                scratch.clear();
                scratch.extend_from_slice(buf);
                sidechain.process(scratch);
                for (s, side) in buf.iter_mut().zip(scratch.iter()) {
                    *s = compressor.process(f64::from(*side), f64::from(*s)) as f32;
                }
            }
            Effect::Gain(g) => dsp::gain(buf, *g),
            Effect::Cvsd(c) => {
                for s in buf.iter_mut() {
                    *s = c.transform(*s);
                }
            }
        }
    }
}

/// Instantiated TX side of a model (`TxRadioModel`): the TX effect and,
/// when encryption is on, the TX effect followed by the encryption effect.
pub struct TxModel {
    pub tx_effect: Effect,
    pub encryption_effect: Option<Effect>,
    pub noise_gain: f32,
}

impl TxModel {
    pub fn new(spec: &RadioModelSpec) -> Self {
        Self {
            tx_effect: Effect::from_spec(&spec.tx_effect),
            encryption_effect: spec.encryption_effect.as_ref().map(Effect::from_spec),
            noise_gain: spec.noise_gain,
        }
    }

    /// `BuildRadioPipeline`: encryption effect (after the TX effect) when
    /// requested and available, else the TX effect alone.
    pub fn process(&mut self, buf: &mut [f32], encrypted: bool) {
        self.tx_effect.process(buf);
        if encrypted {
            if let Some(e) = &mut self.encryption_effect {
                e.process(buf);
            }
        }
    }
}

/// Instantiated RX side (`RxRadioModel`).
pub struct RxModel {
    pub rx_effect: Effect,
}

impl RxModel {
    pub fn new(spec: &RadioModelSpec) -> Self {
        Self {
            rx_effect: Effect::from_spec(&spec.rx_effect),
        }
    }
}

/// `DefaultRadioModels.BuildArc210`.
pub fn default_arc210() -> RadioModelSpec {
    RadioModelSpec {
        version: 1,
        noise_gain: -33.0,
        tx_effect: EffectSpec::Chain {
            effects: vec![
                EffectSpec::Filters {
                    filters: vec![
                        FilterSpec::HighPass {
                            frequency: 1700.0,
                            q: Some(0.53),
                        },
                        FilterSpec::Peak {
                            frequency: 2801.0,
                            q: 0.5,
                            gain: 5.0,
                        },
                        FilterSpec::LowPass {
                            frequency: 5538.0,
                            q: None,
                        },
                    ],
                },
                EffectSpec::Saturation {
                    gain: 9.0,
                    threshold: -23.0,
                },
                EffectSpec::SidechainCompressor {
                    attack: 0.01,
                    make_up: 6.0,
                    release: 0.2,
                    threshold: -33.0,
                    ratio: 1.18,
                    sidechain_effect: Box::new(EffectSpec::Filters {
                        filters: vec![FilterSpec::HighPass {
                            frequency: 709.0,
                            q: None,
                        }],
                    }),
                },
                EffectSpec::Filters {
                    filters: vec![
                        FilterSpec::HighPass {
                            frequency: 456.0,
                            q: Some(0.36),
                        },
                        FilterSpec::LowPass {
                            frequency: 5435.0,
                            q: Some(0.39),
                        },
                    ],
                },
                EffectSpec::Gain { gain: 12.0 },
            ],
        },
        rx_effect: EffectSpec::Filters {
            filters: vec![
                FilterSpec::HighPass {
                    frequency: 270.0,
                    q: None,
                },
                FilterSpec::LowPass {
                    frequency: 4500.0,
                    q: None,
                },
            ],
        },
        encryption_effect: Some(EffectSpec::Cvsd),
    }
}

/// `DefaultRadioModels.BuildIntercom`.
pub fn default_intercom() -> RadioModelSpec {
    RadioModelSpec {
        version: 1,
        noise_gain: -60.0,
        tx_effect: EffectSpec::Chain {
            effects: vec![
                EffectSpec::Filters {
                    filters: vec![
                        FilterSpec::HighPass {
                            frequency: 207.0,
                            q: Some(0.5),
                        },
                        FilterSpec::Peak {
                            frequency: 3112.0,
                            q: 0.4,
                            gain: 16.0,
                        },
                        FilterSpec::LowPass {
                            frequency: 6036.0,
                            q: Some(0.4),
                        },
                        FilterSpec::LowPass {
                            frequency: 5538.0,
                            q: None,
                        },
                    ],
                },
                EffectSpec::Saturation {
                    gain: 2.0,
                    threshold: -33.0,
                },
                EffectSpec::SidechainCompressor {
                    attack: 0.01,
                    make_up: -1.0,
                    release: 0.2,
                    threshold: -17.0,
                    ratio: 1.18,
                    sidechain_effect: Box::new(EffectSpec::Filters {
                        filters: vec![FilterSpec::HighPass {
                            frequency: 709.0,
                            q: None,
                        }],
                    }),
                },
                EffectSpec::Filters {
                    filters: vec![
                        FilterSpec::HighPass {
                            frequency: 393.0,
                            q: Some(0.43),
                        },
                        FilterSpec::LowPass {
                            frequency: 4875.0,
                            q: Some(0.3),
                        },
                    ],
                },
                EffectSpec::Gain { gain: 8.0 },
            ],
        },
        rx_effect: EffectSpec::Filters {
            filters: vec![
                FilterSpec::HighPass {
                    frequency: 270.0,
                    q: None,
                },
                FilterSpec::LowPass {
                    frequency: 4500.0,
                    q: None,
                },
            ],
        },
        encryption_effect: None,
    }
}

/// `RadioModelFactory`: templates loaded once from `RadioModels/` and
/// `RadioModelsCustom/` (working directory), keyed by lowercase file stem.
#[derive(Default)]
pub struct RadioModelFactory {
    templates: HashMap<String, RadioModelSpec>,
}

impl RadioModelFactory {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load `<base>/RadioModels/*.json` then `<base>/RadioModelsCustom/*.json`.
    pub fn load(base: &Path) -> Self {
        let mut f = Self::default();
        for folder in ["RadioModels", "RadioModelsCustom"] {
            let dir = base.join(folder);
            let Ok(entries) = std::fs::read_dir(&dir) else {
                log::info!("No radio model folder {}", dir.display());
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("json"))
                    != Some(true)
                {
                    continue;
                }
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                match std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| Self::parse(&t).map_err(|e| e.to_string()))
                {
                    Ok(spec) => {
                        log::info!("Loaded radio model {name} from {}", path.display());
                        f.templates.insert(name, spec);
                    }
                    Err(e) => log::error!("Unable to parse radio preset file {}: {e}", path.display()),
                }
            }
        }
        f
    }

    pub fn parse(json: &str) -> Result<RadioModelSpec, serde_json::Error> {
        let cleaned = srs_protocol::server_settings::strip_json_comments_and_trailing_commas(json);
        serde_json::from_str(&cleaned)
    }

    pub fn insert(&mut self, name: &str, spec: RadioModelSpec) {
        self.templates.insert(name.to_ascii_lowercase(), spec);
    }

    pub fn template(&self, name: &str) -> Option<&RadioModelSpec> {
        self.templates.get(&name.to_ascii_lowercase())
    }

    /// `LoadTxRadio`.
    pub fn tx(&self, name: &str) -> Option<TxModel> {
        self.template(name).map(TxModel::new)
    }

    /// `LoadTxOrDefaultRadio`.
    pub fn tx_or_default_radio(&self, name: &str) -> TxModel {
        self.tx(name).unwrap_or_else(|| TxModel::new(&default_arc210()))
    }

    /// `LoadTxOrDefaultIntercom`.
    pub fn tx_or_default_intercom(&self, name: &str) -> TxModel {
        self.tx(name).unwrap_or_else(|| TxModel::new(&default_intercom()))
    }

    /// `LoadRxOrDefaultIntercom`.
    pub fn rx_or_default_intercom(&self, name: &str) -> RxModel {
        self.template(name)
            .map(RxModel::new)
            .unwrap_or_else(|| RxModel::new(&default_intercom()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shipped_arc210_json() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../DCS-SR-Client/RadioModels/arc210.json"
        );
        let text = std::fs::read_to_string(path).expect("arc210.json in the C# tree");
        let spec = RadioModelFactory::parse(&text).unwrap();
        assert_eq!(spec.version, 1);
        assert!(matches!(spec.tx_effect, EffectSpec::Chain { .. }));
        assert!(spec.encryption_effect.is_some());
        let mut tx = TxModel::new(&spec);
        let mut buf: Vec<f32> = (0..1920).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        tx.process(&mut buf, false);
        assert!(buf.iter().all(|s| s.is_finite()));
        assert!(buf.iter().any(|s| s.abs() > 0.01));
    }

    #[test]
    fn filter_json_rules() {
        let f: FilterSpec = serde_json::from_str(r#"{"$type":"lowpass","frequency":5538}"#).unwrap();
        assert_eq!(
            f,
            FilterSpec::LowPass {
                frequency: 5538.0,
                q: None
            }
        );
        let f: FilterSpec =
            serde_json::from_str(r#"{"$type":"highpass","frequency":1700,"q":0.53}"#).unwrap();
        assert_eq!(
            f,
            FilterSpec::HighPass {
                frequency: 1700.0,
                q: Some(0.53)
            }
        );
        assert!(serde_json::from_str::<FilterSpec>(r#"{"$type":"peak","frequency":1,"q":1}"#).is_err());
        assert!(serde_json::from_str::<FilterSpec>(r#"{"$type":"lowpass"}"#).is_err());
    }

    #[test]
    fn defaults_build_and_run() {
        let mut arc = TxModel::new(&default_arc210());
        let mut ic = TxModel::new(&default_intercom());
        let mut a: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut b = a.clone();
        arc.process(&mut a, true);
        ic.process(&mut b, false);
        assert!(
            a.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
            "cvsd output stays in range"
        );
        assert!(b.iter().all(|s| s.is_finite()));
        let mut rx = RxModel::new(&default_intercom());
        rx.rx_effect.process(&mut b);
    }

    #[test]
    fn factory_falls_back_to_defaults() {
        let f = RadioModelFactory::empty();
        assert_eq!(f.tx_or_default_radio("nope").noise_gain, -33.0);
        assert_eq!(f.tx_or_default_intercom("intercom").noise_gain, -60.0);
        assert!(f.tx("hfnoise").is_none());
    }
}
