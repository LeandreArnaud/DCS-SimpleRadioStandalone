//! TCP control-channel messages: one JSON object per line, terminated by `\n`.
//!
//! Field names are the C# member names verbatim (System.Text.Json with
//! `IncludeFields = true`, no naming policy, enums as integers, nulls omitted
//! on write). Every struct tolerates missing fields on read, exactly like STJ.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::constants::MAX_RADIOS;
use crate::hearing::freq_close_enough;
use crate::modulation::Modulation;

/// `NetworkMessage.MessageType`. Never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MessageType {
    /// Metadata update, no radio information.
    #[default]
    Update,
    Ping,
    Sync,
    /// Full update including `RadioInfo`.
    RadioUpdate,
    ServerSettings,
    ClientDisconnect,
    VersionMismatch,
    ExternalAwacsModePassword,
    ExternalAwacsModeDisconnect,
    GatewayClientFullUpdate,
    GatewayClientMetadataUpdate,
    GatewayClientDisconnect,
    /// Value this build does not know. Kept so the message still decodes.
    Unknown(i32),
}

impl MessageType {
    pub const fn as_i32(self) -> i32 {
        match self {
            MessageType::Update => 0,
            MessageType::Ping => 1,
            MessageType::Sync => 2,
            MessageType::RadioUpdate => 3,
            MessageType::ServerSettings => 4,
            MessageType::ClientDisconnect => 5,
            MessageType::VersionMismatch => 6,
            MessageType::ExternalAwacsModePassword => 7,
            MessageType::ExternalAwacsModeDisconnect => 8,
            MessageType::GatewayClientFullUpdate => 9,
            MessageType::GatewayClientMetadataUpdate => 10,
            MessageType::GatewayClientDisconnect => 11,
            MessageType::Unknown(v) => v,
        }
    }

    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => MessageType::Update,
            1 => MessageType::Ping,
            2 => MessageType::Sync,
            3 => MessageType::RadioUpdate,
            4 => MessageType::ServerSettings,
            5 => MessageType::ClientDisconnect,
            6 => MessageType::VersionMismatch,
            7 => MessageType::ExternalAwacsModePassword,
            8 => MessageType::ExternalAwacsModeDisconnect,
            9 => MessageType::GatewayClientFullUpdate,
            10 => MessageType::GatewayClientMetadataUpdate,
            11 => MessageType::GatewayClientDisconnect,
            other => MessageType::Unknown(other),
        }
    }
}

impl Serialize for MessageType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i32(self.as_i32())
    }
}

impl<'de> Deserialize<'de> for MessageType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(MessageType::from_i32(i32::deserialize(d)?))
    }
}

// ---------------------------------------------------------------------------
// Helpers shared with the DCS-side models.
// ---------------------------------------------------------------------------

/// Lowercase, trim, keep `[a-zA-Z0-9]` only, max 32 chars. Mirrors the
/// `RadioBase.Model` / `RadioBase.Name` setters.
pub fn normalise_radio_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .take(32)
        .collect()
}

fn de_normalised<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = Option::<String>::deserialize(d)?.unwrap_or_default();
    Ok(normalise_radio_name(&s))
}

/// `Math.Round(value, 5)` with .NET's default banker's rounding; `0` stays `0`.
pub fn round5(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    (v * 1e5).round_ties_even() / 1e5
}

fn de_round5<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(round5(f64::deserialize(d)?))
}

fn ser_round5<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64(round5(*v))
}

fn de_name<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = Option::<String>::deserialize(d)?.unwrap_or_default();
    Ok(SrClient::fix_name(s))
}

/// Accept `12`, `12.0`, `1e+08` or `"12"` for an integer. Lua-produced JSON
/// can contain any of these. Out-of-range values saturate.
pub mod lenient {
    use serde::{Deserialize, Deserializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Num {
        I(i64),
        F(f64),
        S(String),
        B(bool),
    }

    fn to_f64<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(match Num::deserialize(d)? {
            Num::I(i) => i as f64,
            Num::F(f) => f,
            Num::S(s) => s.trim().parse::<f64>().unwrap_or(0.0),
            Num::B(b) => b as u8 as f64,
        })
    }

