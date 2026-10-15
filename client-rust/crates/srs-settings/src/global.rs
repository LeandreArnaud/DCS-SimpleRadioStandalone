//! `global.cfg` — port of `Common/Settings/GlobalSettingsStore.cs`.
//!
//! Behaviour kept from C#:
//! - The file lives in the configuration directory (`-cfg=<dir>`, default:
//!   the current working directory).
//! - Reading a missing key inserts its default and **saves immediately**.
//! - Every setter saves immediately. The file is written without BOM.
//! - A file that fails to parse is copied to `global.cfg.bak` and replaced
//!   by a fresh default configuration.
//! - `Version` is upgraded from `0` to `1` on load.

use std::path::{Path, PathBuf};

use crate::ini::{Document, Setting};
use crate::keys::GlobalSettingsKey;

pub const GLOBAL_CFG_FILE_NAME: &str = "global.cfg";
const CURRENT_VERSION: i32 = 1;
const SECTIONS: [&str; 3] = ["Position Settings", "Client Settings", "Network Settings"];

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// In-memory copy of `global.cfg`, written back on every change.
#[derive(Debug)]
pub struct GlobalSettings {
    dir: PathBuf,
    doc: Document,
    /// When false (tests, dry runs) nothing is written to disk.
    persist: bool,
}

impl GlobalSettings {
    /// Load from `<dir>/global.cfg`, creating it when missing or corrupt.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let dir = dir.as_ref().to_path_buf();
        let path = dir.join(GLOBAL_CFG_FILE_NAME);
        let mut store = Self {
            dir,
            doc: Document::new(),
            persist: true,
        };
        match std::fs::read(&path) {
            Ok(bytes) => match Document::parse_bytes(&bytes) {
                Ok(doc) => {
                    store.doc = doc;
                    store.upgrade();
                }
                Err(e) => {
                    log::error!(
                        "Failed to parse {}: {e}. Backing up and re-initialising.",
                        path.display()
                    );
                    if let Err(e) = std::fs::copy(&path, path.with_extension("cfg.bak")) {
                        log::error!("Failed to create backup of corrupted config file: {e}");
                    }
                    store.init_default()?;
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::info!("No {} found, initialising with defaults", path.display());
                store.init_default()?;
            }
            Err(source) => return Err(SettingsError::Io { path, source }),
        }
        Ok(store)
    }

    /// In-memory store that never touches the disk (tests).
    pub fn in_memory() -> Self {
        let mut s = Self {
            dir: PathBuf::new(),
            doc: Document::new(),
            persist: false,
        };
        for sec in SECTIONS {
            s.doc.section_or_insert(sec);
        }
        s.set_i32(GlobalSettingsKey::Version, CURRENT_VERSION);
        s
    }

    /// Parse from text without touching the disk (tests, fixtures).
    pub fn from_text(text: &str) -> Result<Self, crate::ini::ParseError> {
        Ok(Self {
            dir: PathBuf::new(),
            doc: Document::parse(text)?,
            persist: false,
        })
    }

    fn init_default(&mut self) -> Result<(), SettingsError> {
        self.doc = Document::new();
        for sec in SECTIONS {
            self.doc.section_or_insert(sec);
        }
        self.set_i32(GlobalSettingsKey::Version, CURRENT_VERSION);
        self.save()
    }

