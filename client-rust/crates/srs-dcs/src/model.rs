//! `DCSPlayerRadioInfo` / `DCSRadio` (`DCS-SR-Client/Network/DCS/Models/DCSState/`).
//!
//! This is the client's authoritative radio state: what DCS exports (UDP
//! 9084) merged with what the user set in the overlay. The wire form sent to
//! other clients is [`srs_protocol::RadioInfo`], produced by
//! [`DcsPlayerRadioInfo::to_radio_info`].
//!
//! JSON names are the C# field names (Lua writes them). Integers may arrive
//! as floats from Lua, hence the lenient deserialisers.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use srs_protocol::constants::MAX_RADIOS;
use srs_protocol::hearing::RxRadio;
use srs_protocol::message::lenient;
use srs_protocol::{Ambient, LatLng, Modulation, Radio, RadioInfo, Transponder};

/// Where non-aircraft unit ids start (EAM intercom groups).
pub const UNIT_ID_OFFSET: u32 = 100_000_000;

macro_rules! int_enum {
    ($(#[$meta:meta])* $name:ident { $( $variant:ident = $val:literal ),* $(,)? } default $default:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(i32)]
        pub enum $name { $( $variant = $val ),* }

        impl $name {
            pub const fn from_i32(v: i32) -> Self {
                match v { $( $val => $name::$variant, )* _ => $name::$default }
            }
            pub const fn as_i32(self) -> i32 { self as i32 }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_i32(self.as_i32())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Ok($name::from_i32(lenient::i32(d)?))
            }
        }
    };
}

int_enum! {
    /// `DCSRadio.EncryptionMode`.
    EncryptionMode {
        NoEncryption = 0,
        EncryptionJustOverlay = 1,
        EncryptionFull = 2,
        EncryptionCockpitToggleOverlayCode = 3,
    } default NoEncryption
}

#[allow(clippy::derivable_impls)]
impl Default for EncryptionMode {
    fn default() -> Self {
        EncryptionMode::NoEncryption
    }
}

int_enum! {
    /// `DCSRadio.FreqMode` / `VolumeMode` share the same values.
    FreqMode { Cockpit = 0, Overlay = 1 } default Cockpit
}

#[allow(clippy::derivable_impls)]
impl Default for FreqMode {
    fn default() -> Self {
        FreqMode::Cockpit
    }
}

int_enum! {
    /// `DCSRadio.VolumeMode`.
    VolumeMode { Cockpit = 0, Overlay = 1 } default Cockpit
}

#[allow(clippy::derivable_impls)]
impl Default for VolumeMode {
    fn default() -> Self {
        VolumeMode::Cockpit
    }
}

int_enum! {
    /// `DCSRadio.RetransmitMode`.
    RetransmitMode { Cockpit = 0, Overlay = 1, Disabled = 2 } default Disabled
}

#[allow(clippy::derivable_impls)]
impl Default for RetransmitMode {
    fn default() -> Self {
        RetransmitMode::Disabled
    }
}

int_enum! {
    /// `DCSPlayerRadioInfo.RadioSwitchControls`.
    SwitchControls { Hotas = 0, InCockpit = 1 } default Hotas
}

#[allow(clippy::derivable_impls)]
impl Default for SwitchControls {
    fn default() -> Self {
        SwitchControls::Hotas
    }
}

int_enum! {
    /// `DCSPlayerRadioInfo.SimultaneousTransmissionControl`.
    SimultaneousTransmissionControl { ExternalDcsControl = 0, EnabledInternalSrsControls = 1 } default ExternalDcsControl
}

#[allow(clippy::derivable_impls)]
impl Default for SimultaneousTransmissionControl {
    fn default() -> Self {
        SimultaneousTransmissionControl::ExternalDcsControl
    }
}

