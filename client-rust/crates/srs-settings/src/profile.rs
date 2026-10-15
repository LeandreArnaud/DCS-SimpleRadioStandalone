//! `<profile>.cfg` — port of `Common/Settings/ProfileSettingsStore.cs`.
//!
//! One file per profile listed in `global.cfg` `SettingsProfiles`. Layout:
//!
//! ```text
//! [Client Settings]
//! RadioEffectsRatio=1.0
//! ...
//! [Ptt]
//! name=Joystick - HOTAS Warthog
//! button=5
//! value=0
//! guid=12345678-1234-1234-1234-123456789abc
//! ```
//!
//! Each input binding is its own section named after the `InputBinding`
//! variant. `button` is a SharpDX index (see `srs-input`), `value` the POV
//! angle for hat bindings, `guid` the DirectInput instance GUID.
//!
//! C# quirks kept: a missing key is inserted with its default and the file is
//! saved; every setter saves. Files are written **with** a UTF-8 BOM
//! (`Encoding.UTF8` default), except that the C# code writes the default
//! profile relative to the working directory instead of the config
//! directory; this port always uses the config directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::global::SettingsError;
use crate::ini::{Document, Setting};
use crate::keys::{InputBinding, ProfileSettingsKey};

const CLIENT_SETTINGS: &str = "Client Settings";

/// A stored input binding (`Common/Settings/Input/InputDevice.cs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDevice {
    pub bind: InputBinding,
    pub device_name: String,
    pub button: i32,
    /// POV angle (hundredths of a degree) for hat bindings, else 0.
    pub button_value: i32,
    /// DirectInput instance GUID in `Guid.ToString()` form (lowercase, dashed).
    pub instance_guid: String,
}

#[derive(Debug)]
pub struct ProfileSettings {
    name: String,
    path: PathBuf,
    doc: Document,
    persist: bool,
}

impl ProfileSettings {
    /// File name for a profile (`default` -> `default.cfg`).
    pub fn file_name(profile: &str) -> String {
        if profile.contains(".cfg") {
            profile.to_string()
        } else {
            format!("{profile}.cfg")
        }
    }

    /// Profile name for a file (`default.cfg` -> `default`).
    pub fn profile_name(file: &str) -> String {
        file.replace(".cfg", "")
    }

