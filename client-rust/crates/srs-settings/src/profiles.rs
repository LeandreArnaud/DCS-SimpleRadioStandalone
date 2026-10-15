//! Profile management (`ProfileSettingsStore`: add / copy / rename /
//! remove) and the automatic profile selection for an aircraft
//! (`ClientSettingsViewModel.HandleAsync(NewUnitEnteredMessage)`).

use std::path::Path;

use crate::global::GlobalSettings;
use crate::keys::GlobalSettingsKey;
use crate::profile::ProfileSettings;

/// `InputProfileWindow.CleanString`: strip invalid path characters and
/// `.cfg`; `default` becomes `default 1`.
pub fn clean_profile_name(name: &str) -> String {
    const INVALID: &[char] = &['"', '<', '>', '|', ':', '*', '?', '\\', '/'];
    let cleaned: String = name
        .chars()
        .filter(|c| !INVALID.contains(c) && !c.is_control())
        .collect();
    let cleaned = cleaned.replace(".cfg", "");
    if cleaned == "default" {
        return "default 1".to_string();
    }
    cleaned.trim().to_string()
}

fn set_profiles(settings: &mut GlobalSettings, names: &[String]) {
    settings.set_string_array(GlobalSettingsKey::SettingsProfiles, names);
}

/// `AddNewProfile`: registers the name; the file is written on first save.
pub fn add_profile(settings: &mut GlobalSettings, name: &str) {
    let mut names = settings.profiles();
    if !names.iter().any(|n| n == name) {
        names.push(name.to_string());
        set_profiles(settings, &names);
    }
}

/// `CopyProfile`: copies `<src>.cfg` to `<dst>.cfg` (the source must exist).
pub fn copy_profile(settings: &mut GlobalSettings, src: &str, dst: &str) -> std::io::Result<()> {
    let dir = settings.dir().to_path_buf();
    let from = dir.join(ProfileSettings::file_name(src));
    let to = dir.join(ProfileSettings::file_name(dst));
    let bytes = std::fs::read(&from)?;
    std::fs::write(&to, bytes)?;
    add_profile(settings, dst);
    Ok(())
}

/// `RenameProfile`: `<old>.cfg` → `<new>.cfg`, list updated (new name at the end).
pub fn rename_profile(settings: &mut GlobalSettings, old: &str, new: &str) -> std::io::Result<()> {
    let dir = settings.dir().to_path_buf();
    let from = dir.join(ProfileSettings::file_name(old));
    let to = dir.join(ProfileSettings::file_name(new));
    match std::fs::rename(&from, &to) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Never saved yet: create the new file empty like the C# save would.
            std::fs::write(&to, b"")?;
        }
        Err(e) => return Err(e),
    }
    let mut names: Vec<String> = settings.profiles().into_iter().filter(|n| n != old).collect();
    names.push(new.to_string());
    set_profiles(settings, &names);
    Ok(())
}

/// `RemoveProfile`: list updated and `<name>.cfg` deleted (errors ignored).
pub fn remove_profile(settings: &mut GlobalSettings, name: &str) {
    let names: Vec<String> = settings.profiles().into_iter().filter(|n| n != name).collect();
    set_profiles(settings, &names);
    let path = settings.dir().join(ProfileSettings::file_name(name));
    if let Err(e) = std::fs::remove_file(&path) {
        log::info!("Could not delete profile file {}: {e}", path.display());
    }
}

fn alnum_lower(s: &str, keep_underscore: bool) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || (keep_underscore && *c == '_'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Profile to select for `unit` in `seat` (0-based), or `default`.
///
/// Pass 1: profiles whose prefix (before `_`) matches and that end with
/// `_<seat+1>`. Pass 2: any profile whose normalised name (underscores
/// kept, a C# quirk) is a prefix of the unit name.
pub fn auto_select_profile(profiles: &[String], unit: &str, seat: i32) -> String {
    let name = alnum_lower(unit.trim(), false);
    let name_seat = format!("_{}", seat + 1);
    for p in profiles {
        let trimmed = p.trim();
        let split = trimmed.to_lowercase();
        let prefix = split.split('_').next().unwrap_or_default();
        if name.starts_with(&alnum_lower(prefix, false)) && trimmed.ends_with(&name_seat) {
            return p.clone();
        }
    }
    for p in profiles {
        if name.starts_with(&alnum_lower(&p.trim().to_lowercase(), true)) {
            return p.clone();
        }
    }
    "default".to_string()
}

/// Does `<dir>/<profile>.cfg` exist?
pub fn profile_file_exists(dir: &Path, profile: &str) -> bool {
    dir.join(ProfileSettings::file_name(profile)).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn clean_names() {
        assert_eq!(clean_profile_name(" F-14B.cfg "), "F-14B");
        assert_eq!(clean_profile_name("default"), "default 1");
        assert_eq!(clean_profile_name("a:b*c?"), "abc");
    }

    #[test]
    fn auto_select_rules() {
        let p = names(&["default", "F14", "F14_2", "AH64D_2", "F-18"]);
        assert_eq!(auto_select_profile(&p, "F-14B", 0), "F14");
        assert_eq!(auto_select_profile(&p, "F-14B", 1), "F14_2");
        assert_eq!(auto_select_profile(&p, "AH-64D_BLK_II", 1), "AH64D_2");
        assert_eq!(auto_select_profile(&p, "AH-64D_BLK_II", 0), "default");
        // Pass 2 keeps underscores, so "F-18" ("f18") matches FA-18C? No: "fa18c" does not start with "f18".
        assert_eq!(auto_select_profile(&p, "FA-18C_hornet", 0), "default");
        assert_eq!(auto_select_profile(&p, "F-18C", 0), "F-18");
        let empty = names(&["default", "_2"]);
        assert_eq!(
            auto_select_profile(&empty, "Anything", 1),
            "_2",
            "empty prefix matches everything (C# quirk)"
        );
    }

    #[test]
    fn profile_file_ops() {
        let dir = std::env::temp_dir().join(format!("srs-profiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut g = GlobalSettings::load(&dir).unwrap();
        assert_eq!(g.profiles(), names(&["default"]));
        let _ = ProfileSettings::load(&dir, "default").unwrap();
        add_profile(&mut g, "F14");
        assert_eq!(g.profiles(), names(&["default", "F14"]));
        copy_profile(&mut g, "default", "F14_2").unwrap();
        assert!(profile_file_exists(&dir, "F14_2"));
        rename_profile(&mut g, "F14_2", "F14_RIO").unwrap();
        assert!(!profile_file_exists(&dir, "F14_2") && profile_file_exists(&dir, "F14_RIO"));
        assert_eq!(g.profiles(), names(&["default", "F14", "F14_RIO"]));
        remove_profile(&mut g, "F14_RIO");
        assert!(!profile_file_exists(&dir, "F14_RIO"));
        assert_eq!(g.profiles(), names(&["default", "F14"]));
        std::fs::remove_dir_all(&dir).ok();
    }
}