/// `DCSRadio`. Field names are the JSON keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DcsRadio {
    #[serde(deserialize_with = "lenient::i32")]
    pub channel: i32,
    #[serde(deserialize_with = "lenient::bool")]
    pub enc: bool,
    #[serde(rename = "encKey", deserialize_with = "lenient::u8")]
    pub enc_key: u8,
    #[serde(rename = "encMode")]
    pub enc_mode: EncryptionMode,
    #[serde(deserialize_with = "lenient::bool")]
    pub expansion: bool,
    #[serde(deserialize_with = "lenient::f64")]
    pub freq: f64,
    #[serde(rename = "freqMax", deserialize_with = "lenient::f64")]
    pub freq_max: f64,
    #[serde(rename = "freqMin", deserialize_with = "lenient::f64")]
    pub freq_min: f64,
    #[serde(rename = "freqMode")]
    pub freq_mode: FreqMode,
    #[serde(rename = "guardFreqMode")]
    pub guard_freq_mode: FreqMode,
    pub modulation: Modulation,
    pub name: String,
    pub model: String,
    #[serde(deserialize_with = "lenient::bool")]
    pub retransmit: bool,
    #[serde(rename = "IntercomUnitId", deserialize_with = "lenient::u32")]
    pub intercom_unit_id: u32,
    #[serde(rename = "rtMode")]
    pub rt_mode: RetransmitMode,
    #[serde(rename = "rxOnly", deserialize_with = "lenient::bool")]
    pub rx_only: bool,
    #[serde(rename = "secFreq", deserialize_with = "lenient::f64")]
    pub sec_freq: f64,
    #[serde(deserialize_with = "lenient::bool")]
    pub simul: bool,
    #[serde(rename = "volMode")]
    pub vol_mode: VolumeMode,
    #[serde(deserialize_with = "lenient::f32")]
    pub volume: f32,
}

#[allow(clippy::derivable_impls)]
impl Default for DcsRadio {
    fn default() -> Self {
        Self {
            channel: -1,
            enc: false,
            enc_key: 0,
            enc_mode: EncryptionMode::NoEncryption,
            expansion: false,
            freq: 1.0,
            freq_max: 1.0,
            freq_min: 1.0,
            freq_mode: FreqMode::Cockpit,
            guard_freq_mode: FreqMode::Cockpit,
            modulation: Modulation::Disabled,
            name: String::new(),
            model: String::new(),
            retransmit: false,
            intercom_unit_id: 0,
            rt_mode: RetransmitMode::Disabled,
            rx_only: false,
            sec_freq: 1.0,
            simul: false,
            vol_mode: VolumeMode::Cockpit,
            volume: 1.0,
        }
    }
}

impl DcsRadio {
    /// The "No Radio" placeholder used for EAM and unknown aircraft.
    pub fn no_radio() -> Self {
        Self {
            sec_freq: 0.0,
            name: "No Radio".into(),
            ..Default::default()
        }
    }

    /// C# `DCSRadio.Equals` (name, model, freq ±500 Hz, modulation, enc,
    /// encKey, retransmit, secFreq ±500 Hz, IntercomUnitId).
    pub fn same_as(&self, o: &Self) -> bool {
        self.name == o.name
            && self.model == o.model
            && srs_protocol::hearing::freq_close_enough(self.freq, o.freq)
            && self.modulation == o.modulation
            && self.enc == o.enc
            && self.enc_key == o.enc_key
            && self.retransmit == o.retransmit
            && srs_protocol::hearing::freq_close_enough(self.sec_freq, o.sec_freq)
            && self.intercom_unit_id == o.intercom_unit_id
    }

    /// Apply the MIDS overrides the C# code forces on AWACS radio files.
    pub fn apply_mids_override(&mut self) {
        if self.modulation == Modulation::Mids {
            self.freq = 1_030_100_000.0;
            self.freq_min = 1_030_000_000.0;
            self.freq_max = 1_060_000_000.0;
            self.enc_mode = EncryptionMode::NoEncryption;
            self.guard_freq_mode = FreqMode::Cockpit;
            self.vol_mode = VolumeMode::Overlay;
            self.freq_mode = FreqMode::Overlay;
        }
    }

    /// Wire form (`ConvertToRadioBase`): model normalised, name dropped.
    pub fn to_radio(&self) -> Radio {
        let mut r = Radio {
            enc: self.enc,
            enc_key: self.enc_key,
            freq: self.freq,
            modulation: self.modulation,
            retransmit: self.retransmit,
            sec_freq: self.sec_freq,
            intercom_unit_id: self.intercom_unit_id,
            ..Default::default()
        };
        r.set_model(&self.model);
        r
    }
}