    /// Load `<dir>/<profile>.cfg`; a missing or corrupt file yields an empty
    /// profile which is written to disk immediately (C# behaviour).
    pub fn load(dir: impl AsRef<Path>, profile: &str) -> Result<Self, SettingsError> {
        let path = dir.as_ref().join(Self::file_name(profile));
        let doc = match std::fs::read(&path) {
            Ok(bytes) => Document::parse_bytes(&bytes).unwrap_or_else(|e| {
                log::info!(
                    "Error with input config {}: {e} - creating a new default",
                    path.display()
                );
                Document::new()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::info!(
                    "Did not find input config file at {}, initialising with default config",
                    path.display()
                );
                Document::new()
            }
            Err(source) => return Err(SettingsError::Io { path, source }),
        };
        let p = Self {
            name: Self::profile_name(profile),
            path,
            doc,
            persist: true,
        };
        p.save()?;
        Ok(p)
    }

    pub fn in_memory(profile: &str) -> Self {
        Self {
            name: profile.to_string(),
            path: PathBuf::new(),
            doc: Document::new(),
            persist: false,
        }
    }

    pub fn from_text(profile: &str, text: &str) -> Result<Self, crate::ini::ParseError> {
        Ok(Self {
            name: profile.to_string(),
            path: PathBuf::new(),
            doc: Document::parse(text)?,
            persist: false,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn to_text(&self) -> String {
        self.doc.to_text()
    }

    /// Write the file (UTF-8 with BOM). No-op for in-memory profiles.
    pub fn save(&self) -> Result<(), SettingsError> {
        if !self.persist {
            return Ok(());
        }
        std::fs::write(&self.path, self.doc.to_bytes(true)).map_err(|source| SettingsError::Io {
            path: self.path.clone(),
            source,
        })
    }

    fn save_logged(&self) {
        if let Err(e) = self.save() {
            log::error!("Unable to save settings! {e}");
        }
    }

    fn setting(&mut self, key: ProfileSettingsKey) -> &Setting {
        let section = self.doc.section_or_insert(CLIENT_SETTINGS);
        let default = key.default_value().unwrap_or_else(|| {
            log::warn!("Setting {key} not found in default settings, creating with empty value");
            ""
        });
        let (_, created) = section.get_or_insert(key.as_str(), default);
        if created {
            self.save_logged();
        }
        self.doc
            .section(CLIENT_SETTINGS)
            .and_then(|s| s.get(key.as_str()))
            .expect("just inserted")
    }

    /// `GetClientSettingBool`: `GetValueOrDefault(false)`.
    pub fn get_bool(&mut self, key: ProfileSettingsKey) -> bool {
        self.setting(key).bool_value().unwrap_or(false)
    }

    /// `GetClientSettingFloat`: `GetValueOrDefault(0f)`.
    pub fn get_f32(&mut self, key: ProfileSettingsKey) -> f32 {
        self.setting(key).f32_value().unwrap_or(0.0)
    }

    /// `GetClientSettingString`: the **raw** value (quotes included).
    pub fn get_string(&mut self, key: ProfileSettingsKey) -> String {
        self.setting(key).raw.clone()
    }

    fn set_with(&mut self, key: ProfileSettingsKey, f: impl FnOnce(&mut Setting)) {
        let section = self.doc.section_or_insert(CLIENT_SETTINGS);
        let (setting, _) = section.get_or_insert(key.as_str(), "");
        f(setting);
        self.save_logged();
    }

    pub fn set_bool(&mut self, key: ProfileSettingsKey, value: bool) {
        self.set_with(key, |s| s.set_bool(value));
    }

    pub fn set_f32(&mut self, key: ProfileSettingsKey, value: f32) {
        self.set_with(key, |s| s.set_f32(value));
    }

    pub fn set_string(&mut self, key: ProfileSettingsKey, value: &str) {
        self.set_with(key, |s| s.set_string(value));
    }

    // ---- input bindings -------------------------------------------------

    /// `GetControlSetting`: `None` when the section is missing or malformed.
    pub fn binding(&self, bind: InputBinding) -> Option<InputDevice> {
        let section = self.doc.section(bind.as_str())?;
        let device_name = section.get("name")?.string_value().to_string();
        let button = section.get("button")?.i32_value()?;
        let instance_guid = section.get("guid")?.raw.trim().to_string();
        let button_value = section.get("value")?.i32_value()?;
        if instance_guid.is_empty() {
            return None;
        }
        Some(InputDevice {
            bind,
            device_name,
            button,
            button_value,
            instance_guid,
        })
    }

    /// All bindings present in the file, keyed by binding.
    pub fn bindings(&self) -> BTreeMap<InputBinding, InputDevice> {
        InputBinding::ALL
            .iter()
            .filter_map(|b| self.binding(*b).map(|d| (*b, d)))
            .collect()
    }

    /// `SetControlSetting`: replaces the section for that binding.
    pub fn set_binding(&mut self, device: &InputDevice) {
        self.doc.remove_section(device.bind.as_str());
        let section = self.doc.section_or_insert(device.bind.as_str());
        section
            .settings
            .push(Setting::new("name", device.device_name.replace('\0', "")));
        section
            .settings
            .push(Setting::new("button", device.button.to_string()));
        section
            .settings
            .push(Setting::new("value", device.button_value.to_string()));
        section
            .settings
            .push(Setting::new("guid", device.instance_guid.clone()));
        self.save_logged();
    }

    /// `RemoveControlSetting`.
    pub fn remove_binding(&mut self, bind: InputBinding) {
        self.doc.remove_section(bind.as_str());
        self.save_logged();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}[Client Settings]\r\nRadioEffectsRatio=1.0\r\nAllowDCSPTT=True\r\n\r\n[Ptt]\r\nname=\"Joystick - HOTAS Warthog\"\r\nbutton=5\r\nvalue=0\r\nguid=2b7d3ea0-1a2b-11ee-8001-444553540000\r\n\r\n[ModifierPtt]\r\nname=Keyboard\r\nbutton=29\r\nvalue=0\r\nguid=6f1d2b61-d5a0-11cf-bfc7-444553540000\r\n";

    #[test]
    fn reads_bindings_and_settings() {
        let doc = Document::parse_bytes(SAMPLE.as_bytes()).unwrap();
        let mut p = ProfileSettings {
            name: "default".into(),
            path: PathBuf::new(),
            doc,
            persist: false,
        };
        assert!(p.get_bool(ProfileSettingsKey::AllowDCSPTT));
        assert_eq!(p.get_f32(ProfileSettingsKey::RadioEffectsRatio), 1.0);
        // Missing key: default inserted.
        assert_eq!(p.get_f32(ProfileSettingsKey::NATOToneVolume), 1.2);
        assert_eq!(
            p.get_string(ProfileSettingsKey::RadioTransmissionStartSelection),
            "RADIO_TRANS_START.wav"
        );
        let b = p.binding(InputBinding::Ptt).unwrap();
        assert_eq!(b.device_name, "Joystick - HOTAS Warthog");
        assert_eq!(b.button, 5);
        assert_eq!(b.instance_guid, "2b7d3ea0-1a2b-11ee-8001-444553540000");
        assert_eq!(p.bindings().len(), 2);
        assert!(p.binding(InputBinding::Intercom).is_none());
    }

    #[test]
    fn write_binding_matches_csharp_layout() {
        let mut p = ProfileSettings::in_memory("default");
        p.set_binding(&InputDevice {
            bind: InputBinding::Ptt,
            device_name: "Joystick - HOTAS Warthog".into(),
            button: 5,
            button_value: 0,
            instance_guid: "2b7d3ea0-1a2b-11ee-8001-444553540000".into(),
        });
        assert_eq!(
            p.to_text(),
            "[Ptt]\r\nname=\"Joystick - HOTAS Warthog\"\r\nbutton=5\r\nvalue=0\r\nguid=2b7d3ea0-1a2b-11ee-8001-444553540000\r\n"
        );
        p.remove_binding(InputBinding::Ptt);
        assert_eq!(p.to_text(), "");
    }

    #[test]
    fn disk_roundtrip_with_bom() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = ProfileSettings::load(dir.path(), "F18_1").unwrap();
        p.set_bool(ProfileSettingsKey::InstructorMode, true);
        let bytes = std::fs::read(dir.path().join("F18_1.cfg")).unwrap();
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
        let mut again = ProfileSettings::load(dir.path(), "F18_1.cfg").unwrap();
        assert_eq!(again.name(), "F18_1");
        assert!(again.get_bool(ProfileSettingsKey::InstructorMode));
    }
}
