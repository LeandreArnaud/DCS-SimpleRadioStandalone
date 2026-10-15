//! Receive-side audio engine: the set of remote clients (keyed by
//! `OriginalClientGuid`, like `AudioManager._clientsBufferedAudio`) and the
//! master mixer. Shared between the decode thread (`add_client_audio`) and
//! the output callback (`render`) behind a mutex; the critical sections are
//! short and allocation-free in steady state.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use srs_protocol::{Modulation, ShortGuid};

use crate::effects::{EffectCache, EffectSettings, RadioModelFactory};
use crate::master::MasterMixer;
use crate::remote_client::{ClientAudio, IncomingAudioSettings, RemoteClientAudio};

pub struct AudioEngine {
    pub clients: HashMap<ShortGuid, RemoteClientAudio>,
    pub master: MasterMixer,
    pub incoming: IncomingAudioSettings,
    pub effects: EffectSettings,
    pub cache: Arc<EffectCache>,
    pub factory: Arc<RadioModelFactory>,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    /// Engine without any WAV effect or radio model file (defaults only).
    pub fn new() -> Self {
        Self::with_assets(
            Arc::new(EffectCache::default()),
            Arc::new(RadioModelFactory::empty()),
        )
    }

    pub fn with_assets(cache: Arc<EffectCache>, factory: Arc<RadioModelFactory>) -> Self {
        Self {
            clients: HashMap::new(),
            master: MasterMixer::new(cache.clone(), factory.clone()),
            incoming: IncomingAudioSettings::default(),
            effects: EffectSettings::default(),
            cache,
            factory,
        }
    }

    /// Replace the effect/model assets; rebuilds the mixers (keeps volume
    /// and balances) and drops the remote clients.
    pub fn set_assets(&mut self, cache: Arc<EffectCache>, factory: Arc<RadioModelFactory>) {
        let volume = self.master.volume;
        let balances: Vec<f32> = self.master.radios.iter().map(|r| r.balance).collect();
        self.master = MasterMixer::new(cache.clone(), factory.clone());
        self.master.volume = volume;
        for (r, b) in self.master.radios.iter_mut().zip(balances) {
            r.balance = b;
        }
        self.cache = cache;
        self.factory = factory;
        self.clients.clear();
    }

    /// Play a UI sound (connection beeps) over the mix.
    pub fn play_sound(&mut self, sound: &crate::sounds::Sound) {
        if sound.loaded() {
            self.master.play_system(&sound.stereo);
        }
    }

    /// `AudioManager.PlaySoundEffectStartTransmit`.
    pub fn play_start_transmit(&mut self, radio: usize, encrypted: bool, modulation: Modulation) {
        if let Some(m) = self.master.radios.get_mut(radio) {
            m.play_start_transmit(&self.effects, encrypted, modulation);
        }
    }

    /// `AudioManager.PlaySoundEffectEndTransmit`.
    pub fn play_end_transmit(&mut self, radio: usize, modulation: Modulation) {
        if let Some(m) = self.master.radios.get_mut(radio) {
            m.play_end_transmit(&self.effects, modulation);
        }
    }

    /// `AudioManager.AddClientAudio`.
    pub fn add_client_audio(&mut self, audio: &ClientAudio) {
        let incoming = &self.incoming;
        let entry = match self.clients.entry(audio.original_client_guid) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => {
                match RemoteClientAudio::new(self.cache.clone(), self.factory.clone()) {
                    Ok(mut rc) => {
                        rc.set_settings(incoming);
                        v.insert(rc)
                    }
                    Err(e) => {
                        log::error!("Cannot create decoder for client: {e}");
                        return;
                    }
                }
            }
        };
        entry.set_settings(incoming);
        entry.add_samples(audio);
    }

    /// `RemoveClientBuffer`.
    pub fn remove_client(&mut self, guid: &ShortGuid) {
        self.clients.remove(guid);
    }

    pub fn clear(&mut self) {
        self.clients.clear();
    }

    /// Render interleaved stereo 48 kHz.
    pub fn render(&mut self, out: &mut [f32]) {
        self.master.read(&self.effects, &mut self.clients, out);
    }
}

pub type SharedEngine = Arc<Mutex<AudioEngine>>;

pub fn shared_engine() -> SharedEngine {
    Arc::new(Mutex::new(AudioEngine::new()))
}
