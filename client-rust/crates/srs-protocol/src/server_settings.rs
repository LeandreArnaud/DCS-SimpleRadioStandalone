//! Server settings as pushed to clients in SYNC / SERVER_SETTINGS messages.
//! Port of `Common/Settings/Setting/ServerSettingsKeys.cs` and
//! `Common/Network/Singletons/SyncedServerSettings.cs`.

use std::collections::HashMap;

use serde::Deserialize;

use crate::message::lenient;
use crate::modulation::Modulation;

macro_rules! server_keys {
    ($( $name:ident = $num:literal ),* $(,)?) => {
        /// Keys of the `ServerSettings` dictionary. The string form is the
        /// C# enum name; the number is only informative. Append only.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(non_camel_case_types)]
        pub enum ServerSettingsKey { $( $name = $num ),* }

        impl ServerSettingsKey {
            pub const ALL: &'static [ServerSettingsKey] = &[ $( ServerSettingsKey::$name ),* ];

            pub const fn as_str(self) -> &'static str {
                match self { $( ServerSettingsKey::$name => stringify!($name) ),* }
            }

            pub fn from_str_opt(s: &str) -> Option<Self> {
                match s { $( stringify!($name) => Some(ServerSettingsKey::$name), )* _ => None }
            }
        }
    };
}

server_keys! {
    SERVER_PORT = 0,
    COALITION_AUDIO_SECURITY = 1,
    SPECTATORS_AUDIO_DISABLED = 2,
    CLIENT_EXPORT_ENABLED = 3,
    LOS_ENABLED = 4,
    DISTANCE_ENABLED = 5,
    IRL_RADIO_TX = 6,
    IRL_RADIO_RX_INTERFERENCE = 7,
    IRL_RADIO_STATIC = 8,
    RADIO_EXPANSION = 9,
    EXTERNAL_AWACS_MODE = 10,
    EXTERNAL_AWACS_MODE_BLUE_PASSWORD = 11,
    EXTERNAL_AWACS_MODE_RED_PASSWORD = 12,
    CLIENT_EXPORT_FILE_PATH = 13,
    CHECK_FOR_BETA_UPDATES = 14,
    ALLOW_RADIO_ENCRYPTION = 15,
    TEST_FREQUENCIES = 16,
    SHOW_TUNED_COUNT = 17,
    GLOBAL_LOBBY_FREQUENCIES = 18,
    SHOW_TRANSMITTER_NAME = 19,
    LOTATC_EXPORT_ENABLED = 20,
    LOTATC_EXPORT_PORT = 21,
    LOTATC_EXPORT_IP = 22,
    UPNP_ENABLED = 23,
    RETRANSMISSION_NODE_LIMIT = 24,
    STRICT_RADIO_ENCRYPTION = 25,
    TRANSMISSION_LOG_ENABLED = 26,
    TRANSMISSION_LOG_RETENTION = 27,
    RADIO_EFFECT_OVERRIDE = 28,
    SERVER_IP = 29,
    SERVER_PRESETS_ENABLED = 30,
    SERVER_PRESETS = 31,
    HTTP_SERVER_ENABLED = 32,
    HTTP_SERVER_PORT = 33,
    HTTP_SERVER_API_KEY = 34,
    SERVER_EAM_RADIO_PRESET_ENABLED = 35,
    SERVER_EAM_RADIO_PRESET = 36,
    ALLOW_INSTRUCTOR_MODE = 37,
    HTTP_SERVER_ADDRESS = 38,
}

impl ServerSettingsKey {
    /// `DefaultServerSettings.Defaults`. Keys without a default return `""`.
    pub const fn default_value(self) -> &'static str {
        use ServerSettingsKey::*;
        match self {
            CLIENT_EXPORT_ENABLED => "false",
            COALITION_AUDIO_SECURITY => "false",
            DISTANCE_ENABLED => "false",
            EXTERNAL_AWACS_MODE => "false",
            EXTERNAL_AWACS_MODE_BLUE_PASSWORD => "",
            EXTERNAL_AWACS_MODE_RED_PASSWORD => "",
            IRL_RADIO_RX_INTERFERENCE => "false",
            IRL_RADIO_STATIC => "false",
            IRL_RADIO_TX => "false",
            LOS_ENABLED => "false",
            RADIO_EXPANSION => "false",
            SERVER_PORT => "5002",
            SPECTATORS_AUDIO_DISABLED => "false",
            CLIENT_EXPORT_FILE_PATH => "clients-list.json",
            CHECK_FOR_BETA_UPDATES => "false",
            ALLOW_RADIO_ENCRYPTION => "true",
            TEST_FREQUENCIES => "247.2,120.3",
            SHOW_TUNED_COUNT => "true",
            GLOBAL_LOBBY_FREQUENCIES => "248.22",
            LOTATC_EXPORT_ENABLED => "false",
            LOTATC_EXPORT_PORT => "10712",
            LOTATC_EXPORT_IP => "127.0.0.1",
            UPNP_ENABLED => "true",
            SHOW_TRANSMITTER_NAME => "false",
            RETRANSMISSION_NODE_LIMIT => "0",
            STRICT_RADIO_ENCRYPTION => "false",
            TRANSMISSION_LOG_ENABLED => "false",
            TRANSMISSION_LOG_RETENTION => "2",
            RADIO_EFFECT_OVERRIDE => "false",
            SERVER_IP => "0.0.0.0",
            SERVER_PRESETS_ENABLED => "false",
            HTTP_SERVER_ENABLED => "false",
            HTTP_SERVER_PORT => "8080",
            HTTP_SERVER_API_KEY => "",
            SERVER_EAM_RADIO_PRESET_ENABLED => "false",
            ALLOW_INSTRUCTOR_MODE => "false",
            HTTP_SERVER_ADDRESS => "localhost",
            SERVER_PRESETS | SERVER_EAM_RADIO_PRESET => "",
        }
    }
}