    pub fn i32<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
        Ok(to_f64(d)? as i32)
    }
    pub fn u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        Ok(to_f64(d)?.max(0.0) as u32)
    }
    pub fn u8<'de, D: Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
        Ok(to_f64(d)?.clamp(0.0, 255.0) as u8)
    }
    pub fn i16<'de, D: Deserializer<'de>>(d: D) -> Result<i16, D::Error> {
        Ok(to_f64(d)? as i16)
    }
    pub fn f64<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        to_f64(d)
    }
    pub fn f32<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
        Ok(to_f64(d)? as f32)
    }
    pub fn bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
        Ok(match Num::deserialize(d)? {
            Num::I(i) => i != 0,
            Num::F(f) => f != 0.0,
            Num::S(s) => s.eq_ignore_ascii_case("true") || s == "1",
            Num::B(b) => b,
        })
    }
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/// `Common/Models/Player/Ambient.cs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ambient {
    #[serde(rename = "abType")]
    pub ab_type: String,
    #[serde(deserialize_with = "lenient::f32")]
    pub vol: f32,
}

impl Default for Ambient {
    fn default() -> Self {
        // C# field initialiser. `RadioInfo::default()` overrides vol to 0.0.
        Self {
            ab_type: String::new(),
            vol: 0.15,
        }
    }
}

/// `Common/Models/Player/Transponder.cs` (the `expansion` field is
/// `[JsonIgnore]` and therefore absent here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transponder {
    /// 0 cockpit, 1 overlay, 2 disabled / not fitted.
    #[serde(deserialize_with = "lenient::i32")]
    pub control: i32,
    #[serde(deserialize_with = "lenient::i32")]
    pub mic: i32,
    #[serde(deserialize_with = "lenient::i32")]
    pub mode1: i32,
    #[serde(deserialize_with = "lenient::i32")]
    pub mode2: i32,
    #[serde(deserialize_with = "lenient::i32")]
    pub mode3: i32,
    #[serde(deserialize_with = "lenient::bool")]
    pub mode4: bool,
    /// 0 off, 1 normal, 2 ident.
    #[serde(deserialize_with = "lenient::i32")]
    pub status: i32,
}

impl Transponder {
    pub const CONTROL_COCKPIT: i32 = 0;
    pub const CONTROL_OVERLAY: i32 = 1;
    pub const CONTROL_DISABLED: i32 = 2;
    pub const STATUS_OFF: i32 = 0;
    pub const STATUS_NORMAL: i32 = 1;
    pub const STATUS_IDENT: i32 = 2;

    /// C# `Transponder.Equals`: modes and status only.
    pub fn same_as(&self, other: &Self) -> bool {
        self.mode1 == other.mode1
            && self.mode2 == other.mode2
            && self.mode3 == other.mode3
            && self.mode4 == other.mode4
            && self.status == other.status
    }
}

impl Default for Transponder {
    fn default() -> Self {
        Self {
            control: Self::CONTROL_DISABLED,
            mic: -1,
            mode1: -1,
            mode2: -1,
            mode3: -1,
            mode4: false,
            status: 0,
        }
    }
}

/// `Common/Models/Player/RadioBase.cs`: what other clients know about one of
/// our radios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Radio {
    #[serde(deserialize_with = "lenient::bool")]
    pub enc: bool,
    #[serde(rename = "encKey", deserialize_with = "lenient::u8")]
    pub enc_key: u8,
    /// Hz. Default `1`, not `0`.
    #[serde(deserialize_with = "lenient::f64")]
    pub freq: f64,
    pub modulation: Modulation,
    #[serde(deserialize_with = "lenient::bool")]
    pub retransmit: bool,
    /// Guard frequency in Hz. Default `1`.
    #[serde(rename = "secFreq", deserialize_with = "lenient::f64")]
    pub sec_freq: f64,
    /// Instructor mode: unit whose intercom this radio listens to.
    #[serde(rename = "IntercomUnitId", deserialize_with = "lenient::u32")]
    pub intercom_unit_id: u32,
    /// Normalised radio model (`arc210`, ...). Always lowercase alphanumeric.
    #[serde(rename = "Model", deserialize_with = "de_normalised")]
    pub model: String,
    /// Normalised radio name. The C# client always sends `""`.
    #[serde(rename = "Name", deserialize_with = "de_normalised")]
    pub name: String,
}

