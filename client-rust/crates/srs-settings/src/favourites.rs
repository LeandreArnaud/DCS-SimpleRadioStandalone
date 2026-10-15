//! `FavouriteServers.csv` (`CsvFavouriteServerStore`): one line per server,
//! `Name,Address,IsDefault,EAMCoalitionPassword`, no quoting, UTF-8 without
//! BOM, in the working directory (it does not follow `-cfg`).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAddress {
    pub name: String,
    pub address: String,
    pub is_default: bool,
    pub eam_password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FavouriteStore {
    path: PathBuf,
    pub servers: Vec<ServerAddress>,
}

impl FavouriteStore {
    pub const FILE_NAME: &'static str = "FavouriteServers.csv";

    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join(Self::FILE_NAME)
    }

    /// Load; a missing file gives an empty list, a read error is logged.
    pub fn load(path: PathBuf) -> Self {
        let servers = match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                log::error!("Failed to load favourite servers from {}: {e}", path.display());
                Vec::new()
            }
        };
        Self { path, servers }
    }

    /// `LoadFromStore`: lines with fewer than 3 fields or a bad bool are skipped.
    pub fn parse(text: &str) -> Vec<ServerAddress> {
        let mut out = Vec::new();
        for line in text.lines() {
            let split: Vec<&str> = line.split(',').collect();
            if split.len() < 3 {
                if !line.trim().is_empty() {
                    log::warn!("Failed to parse favourite server line: {line}");
                }
                continue;
            }
            let Some(is_default) = parse_bool(split[2]) else {
                log::warn!("Failed to parse favourite server line: {line}");
                continue;
            };
            let eam_password = split
                .get(3)
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string());
            out.push(ServerAddress {
                name: split[0].to_string(),
                address: split[1].to_string(),
                is_default,
                eam_password,
            });
        }
        out
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for a in &self.servers {
            s.push_str(&format!(
                "{},{},{},{}\n",
                a.name,
                a.address,
                if a.is_default { "True" } else { "False" },
                a.eam_password.as_deref().unwrap_or("")
            ));
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::write(&self.path, self.to_text())
    }

    /// `Add`: the first favourite becomes the default.
    pub fn add(&mut self, name: &str, address: &str, eam_password: &str) {
        let is_default = self.servers.is_empty();
        self.servers.push(ServerAddress {
            name: name.to_string(),
            address: address.to_string(),
            is_default,
            eam_password: if eam_password.trim().is_empty() {
                None
            } else {
                Some(eam_password.to_string())
            },
        });
    }

    /// `Remove`: when the list becomes empty, `last_server` (if any) is re-added as default.
    pub fn remove(&mut self, index: usize, last_server: &str) {
        if index < self.servers.len() {
            self.servers.remove(index);
        }
        if self.servers.is_empty() && !last_server.is_empty() {
            self.servers.push(ServerAddress {
                name: last_server.to_string(),
                address: last_server.to_string(),
                is_default: true,
                eam_password: None,
            });
        }
    }

    /// Make `index` the only default.
    pub fn set_default(&mut self, index: usize) {
        for (i, s) in self.servers.iter_mut().enumerate() {
            s.is_default = i == index;
        }
    }

    pub fn move_item(&mut self, from: usize, to: usize) {
        if from < self.servers.len() && to < self.servers.len() && from != to {
            let item = self.servers.remove(from);
            self.servers.insert(to, item);
        }
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_write_roundtrip() {
        let text = "Home,127.0.0.1:5002,True,secret\nSquad,srs.example.org,False,\nbad line\nX,Y,maybe\n";
        let list = FavouriteStore::parse(text);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].eam_password.as_deref(), Some("secret"));
        assert!(list[0].is_default);
        assert_eq!(list[1].eam_password, None);
        let store = FavouriteStore {
            path: PathBuf::new(),
            servers: list,
        };
        assert_eq!(
            store.to_text(),
            "Home,127.0.0.1:5002,True,secret\nSquad,srs.example.org,False,\n"
        );
    }

    #[test]
    fn add_remove_default_rules() {
        let mut s = FavouriteStore {
            path: PathBuf::new(),
            servers: Vec::new(),
        };
        s.add("A", "a:5002", "");
        s.add("B", "b:5002", "pw");
        assert!(s.servers[0].is_default && !s.servers[1].is_default);
        s.set_default(1);
        assert!(!s.servers[0].is_default && s.servers[1].is_default);
        s.remove(0, "last");
        s.remove(0, "last");
        assert_eq!(s.servers.len(), 1);
        assert_eq!(s.servers[0].address, "last");
        assert!(s.servers[0].is_default);
    }
}