/// One entry of `SERVER_PRESETS` (`ServerPresetChannel`), keyed by radio name
/// prefix. Deserialised case-insensitively like the C# code.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(default)]
pub struct ServerPresetChannel {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "frequency", deserialize_with = "lenient::f64")]
    pub frequency: f64,
}

/// `Common/Models/Player/DCSRadioCustom.cs`: a radio definition for the
/// External AWACS Mode preset (`SERVER_EAM_RADIO_PRESET`, also the local
/// `awacs-radios.json`). Keys are matched case-insensitively.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct DcsRadioCustom {
    #[serde(deserialize_with = "lenient::i32")]
    pub channel: i32,
    #[serde(deserialize_with = "lenient::bool")]
    pub enc: bool,
    #[serde(rename = "enckey", deserialize_with = "lenient::u8")]
    pub enc_key: u8,
    #[serde(rename = "encmode", deserialize_with = "lenient::i32")]
    pub enc_mode: i32,
    #[serde(deserialize_with = "lenient::bool")]
    pub expansion: bool,
    #[serde(deserialize_with = "lenient::f64")]
    pub freq: f64,
    #[serde(rename = "freqmax", deserialize_with = "lenient::f64")]
    pub freq_max: f64,
    #[serde(rename = "freqmin", deserialize_with = "lenient::f64")]
    pub freq_min: f64,
    #[serde(rename = "freqmode", deserialize_with = "lenient::i32")]
    pub freq_mode: i32,
    #[serde(rename = "guardfreqmode", deserialize_with = "lenient::i32")]
    pub guard_freq_mode: i32,
    pub modulation: Modulation,
    pub name: String,
    pub model: String,
    #[serde(rename = "rtmode", deserialize_with = "lenient::i32")]
    pub rt_mode: i32,
    #[serde(rename = "rxonly", deserialize_with = "lenient::bool")]
    pub rx_only: bool,
    #[serde(rename = "secfreq", deserialize_with = "lenient::f64")]
    pub sec_freq: f64,
    #[serde(rename = "volmode", deserialize_with = "lenient::i32")]
    pub vol_mode: i32,
    #[serde(deserialize_with = "lenient::bool")]
    pub simul: bool,
}

impl Default for DcsRadioCustom {
    fn default() -> Self {
        Self {
            channel: -1,
            enc: false,
            enc_key: 0,
            enc_mode: 0,
            expansion: false,
            freq: 1.0,
            freq_max: 1.0,
            freq_min: 1.0,
            freq_mode: 0,
            guard_freq_mode: 0,
            modulation: Modulation::Disabled,
            name: String::new(),
            model: String::new(),
            rt_mode: 2,
            rx_only: false,
            sec_freq: 1.0,
            vol_mode: 0,
            simul: false,
        }
    }
}

/// Lower-case every object key, recursively, so structs with lowercase
/// `rename`s deserialise case-insensitively (STJ `PropertyNameCaseInsensitive`).
fn lowercase_keys(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), lowercase_keys(v)))
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(lowercase_keys).collect())
        }
        other => other,
    }
}

/// Deserialise JSON with case-insensitive keys, trailing commas and comments
/// tolerated (what the C# code configures for these two settings).
pub fn from_json_case_insensitive<T: serde::de::DeserializeOwned>(
    json: &str,
) -> Result<T, serde_json::Error> {
    let cleaned = strip_json_comments_and_trailing_commas(json);
    let v: serde_json::Value = serde_json::from_str(&cleaned)?;
    serde_json::from_value(lowercase_keys(v))
}

