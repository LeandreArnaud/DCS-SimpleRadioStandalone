//! Preset channel files and the per-radio channel list: port of
//! `FilePresetChannelsStore.cs` and `PresetChannelsViewModel.Reload`.
//!
//! A radio's presets come from `<presets folder>/<radio>.txt` (one
//! `name|MHz` or `MHz` per line; MIDS files hold channel numbers) and from
//! the server's `SERVER_PRESETS`, combined according to the profile's
//! `ServerPresetSelection`.

use std::path::{Path, PathBuf};

use srs_protocol::radio_calc::{link16, MHZ};
use srs_protocol::server_settings::ServerPresetChannel;
use srs_protocol::Modulation;

use crate::radio_helper::PresetChannel;

/// `ServerPresetConfiguration` (profile `ServerPresetSelection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresetSource {
    UseServerOnlyIfSet,
    UseClientOnly,
    #[default]
    UseClientAndServerIfSet,
}

impl PresetSource {
    /// Unknown strings fall back to the default, like `TryParse`.
    pub fn from_setting(s: &str) -> Self {
        match s {
            "USE_SERVER_ONLY_IF_SET" => Self::UseServerOnlyIfSet,
            "USE_CLIENT_ONLY" => Self::UseClientOnly,
            _ => Self::UseClientAndServerIfSet,
        }
    }
}

/// `NormaliseString`: ASCII alphanumerics only, lowercase.
pub fn normalise(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// `FindRadioFile`: first `.txt` whose normalised stem is a prefix of the
/// normalised radio name (directory order, like `EnumerateFiles`).
pub fn find_radio_file(folder: &Path, radio_name: &str) -> Option<PathBuf> {
    let wanted = normalise(radio_name);
    let entries = std::fs::read_dir(folder).ok()?;
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    files.sort();
    files.into_iter().find(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
            && p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|stem| wanted.starts_with(&normalise(stem)))
    })
}

fn split_line(trimmed: &str) -> (String, &str) {
    let mut parts = trimmed.splitn(2, '|');
    let first = parts.next().unwrap_or_default();
    match parts.next() {
        Some(second) => (first.to_string(), second),
        None => (trimmed.to_string(), trimmed),
    }
}

/// `ReadFrequenciesFromFile`: `name|MHz` or `MHz` per line.
pub fn parse_frequencies(text: &str) -> Vec<PresetChannel> {
    let mut channels = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (name, value) = split_line(trimmed);
        match value.trim().parse::<f64>() {
            Ok(mhz) => channels.push(PresetChannel {
                text: name,
                value: mhz * MHZ,
                channel: 0,
                mids_channel: 0,
            }),
            Err(_) => log::info!("Error parsing frequency  {trimmed}"),
        }
    }
    channels
}

/// `ReadMidsFrequenciesFromFile`: `name|channel` or `channel` per line, 1..126.
pub fn parse_mids_channels(text: &str) -> Vec<PresetChannel> {
    let mut channels = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (name, value) = split_line(trimmed);
        match value.trim().parse::<i32>() {
            Ok(ch) if ch > 0 && ch < 127 => channels.push(PresetChannel {
                text: name,
                value: link16::channel_to_frequency(ch),
                channel: 0,
                mids_channel: ch,
            }),
            Ok(_) => {}
            Err(_) => log::info!("Error parsing frequency  {trimmed}"),
        }
    }
    channels
}

/// `LoadFromStore`.
pub fn load_from_store(folder: &Path, radio_name: &str, mids: bool) -> Vec<PresetChannel> {
    let Some(file) = find_radio_file(folder, radio_name) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Vec::new();
    };
    if mids {
        parse_mids_channels(&text)
    } else {
        parse_frequencies(&text)
    }
}

/// `NameChannel`.
fn name_channel(p: &mut PresetChannel) {
    p.text = if p.mids_channel > 0 {
        format!("{} | {}", p.text, p.mids_channel)
    } else {
        format!("{}: {}", p.channel, p.text)
    };
}