impl Default for Radio {
    fn default() -> Self {
        Self {
            enc: false,
            enc_key: 0,
            freq: 1.0,
            modulation: Modulation::Disabled,
            retransmit: false,
            sec_freq: 1.0,
            intercom_unit_id: 0,
            model: String::new(),
            name: String::new(),
        }
    }
}

impl Radio {
    pub fn set_model(&mut self, model: &str) {
        self.model = normalise_radio_name(model);
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = normalise_radio_name(name);
    }

    /// C# `RadioBase.Equals`: "would a change here stop us receiving?"
    /// Frequencies compare within 500 Hz, `name` is ignored.
    pub fn same_as(&self, other: &Self) -> bool {
        freq_close_enough(self.freq, other.freq)
            && self.modulation == other.modulation
            && self.enc == other.enc
            && self.enc_key == other.enc_key
            && self.retransmit == other.retransmit
            && freq_close_enough(self.sec_freq, other.sec_freq)
            && self.model == other.model
            && self.intercom_unit_id == other.intercom_unit_id
    }
}

/// `Common/Models/Player/PlayerRadioInfoBase.cs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RadioInfo {
    pub ambient: Ambient,
    pub iff: Transponder,
    /// Always [`MAX_RADIOS`] entries when produced by this crate; index 0 is
    /// the intercom. Decoded messages keep whatever length was received.
    pub radios: Vec<Radio>,
    pub unit: String,
    #[serde(rename = "unitId", deserialize_with = "lenient::u32")]
    pub unit_id: u32,
}

impl Default for RadioInfo {
    fn default() -> Self {
        Self {
            ambient: Ambient {
                ab_type: String::new(),
                vol: 0.0,
            },
            iff: Transponder::default(),
            radios: (0..MAX_RADIOS).map(|_| Radio::default()).collect(),
            unit: String::new(),
            unit_id: 0,
        }
    }
}

impl RadioInfo {
    /// C# `PlayerRadioInfoBase.Reset()`: ambient volume becomes `1.0`.
    pub fn reset(&mut self) {
        self.ambient = Ambient {
            ab_type: String::new(),
            vol: 1.0,
        };
        self.unit.clear();
        for r in &mut self.radios {
            *r = Radio::default();
        }
    }

    pub fn radio(&self, index: usize) -> Option<&Radio> {
        self.radios.get(index)
    }

    /// C# `PlayerRadioInfoBase.Equals`. A length mismatch in `radios` makes
    /// the C# code throw inside its `try`, hence `false`.
    pub fn same_as(&self, other: &Self) -> bool {
        if self.unit != other.unit || self.unit_id != other.unit_id {
            return false;
        }
        if !self.iff.same_as(&other.iff) || self.ambient != other.ambient {
            return false;
        }
        if self.radios.len() != other.radios.len() {
            return false;
        }
        self.radios.iter().zip(&other.radios).all(|(a, b)| a.same_as(b))
    }
}

/// `Common/Models/Player/LatLngPosition.cs`. Stored to 5 decimals.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LatLng {
    #[serde(deserialize_with = "de_round5", serialize_with = "ser_round5")]
    pub alt: f64,
    #[serde(deserialize_with = "de_round5", serialize_with = "ser_round5")]
    pub lat: f64,
    #[serde(deserialize_with = "de_round5", serialize_with = "ser_round5")]
    pub lng: f64,
}

impl LatLng {
    pub fn new(lat: f64, lng: f64, alt: f64) -> Self {
        Self {
            alt: round5(alt),
            lat: round5(lat),
            lng: round5(lng),
        }
    }