/// Minimal cleaner: removes `//` and `/* */` comments outside strings and
/// trailing commas before `]` / `}` (STJ `ReadCommentHandling.Skip` +
/// `AllowTrailingCommas`).
pub fn strip_json_comments_and_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut in_str = false;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        let next = chars.get(i + 1).copied();
        match c {
            '"' => {
                in_str = true;
                out.push(c);
                i += 1;
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
            }
            ',' => {
                let mut j = i + 1;
                // Skip whitespace and comments to find the next significant char.
                loop {
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j + 1 < chars.len() && chars[j] == '/' && chars[j + 1] == '/' {
                        while j < chars.len() && chars[j] != '\n' {
                            j += 1;
                        }
                        continue;
                    }
                    if j + 1 < chars.len() && chars[j] == '/' && chars[j + 1] == '*' {
                        j += 2;
                        while j + 1 < chars.len() && !(chars[j] == '*' && chars[j + 1] == '/') {
                            j += 1;
                        }
                        j += 2;
                        continue;
                    }
                    break;
                }
                if !(j < chars.len() && (chars[j] == ']' || chars[j] == '}')) {
                    out.push(',');
                }
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn parse_server_presets(json: &str) -> Result<HashMap<String, Vec<ServerPresetChannel>>, serde_json::Error> {
    let cleaned = strip_json_comments_and_trailing_commas(json);
    let raw: HashMap<String, Vec<serde_json::Value>> = serde_json::from_str(&cleaned)?;
    raw.into_iter()
        .map(|(k, items)| {
            let channels = items
                .into_iter()
                .map(|v| serde_json::from_value::<ServerPresetChannel>(lowercase_keys(v)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((k, channels))
        })
        .collect()
}

/// Client-side view of the server settings (`SyncedServerSettings`).
#[derive(Debug, Clone, Default)]
pub struct SyncedServerSettings {
    settings: HashMap<String, String>,
    /// `GLOBAL_LOBBY_FREQUENCIES`, converted from MHz to Hz.
    pub global_frequencies_hz: Vec<f64>,
    /// `RETRANSMISSION_NODE_LIMIT`; 0 means no retransmission.
    pub retransmit_node_limit: i32,
    /// `SERVER_PRESETS`: radio-name prefix -> channels.
    pub server_presets: HashMap<String, Vec<ServerPresetChannel>>,
    /// `SERVER_EAM_RADIO_PRESET` when valid (exactly 11 radios, first one
    /// DISABLED or INTERCOM), else empty.
    pub custom_eam_radios: Vec<DcsRadioCustom>,
    /// `Version` of the last SYNC / SERVER_SETTINGS message.
    pub server_version: Option<String>,
}

impl SyncedServerSettings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a `ServerSettings` dictionary. Mirrors `Decode()`: unknown keys
    /// are kept, special keys are parsed into typed fields.
    pub fn decode(&mut self, encoded: &HashMap<String, String>) {
        for (k, v) in encoded {
            self.settings.insert(k.clone(), v.clone());
            match ServerSettingsKey::from_str_opt(k) {
                Some(ServerSettingsKey::GLOBAL_LOBBY_FREQUENCIES) => {
                    // The C# code uses the *current* culture here; this port
                    // always parses with `.` as decimal separator.
                    self.global_frequencies_hz = v
                        .split(',')
                        .filter_map(|f| f.trim().parse::<f64>().ok())
                        .map(|mhz| mhz * 1e6)
                        .collect();
                }
                Some(ServerSettingsKey::RETRANSMISSION_NODE_LIMIT) => {
                    // C# quirk: an unparsable value leaves the previous limit untouched.
                    if let Ok(n) = v.trim().parse::<i32>() {
                        self.retransmit_node_limit = n;
                    }
                }
                Some(ServerSettingsKey::SERVER_PRESETS) => {
                    // Dictionary keys keep their case (STJ only matches
                    // *property* names case-insensitively); the channel
                    // objects inside are case-insensitive.
                    self.server_presets = parse_server_presets(v).unwrap_or_default();
                }
                Some(ServerSettingsKey::SERVER_EAM_RADIO_PRESET) => {
                    let radios = from_json_case_insensitive::<Vec<DcsRadioCustom>>(v).unwrap_or_default();
                    let valid = radios.len() == crate::constants::MAX_RADIOS
                        && matches!(radios[0].modulation, Modulation::Disabled | Modulation::Intercom);
                    self.custom_eam_radios = if valid { radios } else { Vec::new() };
                }
                _ => {}
            }
        }
    }

    /// Raw value, falling back to the C# default, then `""`.
    pub fn get(&self, key: ServerSettingsKey) -> &str {
        self.settings
            .get(key.as_str())
            .map(String::as_str)
            .unwrap_or(key.default_value())
    }

    /// `Convert.ToBoolean` semantics on the string value (case-insensitive
    /// `true`); anything else is false.
    pub fn get_bool(&self, key: ServerSettingsKey) -> bool {
        self.get(key).trim().eq_ignore_ascii_case("true")
    }

    pub fn get_i32(&self, key: ServerSettingsKey) -> Option<i32> {
        self.get(key).trim().parse().ok()
    }

    /// `GetPresetChannels`: first preset whose key is a prefix of `radio_name`.
    pub fn preset_channels(&self, radio_name: &str) -> &[ServerPresetChannel] {
        self.server_presets
            .iter()
            .find(|(prefix, _)| radio_name.starts_with(prefix.as_str()))
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// `TEST_FREQUENCIES` in Hz (the client's "echo" frequencies).
    pub fn test_frequencies_hz(&self) -> Vec<f64> {
        self.get(ServerSettingsKey::TEST_FREQUENCIES)
            .split(',')
            .filter_map(|f| f.trim().parse::<f64>().ok())
            .map(|mhz| mhz * 1e6)
            .collect()
    }

    pub fn raw(&self) -> &HashMap<String, String> {
        &self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_are_csharp_enum_names() {
        assert_eq!(ServerSettingsKey::LOS_ENABLED.as_str(), "LOS_ENABLED");
        assert_eq!(
            ServerSettingsKey::from_str_opt("HTTP_SERVER_ADDRESS"),
            Some(ServerSettingsKey::HTTP_SERVER_ADDRESS)
        );
        assert_eq!(ServerSettingsKey::ALL.len(), 39);
        assert_eq!(ServerSettingsKey::HTTP_SERVER_ADDRESS as i32, 38);
    }

    #[test]
    fn decode_specials() {
        let mut s = SyncedServerSettings::new();
        let mut m = HashMap::new();
        m.insert("GLOBAL_LOBBY_FREQUENCIES".into(), "248.22, 251".into());
        m.insert("RETRANSMISSION_NODE_LIMIT".into(), "3".into());
        m.insert("LOS_ENABLED".into(), "True".into());
        m.insert(
            "SERVER_PRESETS".into(),
            r#"{"ARC210":[{"Name":"Tower","Frequency":251000000,}], "uhf": [ /* c */ {"name":"x","frequency":1e8}]}"#.into(),
        );
        s.decode(&m);
        assert_eq!(s.global_frequencies_hz, vec![248_220_000.0, 251_000_000.0]);
        assert_eq!(s.retransmit_node_limit, 3);
        assert!(s.get_bool(ServerSettingsKey::LOS_ENABLED));
        assert!(!s.get_bool(ServerSettingsKey::DISTANCE_ENABLED));
        assert_eq!(s.get(ServerSettingsKey::SERVER_PORT), "5002");
        assert_eq!(s.preset_channels("ARC210_1")[0].name, "Tower");
        assert_eq!(s.preset_channels("uhf2")[0].frequency, 1e8);
        assert!(s.preset_channels("vhf").is_empty());
        assert_eq!(s.test_frequencies_hz(), vec![247_200_000.0, 120_300_000.0]);
    }

    #[test]
    fn stripper() {
        let cleaned = strip_json_comments_and_trailing_commas(
            "{\"a\": [1, 2, /* x */ ], // c\n \"b\": \"k,}\", \"c\": {\"d\": 1,}, }",
        );
        let v: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(v["a"].as_array().unwrap().len(), 2);
        assert_eq!(v["b"], "k,}");
        assert_eq!(v["c"]["d"], 1);
    }

    #[test]
    fn eam_preset_validation() {
        let mut s = SyncedServerSettings::new();
        let mut m = HashMap::new();
        let eleven: Vec<String> = (0..11)
            .map(|i| {
                if i == 0 {
                    r#"{"modulation":2,"name":"Intercom"}"#.to_string()
                } else {
                    format!(r#"{{"Modulation":0,"Freq":{}e6,"encKey":1.0}}"#, 100 + i)
                }
            })
            .collect();
        m.insert(
            "SERVER_EAM_RADIO_PRESET".into(),
            format!("[{}]", eleven.join(",")),
        );
        s.decode(&m);
        assert_eq!(s.custom_eam_radios.len(), 11);
        assert_eq!(s.custom_eam_radios[1].freq, 101e6);
        assert_eq!(s.custom_eam_radios[1].enc_key, 1);
        // Ten radios: invalid.
        m.insert(
            "SERVER_EAM_RADIO_PRESET".into(),
            format!("[{}]", eleven[..10].join(",")),
        );
        s.decode(&m);
        assert!(s.custom_eam_radios.is_empty());
    }
}