impl RxRadio for DcsRadio {
    fn freq(&self) -> f64 {
        self.freq
    }
    fn sec_freq(&self) -> f64 {
        self.sec_freq
    }
    fn modulation(&self) -> Modulation {
        self.modulation
    }
    fn enc(&self) -> bool {
        self.enc
    }
    fn enc_key(&self) -> u8 {
        self.enc_key
    }
    fn intercom_unit_id(&self) -> u32 {
        self.intercom_unit_id
    }
}

/// `DCSAircraftCapabilities`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DcsAircraftCapabilities {
    #[serde(rename = "dcsPtt", deserialize_with = "lenient::bool")]
    pub dcs_ptt: bool,
    #[serde(rename = "dcsIFF", deserialize_with = "lenient::bool")]
    pub dcs_iff: bool,
    #[serde(rename = "dcsRadioSwitch", deserialize_with = "lenient::bool")]
    pub dcs_radio_switch: bool,
    #[serde(rename = "intercomHotMic", deserialize_with = "lenient::bool")]
    pub intercom_hot_mic: bool,
    pub desc: String,
}

/// `DCSPlayerRadioInfo`: everything DCS tells us plus what the user set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DcsPlayerRadioInfo {
    pub ambient: Ambient,
    pub capabilities: DcsAircraftCapabilities,
    pub control: SwitchControls,
    pub iff: Transponder,
    #[serde(rename = "inAircraft", deserialize_with = "lenient::bool")]
    pub in_aircraft: bool,
    #[serde(rename = "intercomHotMic", deserialize_with = "lenient::bool")]
    pub intercom_hot_mic: bool,
    #[serde(rename = "latLng")]
    pub lat_lng: LatLng,
    pub name: String,
    #[serde(deserialize_with = "lenient::bool")]
    pub ptt: bool,
    pub radios: Vec<DcsRadio>,
    #[serde(deserialize_with = "lenient::i32")]
    pub seat: i32,
    #[serde(deserialize_with = "lenient::i16")]
    pub selected: i16,
    #[serde(rename = "simultaneousTransmission", deserialize_with = "lenient::bool")]
    pub simultaneous_transmission: bool,
    #[serde(rename = "simultaneousTransmissionControl")]
    pub simultaneous_transmission_control: SimultaneousTransmissionControl,
    pub unit: String,
    #[serde(rename = "unitId", deserialize_with = "lenient::u32")]
    pub unit_id: u32,
    /// When this state was last refreshed (`LastUpdate`). Not serialised.
    #[serde(skip)]
    pub last_update: Option<Instant>,
}

#[allow(clippy::derivable_impls)]
impl Default for DcsPlayerRadioInfo {
    fn default() -> Self {
        Self {
            ambient: Ambient {
                ab_type: String::new(),
                vol: 0.0,
            },
            capabilities: DcsAircraftCapabilities::default(),
            control: SwitchControls::Hotas,
            iff: Transponder::default(),
            in_aircraft: false,
            intercom_hot_mic: false,
            lat_lng: LatLng::default(),
            name: String::new(),
            ptt: false,
            radios: (0..MAX_RADIOS).map(|_| DcsRadio::default()).collect(),
            seat: 0,
            selected: 0,
            simultaneous_transmission: false,
            simultaneous_transmission_control: SimultaneousTransmissionControl::ExternalDcsControl,
            unit: String::new(),
            unit_id: 0,
            last_update: None,
        }
    }
}

impl DcsPlayerRadioInfo {
    /// Radio data is "current" for 10 s after the last update.
    pub const CURRENT_FOR: Duration = Duration::from_secs(10);

    /// `Reset()`: ambient volume becomes `1.0`, everything else cleared.
    pub fn reset(&mut self) {
        let fresh = Self::default();
        *self = Self {
            ambient: Ambient {
                ab_type: String::new(),
                vol: 1.0,
            },
            ..fresh
        };
    }

    pub fn is_current(&self) -> bool {
        self.last_update.is_some_and(|t| t.elapsed() < Self::CURRENT_FOR)
    }

    pub fn touch(&mut self) {
        self.last_update = Some(Instant::now());
    }

    /// Index of the selected radio if it exists.
    pub fn selected_index(&self) -> Option<usize> {
        usize::try_from(self.selected)
            .ok()
            .filter(|i| *i < self.radios.len())
    }

    pub fn selected_radio(&self) -> Option<&DcsRadio> {
        self.selected_index().map(|i| &self.radios[i])
    }