    /// Non-zero latitude and longitude.
    pub fn is_valid(&self) -> bool {
        self.lat != 0.0 && self.lng != 0.0
    }
}

/// `Common/Models/Player/SRClientBase.cs` as seen on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SrClient {
    #[serde(rename = "ClientGuid")]
    pub client_guid: String,
    /// Never empty after construction or decoding: `""` becomes `"---"`.
    #[serde(rename = "Name", deserialize_with = "de_name")]
    pub name: String,
    /// 0 spectator, 1 red, 2 blue.
    #[serde(rename = "Coalition")]
    pub coalition: i32,
    #[serde(rename = "AllowRecord")]
    pub allow_record: bool,
    #[serde(rename = "Seat")]
    pub seat: i32,
    #[serde(rename = "RadioInfo", skip_serializing_if = "Option::is_none")]
    pub radio_info: Option<RadioInfo>,
    #[serde(rename = "LatLngPosition", skip_serializing_if = "Option::is_none")]
    pub lat_lng_position: Option<LatLng>,
    #[serde(rename = "Gateway")]
    pub gateway: bool,
    #[serde(rename = "DISEntityId")]
    pub dis_entity_id: i32,
    #[serde(rename = "GatewayClient")]
    pub gateway_client: bool,
}

impl Default for SrClient {
    fn default() -> Self {
        Self {
            client_guid: String::new(),
            name: String::new(),
            coalition: 0,
            allow_record: false,
            seat: 0,
            radio_info: None,
            lat_lng_position: None,
            gateway: false,
            dis_entity_id: -1,
            gateway_client: false,
        }
    }
}

impl SrClient {
    pub const COALITION_SPECTATOR: i32 = 0;
    pub const COALITION_RED: i32 = 1;
    pub const COALITION_BLUE: i32 = 2;

    /// `Name` setter: empty or null becomes `"---"`.
    pub fn fix_name(name: String) -> String {
        if name.is_empty() {
            "---".to_string()
        } else {
            name
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = Self::fix_name(name.to_string());
    }

    /// C# `MetaDataEquals`. `RadioInfo` is ignored.
    pub fn metadata_same_as(&self, other: &Self, use_position: bool) -> bool {
        if use_position && self.lat_lng_position != other.lat_lng_position {
            return false;
        }
        self.coalition == other.coalition
            && self.seat == other.seat
            && self.name == other.name
            && self.dis_entity_id == other.dis_entity_id
            && self.allow_record == other.allow_record
            && self.client_guid == other.client_guid
    }
}

/// `Common/Models/NetworkMessage.cs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NetworkMessage {
    #[serde(rename = "Client", skip_serializing_if = "Option::is_none")]
    pub client: Option<SrClient>,
    #[serde(rename = "MsgType")]
    pub msg_type: MessageType,
    #[serde(rename = "Clients", skip_serializing_if = "Option::is_none")]
    pub clients: Option<Vec<SrClient>>,
    #[serde(rename = "ServerSettings", skip_serializing_if = "Option::is_none")]
    pub server_settings: Option<HashMap<String, String>>,
    #[serde(rename = "ExternalAWACSModePassword", skip_serializing_if = "Option::is_none")]
    pub external_awacs_mode_password: Option<String>,
    /// Sender version, always stamped before sending (`UpdaterChecker.VERSION`).
    #[serde(rename = "Version", skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl NetworkMessage {
    pub fn new(msg_type: MessageType) -> Self {
        Self {
            msg_type,
            ..Default::default()
        }
    }

    pub fn with_client(msg_type: MessageType, client: SrClient) -> Self {
        Self {
            msg_type,
            client: Some(client),
            ..Default::default()
        }
    }

    /// Serialise as one line with the given client version, ready to write
    /// to the socket (`\n` included). Mirrors `NetworkMessage.Encode()`.
    pub fn to_line(&self, version: &str) -> String {
        let mut m = self.clone();
        m.version = Some(version.to_string());
        let mut s = serde_json::to_string(&m).expect("NetworkMessage is always serialisable");
        s.push('\n');
        s
    }