    fn upgrade(&mut self) {
        if self.get_i32(GlobalSettingsKey::Version) == 0 {
            self.set_i32(GlobalSettingsKey::Version, CURRENT_VERSION);
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(GLOBAL_CFG_FILE_NAME)
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn to_text(&self) -> String {
        self.doc.to_text()
    }

    /// Write `global.cfg` (UTF-8, no BOM). No-op for in-memory stores.
    pub fn save(&self) -> Result<(), SettingsError> {
        if !self.persist {
            return Ok(());
        }
        let path = self.path();
        std::fs::write(&path, self.doc.to_bytes(false)).map_err(|source| SettingsError::Io { path, source })
    }

    fn save_logged(&self) {
        if let Err(e) = self.save() {
            log::error!("Unable to save settings! {e}");
        }
    }

    /// `GetSetting`: returns the setting, inserting its default (and saving)
    /// when absent — exactly like the C# store.
    fn setting(&mut self, key: GlobalSettingsKey) -> &Setting {
        let section = self.doc.section_or_insert(key.section());
        let default = match key {
            GlobalSettingsKey::SettingsProfiles => "{default.cfg}".to_string(),
            GlobalSettingsKey::ClientIdLong => new_guid_string(),
            k => k.default_value().unwrap_or("").to_string(),
        };
        let (_, created) = section.get_or_insert(key.as_str(), &default);
        if created {
            self.save_logged();
        }
        self.doc
            .section(key.section())
            .and_then(|s| s.get(key.as_str()))
            .expect("just inserted")
    }

    pub fn get_string(&mut self, key: GlobalSettingsKey) -> String {
        self.setting(key).string_value().to_string()
    }

    /// `RawValue`, quotes included.
    pub fn get_raw(&mut self, key: GlobalSettingsKey) -> String {
        self.setting(key).raw.clone()
    }

    /// `GetClientSettingBool`: empty raw value is `false`; unparsable values
    /// throw in C# — here they are `false`.
    pub fn get_bool(&mut self, key: GlobalSettingsKey) -> bool {
        self.setting(key).bool_value().unwrap_or(false)
    }

    pub fn get_i32(&mut self, key: GlobalSettingsKey) -> i32 {
        self.setting(key).i32_value().unwrap_or(0)
    }

    pub fn get_f64(&mut self, key: GlobalSettingsKey) -> f64 {
        self.setting(key).f64_value().unwrap_or(0.0)
    }

    /// `GetNetworkSetting`: a missing/empty port falls back to the default.
    pub fn get_port(&mut self, key: GlobalSettingsKey) -> u16 {
        let s = self.setting(key);
        let v = if s.is_empty() {
            key.default_value().unwrap_or("0").parse().unwrap_or(0)
        } else {
            s.i32_value().unwrap_or(0)
        };
        u16::try_from(v).unwrap_or(0)
    }

    /// `SettingsProfiles`; guarantees `default` is present (C# `GetProfiles`).
    pub fn profiles(&mut self) -> Vec<String> {
        let arr = self.setting(GlobalSettingsKey::SettingsProfiles).string_array();
        match arr {
            // C# `profiles.Contains("default")`: exact element match, so the
            // initial `{default.cfg}` default is immediately rewritten as
            // `{default}`.
            Some(list) if !list.is_empty() && list.iter().any(|p| p == "default") => list,
            _ => {
                let fixed = vec!["default".to_string()];
                self.set_string_array(GlobalSettingsKey::SettingsProfiles, &fixed);
                fixed
            }
        }
    }

    fn set_with(&mut self, key: GlobalSettingsKey, f: impl FnOnce(&mut Setting)) {
        let section = self.doc.section_or_insert(key.section());
        let (setting, _) = section.get_or_insert(key.as_str(), "");
        f(setting);
        self.save_logged();
    }

    pub fn set_string(&mut self, key: GlobalSettingsKey, value: &str) {
        self.set_with(key, |s| s.set_string(value));
    }

    pub fn set_raw(&mut self, key: GlobalSettingsKey, value: &str) {
        self.set_with(key, |s| s.set_raw(value));
    }

    pub fn set_bool(&mut self, key: GlobalSettingsKey, value: bool) {
        self.set_with(key, |s| s.set_bool(value));
    }

    pub fn set_i32(&mut self, key: GlobalSettingsKey, value: i32) {
        self.set_with(key, |s| s.set_i32(value));
    }

    pub fn set_f64(&mut self, key: GlobalSettingsKey, value: f64) {
        self.set_with(key, |s| s.set_f64(value));
    }

    pub fn set_string_array(&mut self, key: GlobalSettingsKey, values: &[String]) {
        self.set_with(key, |s| s.set_string_array(values));
    }
}

/// `Guid.NewGuid().ToString()` format (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`),
/// used for the `ClientIdLong` default. Not used on the network.
fn new_guid_string() -> String {
    let b: [u8; 16] = std::array::from_fn(|_| {
        use std::hash::{BuildHasher, Hasher};
        (std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
            & 0xFF) as u8
    });
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_inserted_on_read() {
        let mut g = GlobalSettings::in_memory();
        assert!(g.get_bool(GlobalSettingsKey::AutoConnect));
        assert_eq!(g.get_port(GlobalSettingsKey::DCSIncomingUDP), 9084);
        assert_eq!(g.get_f64(GlobalSettingsKey::SpeakerBoost), 0.514);
        assert_eq!(g.get_string(GlobalSettingsKey::LastServer), "127.0.0.1");
        assert_eq!(g.profiles(), vec!["default"]);
        assert_eq!(g.get_i32(GlobalSettingsKey::IdleTimeOut), 600);
        let text = g.to_text();
        assert!(text.contains("[Network Settings]\r\nDCSIncomingUDP=9084"));
        assert!(text.contains("SettingsProfiles={default}"), "{text}");
        assert!(text.starts_with("[Position Settings]\r\n"));
    }

    #[test]
    fn setters_write_dotnet_formats() {
        let mut g = GlobalSettings::in_memory();
        g.set_bool(GlobalSettingsKey::DarkMode, true);
        g.set_f64(GlobalSettingsKey::RadioOpacity, 0.5);
        g.set_string(GlobalSettingsKey::LastSeenName, "Maverick 1");
        g.set_string_array(
            GlobalSettingsKey::SettingsProfiles,
            &["default".into(), "F18_1".into()],
        );
        let text = g.to_text();
        assert!(text.contains("DarkMode=True"));
        assert!(text.contains("[Position Settings]\r\nRadioOpacity=0.5"));
        assert!(text.contains("LastSeenName=\"Maverick 1\""));
        assert!(text.contains("SettingsProfiles={default,F18_1}"));
        assert_eq!(g.get_string(GlobalSettingsKey::LastSeenName), "Maverick 1");
        assert_eq!(g.profiles(), vec!["default", "F18_1"]);
    }

    #[test]
    fn load_save_roundtrip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = GlobalSettings::load(dir.path()).unwrap();
        assert!(dir.path().join("global.cfg").exists());
        g.set_string(GlobalSettingsKey::LastServer, "srs.example.org:5002");
        let bytes = std::fs::read(dir.path().join("global.cfg")).unwrap();
        assert_ne!(
            &bytes[..3],
            &[0xEF, 0xBB, 0xBF],
            "global.cfg must not carry a BOM"
        );
        let mut again = GlobalSettings::load(dir.path()).unwrap();
        assert_eq!(
            again.get_string(GlobalSettingsKey::LastServer),
            "srs.example.org:5002"
        );
        assert_eq!(again.get_i32(GlobalSettingsKey::Version), 1);
    }

    #[test]
    fn corrupt_file_is_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("global.cfg"), "[broken\r\n").unwrap();
        let mut g = GlobalSettings::load(dir.path()).unwrap();
        assert!(dir.path().join("global.cfg.bak").exists());
        assert_eq!(g.get_i32(GlobalSettingsKey::Version), 1);
    }
}