    /// Wire form sent to the server (`ConvertToRadioBase`).
    pub fn to_radio_info(&self) -> RadioInfo {
        RadioInfo {
            ambient: self.ambient.clone(),
            iff: self.iff.clone(),
            radios: self.radios.iter().map(DcsRadio::to_radio).collect(),
            unit: self.unit.clone(),
            unit_id: self.unit_id,
        }
    }

    /// C# `DCSPlayerRadioInfo.Equals`: control, name, unit, unitId,
    /// inAircraft, iff, ambient and every radio (`DcsRadio::same_as`).
    pub fn same_as(&self, o: &Self) -> bool {
        self.control == o.control
            && self.name == o.name
            && self.unit == o.unit
            && self.unit_id == o.unit_id
            && self.in_aircraft == o.in_aircraft
            && self.iff.same_as(&o.iff)
            && self.ambient == o.ambient
            && self.radios.len() == o.radios.len()
            && self.radios.iter().zip(&o.radios).all(|(a, b)| a.same_as(b))
    }

    /// The DCS-side `CanHearTransmission`: identical to the shared rule
    /// except that intercom requires **our** unit id to be non-zero too.
    #[allow(clippy::too_many_arguments)]
    pub fn can_hear_transmission(
        &self,
        frequency: f64,
        modulation: Modulation,
        encryption_key: u8,
        strict_encryption: bool,
        sending_unit_id: u32,
        blocked_radios: &[usize],
    ) -> Option<srs_protocol::HearResult> {
        srs_protocol::hearing::can_hear_transmission_ext(
            &self.radios,
            self.unit_id,
            frequency,
            modulation,
            encryption_key,
            strict_encryption,
            sending_unit_id,
            blocked_radios,
            true,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lua_style_export() {
        let json = r#"{"name":"Maverick","latLng":{"lat":43.1,"lng":41.2,"alt":1000.5},"ptt":false,
            "radios":[{"name":"Intercom","freq":100.0,"modulation":2,"volume":1.0},
                      {"name":"AN/ARC-210","freq":251000000.0,"modulation":0,"freqMode":1.0,"encMode":2,"encKey":1.0,"volume":0.8}],
            "selected":1.0,"seat":0,"unit":"FA-18C_hornet","unitId":1.6777216e+07,"iff":{"control":0,"mode1":51,"mode3":7700,"mode4":true,"status":1},
            "ambient":{"abType":"fa18","vol":0.15},"capabilities":{"dcsPtt":true,"dcsIFF":true,"dcsRadioSwitch":true,"intercomHotMic":false,"desc":"x"},
            "control":1,"inAircraft":true,"intercomHotMic":false,"simultaneousTransmission":false,"simultaneousTransmissionControl":0}"#;
        let info: DcsPlayerRadioInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.unit_id, 16_777_216);
        assert_eq!(info.selected, 1);
        assert_eq!(info.radios.len(), 2);
        assert_eq!(info.radios[1].freq_mode, FreqMode::Overlay);
        assert_eq!(info.radios[1].enc_mode, EncryptionMode::EncryptionFull);
        assert_eq!(info.radios[1].enc_key, 1);
        assert_eq!(info.control, SwitchControls::InCockpit);
        assert_eq!(info.iff.mode3, 7700);
        assert!(info.iff.mode4);
        assert!(info.capabilities.dcs_ptt);
        assert!(!info.is_current());
        let wire = info.to_radio_info();
        assert_eq!(wire.radios[1].model, "");
        assert_eq!(wire.unit, "FA-18C_hornet");
    }

    #[test]
    fn intercom_requires_own_unit_id() {
        let mut info = DcsPlayerRadioInfo::default();
        info.radios[0].modulation = Modulation::Intercom;
        info.unit_id = 0;
        assert!(info
            .can_hear_transmission(100.0, Modulation::Intercom, 0, false, 0, &[])
            .is_none());
        info.unit_id = 7;
        assert!(info
            .can_hear_transmission(100.0, Modulation::Intercom, 0, false, 7, &[])
            .is_some());
    }

    #[test]
    fn mids_override() {
        let mut r = DcsRadio {
            modulation: Modulation::Mids,
            ..Default::default()
        };
        r.apply_mids_override();
        assert_eq!(r.freq, 1_030_100_000.0);
        assert_eq!(r.vol_mode, VolumeMode::Overlay);
    }
}