    /// Parse one received line (with or without the trailing newline).
    pub fn from_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim_end_matches(['\r', '\n']))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise() {
        assert_eq!(normalise_radio_name(" AN/ARC-210 "), "anarc210");
        assert_eq!(normalise_radio_name(&"x".repeat(40)).len(), 32);
    }

    #[test]
    fn round5_matches_dotnet() {
        assert_eq!(round5(0.0), 0.0);
        assert_eq!(round5(43.123456789), 43.12346);
        assert_eq!(round5(-1.000004), -1.0);
    }

    #[test]
    fn empty_name_becomes_dashes() {
        let c: SrClient = serde_json::from_str(r#"{"ClientGuid":"x","Name":""}"#).unwrap();
        assert_eq!(c.name, "---");
        let c: SrClient = serde_json::from_str(r#"{"ClientGuid":"x","Name":null}"#).unwrap();
        assert_eq!(c.name, "---");
    }

    #[test]
    fn message_type_unknown_survives() {
        let m: NetworkMessage = serde_json::from_str(r#"{"MsgType":99}"#).unwrap();
        assert_eq!(m.msg_type, MessageType::Unknown(99));
        assert_eq!(serde_json::to_string(&m.msg_type).unwrap(), "99");
    }

    #[test]
    fn nulls_are_omitted_and_names_are_csharp() {
        let m = NetworkMessage::with_client(MessageType::Sync, SrClient::default());
        let line = m.to_line("2.4.1.0");
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        let o = v.as_object().unwrap();
        assert_eq!(o["MsgType"], 2);
        assert_eq!(o["Version"], "2.4.1.0");
        assert!(!o.contains_key("Clients"));
        assert!(!o.contains_key("ServerSettings"));
        let c = o["Client"].as_object().unwrap();
        assert!(!c.contains_key("RadioInfo"));
        assert!(!c.contains_key("LatLngPosition"));
        assert_eq!(c["DISEntityId"], -1);
        for k in [
            "ClientGuid",
            "Name",
            "Coalition",
            "AllowRecord",
            "Seat",
            "Gateway",
            "GatewayClient",
        ] {
            assert!(c.contains_key(k), "{k}");
        }
    }

    #[test]
    fn radio_info_field_names() {
        let ri = RadioInfo::default();
        let v = serde_json::to_value(&ri).unwrap();
        let o = v.as_object().unwrap();
        for k in ["ambient", "iff", "radios", "unit", "unitId"] {
            assert!(o.contains_key(k), "{k}");
        }
        assert_eq!(o["radios"].as_array().unwrap().len(), MAX_RADIOS);
        let r = &o["radios"][0];
        for k in [
            "enc",
            "encKey",
            "freq",
            "modulation",
            "retransmit",
            "secFreq",
            "IntercomUnitId",
            "Model",
            "Name",
        ] {
            assert!(r.get(k).is_some(), "{k}");
        }
        assert_eq!(r["freq"], 1.0);
        assert_eq!(r["modulation"], 3);
        let iff = &o["iff"];
        for k in ["control", "mic", "mode1", "mode2", "mode3", "mode4", "status"] {
            assert!(iff.get(k).is_some(), "{k}");
        }
        assert!(iff.get("expansion").is_none());
        assert_eq!(o["ambient"]["vol"], 0.0);
    }

    #[test]
    fn radio_equality_is_within_500hz_and_ignores_name() {
        let a = Radio {
            freq: 251_000_000.0,
            name: "a".into(),
            ..Default::default()
        };
        let mut b = a.clone();
        b.freq += 499.0;
        b.name = "b".into();
        assert!(a.same_as(&b));
        b.freq += 2.0;
        assert!(!a.same_as(&b));
    }

    #[test]
    fn lenient_ints_from_lua_floats() {
        let r: Radio =
            serde_json::from_str(r#"{"encKey":1.0,"IntercomUnitId":1.6777216e+07,"enc":1}"#).unwrap();
        assert_eq!(r.enc_key, 1);
        assert_eq!(r.intercom_unit_id, 16_777_216);
        assert!(r.enc);
    }
}
