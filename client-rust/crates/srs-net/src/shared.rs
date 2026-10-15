//! State shared between the network tasks and the rest of the client
//! (`ConnectedClientsSingleton` + `SyncedServerSettings` in C#).

use std::collections::HashMap;
use std::sync::RwLock;

use srs_protocol::{SrClient, SyncedServerSettings};

/// Extra, client-local fields the C# code keeps on `SRClientBase` but never
/// sends (`[JsonIgnore]`).
#[derive(Debug, Clone, Default)]
pub struct ClientLocal {
    /// 0.0 = full line of sight. Updated from the DCS LOS replies.
    pub line_of_sight_loss: f32,
    /// Locally muted by the user (client list window).
    pub muted: bool,
}

/// A remote (or our own echoed) client as known to this client.
#[derive(Debug, Clone, Default)]
pub struct ConnectedClient {
    pub client: SrClient,
    pub local: ClientLocal,
}

#[derive(Debug, Default)]
pub struct SharedState {
    /// Keyed by `ClientGuid`. Includes our own entry once the server echoed it.
    pub clients: RwLock<HashMap<String, ConnectedClient>>,
    pub server_settings: RwLock<SyncedServerSettings>,
}

impl SharedState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn client_count(&self) -> usize {
        self.clients.read().map(|c| c.len()).unwrap_or(0)
    }

    pub fn contains(&self, guid: &str) -> bool {
        self.clients.read().map(|c| c.contains_key(guid)).unwrap_or(false)
    }

    pub fn get(&self, guid: &str) -> Option<ConnectedClient> {
        self.clients.read().ok()?.get(guid).cloned()
    }

    /// Client list window: mute / unmute one client locally.
    pub fn set_muted(&self, guid: &str, muted: bool) {
        if let Ok(mut c) = self.clients.write() {
            if let Some(cl) = c.get_mut(guid) {
                cl.local.muted = muted;
            }
        }
    }

    pub fn clear(&self) {
        if let Ok(mut c) = self.clients.write() {
            c.clear();
        }
    }

    /// Snapshot of all clients sorted by name (UI convenience).
    pub fn snapshot(&self) -> Vec<ConnectedClient> {
        let mut v: Vec<ConnectedClient> = self
            .clients
            .read()
            .map(|c| c.values().cloned().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| a.client.name.cmp(&b.client.name));
        v
    }
}