/// `PresetChannelsViewModel.Reload`: build the channel list for one radio.
/// `min`/`max` are the radio's frequency limits in Hz.
pub fn build_channel_list(
    modulation: Modulation,
    min: f64,
    max: f64,
    client: Vec<PresetChannel>,
    server: &[ServerPresetChannel],
    source: PresetSource,
) -> Vec<PresetChannel> {
    let mut client_list: Vec<PresetChannel> = Vec::new();
    let mut server_list: Vec<PresetChannel> = Vec::new();
    if modulation != Modulation::Mids {
        client_list.extend(client.into_iter().filter(|c| c.value <= max && c.value >= min));
        for ch in server {
            let freq = ch.frequency * MHZ;
            if freq <= max && freq >= min {
                server_list.push(PresetChannel {
                    text: ch.name.clone(),
                    value: freq,
                    channel: 0,
                    mids_channel: 0,
                });
            }
        }
    } else {
        client_list.extend(client);
        for ch in server {
            let mids = ch.frequency as i32;
            if mids > 0 && mids < 127 {
                server_list.push(PresetChannel {
                    text: ch.name.clone(),
                    value: link16::channel_to_frequency(mids),
                    channel: 0,
                    mids_channel: mids,
                });
            }
        }
    }
    let mut out: Vec<PresetChannel> = match source {
        PresetSource::UseClientOnly => client_list,
        PresetSource::UseServerOnlyIfSet => {
            if server_list.is_empty() {
                client_list
            } else {
                server_list
            }
        }
        PresetSource::UseClientAndServerIfSet => {
            client_list.extend(server_list);
            client_list
        }
    };
    for (i, p) in out.iter_mut().enumerate() {
        p.channel = i as i32 + 1;
        name_channel(p);
    }
    if modulation == Modulation::Mids && out.is_empty() {
        out = (1..127)
            .map(|chn| PresetChannel {
                mids_channel: chn,
                channel: chn,
                text: format!("MIDS {chn}"),
                value: link16::channel_to_frequency(chn),
            })
            .collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_and_find_file() {
        assert_eq!(normalise("AN/ARC-210 (V)"), "anarc210v");
        let dir = std::env::temp_dir().join(format!("srs-presets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // The file's normalised stem must be a prefix of the normalised radio name.
        std::fs::write(dir.join("AN_ARC-210.txt"), "Tower|251.0\n 305.5 \nbad|xx\n\n").unwrap();
        std::fs::write(dir.join("notes.md"), "ARC210").unwrap();
        let f = find_radio_file(&dir, "AN/ARC-210(V) Radio").unwrap();
        assert!(f.ends_with("AN_ARC-210.txt"));
        assert!(find_radio_file(&dir, "UHF").is_none());
        assert!(
            find_radio_file(&dir, "ARC-210").is_none(),
            "file name longer than radio name"
        );
        let list = load_from_store(&dir, "AN/ARC-210(V) Radio", false);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].text, "Tower");
        assert_eq!(list[0].value, 251e6);
        assert_eq!(list[1].text, "305.5");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mids_parsing_and_defaults() {
        let l = parse_mids_channels("A|5\n200\n7\n");
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].mids_channel, 5);
        assert_eq!(l[1].text, "7");
        let out = build_channel_list(
            Modulation::Mids,
            0.0,
            1e12,
            Vec::new(),
            &[],
            PresetSource::default(),
        );
        assert_eq!(out.len(), 126);
        assert_eq!(out[0].text, "MIDS 1");
        let server = vec![ServerPresetChannel {
            name: "S".into(),
            frequency: 3.0,
        }];
        let out = build_channel_list(
            Modulation::Mids,
            0.0,
            1e12,
            Vec::new(),
            &server,
            PresetSource::default(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "S | 3");
    }

    #[test]
    fn source_selection_and_numbering() {
        let client = vec![PresetChannel {
            text: "C".into(),
            value: 251e6,
            channel: 0,
            mids_channel: 0,
        }];
        let server = vec![
            ServerPresetChannel {
                name: "S1".into(),
                frequency: 252.0,
            },
            ServerPresetChannel {
                name: "Out".into(),
                frequency: 100.0,
            },
        ];
        let both = build_channel_list(
            Modulation::Am,
            225e6,
            400e6,
            client.clone(),
            &server,
            PresetSource::UseClientAndServerIfSet,
        );
        assert_eq!(
            both.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            vec!["1: C", "2: S1"]
        );
        let only = build_channel_list(
            Modulation::Am,
            225e6,
            400e6,
            client.clone(),
            &server,
            PresetSource::UseClientOnly,
        );
        assert_eq!(only.len(), 1);
        let srv = build_channel_list(
            Modulation::Am,
            225e6,
            400e6,
            client.clone(),
            &server,
            PresetSource::UseServerOnlyIfSet,
        );
        assert_eq!(srv[0].text, "1: S1");
        let fallback = build_channel_list(
            Modulation::Am,
            225e6,
            400e6,
            client,
            &[],
            PresetSource::UseServerOnlyIfSet,
        );
        assert_eq!(fallback[0].text, "1: C");
        assert_eq!(
            PresetSource::from_setting("garbage"),
            PresetSource::UseClientAndServerIfSet
        );
    }
}
