//! egui user interface: main window (`MainWindow`), settings, controls,
//! favourites, client list, server settings, and the two overlays.

mod awacs;
mod clients;
mod controls;
mod favourites;
mod overlay;
mod radio_widgets;
mod server_settings;
mod settings;

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use eframe::egui;
use srs_net::tcp::DisconnectReason;
use srs_net::{NetCommand, NetEvent, SessionConfig, VoipEvent};
use srs_settings::{FavouriteStore, GlobalSettings, GlobalSettingsKey, ProfileSettings, ProfileSettingsKey};

use srs_client::{AppEvent, Client, ClientState};

use crate::args::Args;
use radio_widgets::{RadioAction, RadioUiCtx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Radios,
    Controls,
    Settings,
    Favourites,
    Clients,
    Server,
    Log,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileDialogMode {
    Create,
    Copy,
    Rename,
}

struct ProfileDialog {
    mode: ProfileDialogMode,
    text: String,
}

pub struct App {
    args: Args,
    settings: GlobalSettings,
    profile: ProfileSettings,
    net: Client,
    tab: Tab,
    favourites: FavouriteStore,
    favourites_ui: favourites::FavouritesUi,
    sounds: srs_audio::UiSounds,
    input_list: Vec<srs_audio::devices::DeviceInfo>,
    output_list: Vec<srs_audio::devices::DeviceInfo>,
    // General tab
    server_address: String,
    eam_name: String,
    eam_password: String,
    eam_coalition: Option<i32>,
    tcp_connected: bool,
    voip_ready: bool,
    speaker_boost: f32,
    synced: bool,
    server_version: Option<String>,
    status_line: String,
    presets_folder: String,
    auto_connect_at: Option<Instant>,
    pending_switch: Option<(String, Instant)>,
    mismatch_prompt: Option<String>,
    // Overlays
    show_overlay: bool,
    overlay_geometry: overlay::OverlayGeometry,
    show_awacs: bool,
    awacs_geometry: awacs::AwacsGeometry,
    last_overlay_toggle: Option<Instant>,
    instructor_saved: HashMap<usize, srs_dcs::DcsRadio>,
    // Controls / profiles
    pending_assign: Option<srs_input::InputBinding>,
    input_devices: Vec<String>,
    profile_dialog: Option<ProfileDialog>,
    delete_confirm: bool,
    // Window
    main_pos: Option<(f32, f32)>,
    first_frame: bool,
    // Log
    log: VecDeque<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Args, mut settings: GlobalSettings) -> Self {
        let server_address = settings.get_string(GlobalSettingsKey::LastServer);
        let last_seen_name = settings.get_string(GlobalSettingsKey::LastSeenName);
        let state = ClientState::new(last_seen_name.clone());
        log::info!("Client GUID {}", state.guid);
        let ctx = cc.egui_ctx.clone();
        let mut net = Client::new(std::sync::Arc::new(move || ctx.request_repaint()), state);
        // `AudioEffects/` next to the exe, `RadioModels[Custom]/` in the working directory (as in C#).
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let cache = std::sync::Arc::new(srs_audio::EffectCache::load(&exe_dir));
        let factory = std::sync::Arc::new(srs_audio::RadioModelFactory::load(std::path::Path::new(".")));
        net.engine.lock().set_assets(cache, factory);
        let sounds = srs_audio::UiSounds::load(&exe_dir);
        let mut profile = ProfileSettings::load(settings.dir(), "default").unwrap_or_else(|e| {
            log::error!("Cannot load profile: {e}");
            ProfileSettings::in_memory("default")
        });
        apply_settings_to_client(&mut net, &mut settings, &mut profile);
        let speaker_boost = settings.get_f64(GlobalSettingsKey::SpeakerBoost) as f32;
        net.prefs.show_transmitter_name.store(
            settings.get_bool(GlobalSettingsKey::ShowTransmitterName),
            std::sync::atomic::Ordering::Relaxed,
        );
        net.engine.lock().master.volume = srs_audio::volume::slider_to_scale(speaker_boost);
        let favourites = FavouriteStore::load(FavouriteStore::path_in(std::path::Path::new(".")));
        let input_list = srs_audio::devices::input_devices();
        let output_list = srs_audio::devices::output_devices();
        let overlay_geometry = overlay::OverlayGeometry {
            x: settings.get_f64(GlobalSettingsKey::RadioX) as f32,
            y: settings.get_f64(GlobalSettingsKey::RadioY) as f32,
            w: settings.get_f64(GlobalSettingsKey::RadioWidth) as f32,
            h: settings.get_f64(GlobalSettingsKey::RadioHeight) as f32,
            opacity: settings.get_f64(GlobalSettingsKey::RadioOpacity) as f32,
        };
        let awacs_geometry = awacs::AwacsGeometry {
            x: settings.get_f64(GlobalSettingsKey::AwacsX) as f32,
            y: settings.get_f64(GlobalSettingsKey::AwacsY) as f32,
            opacity: 1.0,
        };
        apply_theme(&cc.egui_ctx, settings.get_bool(GlobalSettingsKey::DarkMode));
        let presets_folder = settings.get_raw(GlobalSettingsKey::LastPresetsFolder);
        let mut app = Self {
            args,
            settings,
            profile,
            net,
            tab: Tab::General,
            favourites,
            favourites_ui: Default::default(),
            sounds,
            input_list,
            output_list,
            server_address,
            eam_name: last_seen_name,
            eam_password: String::new(),
            eam_coalition: None,
            tcp_connected: false,
            voip_ready: false,
            speaker_boost,
            synced: false,
            server_version: None,
            status_line: "Disconnected".into(),
            presets_folder,
            auto_connect_at: None,
            pending_switch: None,
            mismatch_prompt: None,
            show_overlay: false,
            overlay_geometry,
            show_awacs: false,
            awacs_geometry,
            last_overlay_toggle: None,
            instructor_saved: HashMap::new(),
            pending_assign: None,
            input_devices: Vec::new(),
            profile_dialog: None,
            delete_confirm: false,
            main_pos: None,
            first_frame: true,
            log: VecDeque::new(),
        };
        app.push_log(format!(
            "Audio devices: {} input(s), {} output(s)",
            app.input_list.len(),
            app.output_list.len()
        ));
        app.start_input(cc);
        if let Some(host) = app.args.host.clone() {
            // `-host=` from the DCS launcher: the C# client connects after 2 s.
            app.server_address = host;
            app.push_log("Auto-connect requested by -host=, connecting in 2 s");
            app.auto_connect_at = Some(Instant::now() + Duration::from_secs(2));
        }
        app
    }

    /// Start the input poller with the current profile's bindings.
    fn start_input(&mut self, cc: &eframe::CreationContext<'_>) {
        let hwnd = window_hwnd(cc);
        let allow_xinput = self.settings.get_bool(GlobalSettingsKey::AllowXInputController);
        let expand = self.settings.get_bool(GlobalSettingsKey::ExpandControls);
        let backend = srs_input::backend::default_backend(allow_xinput, expand, hwnd);
        self.input_devices = backend
            .devices()
            .into_iter()
            .map(|d| format!("{} ({})", d.name, d.instance_guid))
            .collect();
        self.push_ptt_settings();
        let bindings = self.profile.bindings();
        self.push_log(format!(
            "Input: {} device(s), {} binding(s) in profile '{}'",
            self.input_devices.len(),
            bindings.len(),
            self.profile.name()
        ));
        self.net.start_input(backend, bindings);
    }

    fn push_ptt_settings(&mut self) {
        *self.net.ptt.settings.lock() = srs_client::PttSettings {
            radio_switch_is_ptt: self.profile.get_bool(ProfileSettingsKey::RadioSwitchIsPTT),
            radio_switch_is_ptt_only_when_valid: self
                .profile
                .get_bool(ProfileSettingsKey::RadioSwitchIsPTTOnlyWhenValid),
            ptt_start_delay: self.profile.get_f32(ProfileSettingsKey::PTTStartDelay),
            ptt_release_delay: self.profile.get_f32(ProfileSettingsKey::PTTReleaseDelay),
            rotary_style_increment: self.profile.get_bool(ProfileSettingsKey::RotaryStyleIncrement),
        };
    }

    /// Re-push every setting the running client caches (after a change in
    /// the Settings tab or a profile switch).
    fn apply_all_settings(&mut self) {
        apply_settings_to_client(&mut self.net, &mut self.settings, &mut self.profile);
        let mic = self.net.mic_settings.clone();
        self.net.set_mic_settings(mic);
        self.push_ptt_settings();
        self.net.prefs.show_transmitter_name.store(
            self.settings.get_bool(GlobalSettingsKey::ShowTransmitterName),
            std::sync::atomic::Ordering::Relaxed,
        );
        if let Some(p) = self.net.input_poller() {
            p.set_bindings(self.profile.bindings());
        }
        self.net.engine.lock().master.volume = srs_audio::volume::slider_to_scale(self.speaker_boost);
    }

    /// `SelectedProfile` setter: load the profile and reload everything.
    fn switch_profile(&mut self, name: &str) {
        if self.profile.name() == name {
            return;
        }
        match ProfileSettings::load(self.settings.dir(), name) {
            Ok(p) => {
                self.profile = p;
                self.push_log(format!("Profile '{name}' selected"));
                self.apply_all_settings();
                self.reload_all_channels();
            }
            Err(e) => self.push_log(format!("Cannot load profile '{name}': {e}")),
        }
    }

    /// `PresetChannelsViewModel.Reload` for one radio.
    fn reload_channels(&mut self, radio: usize) {
        let (name, modulation, min, max) = {
            let rs = self.net.radio_state.lock();
            let Some(r) = rs.dcs.radios.get(radio) else { return };
            (r.name.clone(), r.modulation, r.freq_min, r.freq_max)
        };
        let (folder, source) = {
            let m = self.net.merge_ctx.lock();
            (m.presets_folder.clone(), m.preset_source)
        };
        let client =
            srs_dcs::presets::load_from_store(&folder, &name, modulation == srs_protocol::Modulation::Mids);
        let server = self
            .net
            .shared
            .server_settings
            .read()
            .map(|s| s.preset_channels(&srs_dcs::presets::normalise(&name)).to_vec())
            .unwrap_or_default();
        let list = srs_dcs::presets::build_channel_list(modulation, min, max, client, &server, source);
        if let Some(slot) = self.net.fixed_channels.lock().get_mut(radio) {
            *slot = list;
        }
    }

    fn reload_all_channels(&mut self) {
        let n = self.net.radio_state.lock().dcs.radios.len();
        for i in 1..n {
            let overlay = self.net.radio_state.lock().dcs.radios[i].freq_mode == srs_dcs::FreqMode::Overlay;
            if overlay {
                self.reload_channels(i);
            }
        }
    }

    fn handle_radio_actions(&mut self, actions: Vec<RadioAction>) {
        for a in actions {
            match a {
                RadioAction::ReloadChannels(i) => self.reload_channels(i),
                RadioAction::StartInstructor { radio, unit_id } => {
                    let mut rs = self.net.radio_state.lock();
                    if let Some(r) = rs.dcs.radios.get_mut(radio) {
                        self.instructor_saved.entry(radio).or_insert_with(|| r.clone());
                        *r = radio_widgets::instructor_radio(unit_id);
                        rs.force_sync = true;
                    }
                }
                RadioAction::StopInstructor(radio) => {
                    if let Some(saved) = self.instructor_saved.remove(&radio) {
                        let mut rs = self.net.radio_state.lock();
                        if let Some(r) = rs.dcs.radios.get_mut(radio) {
                            *r = saved;
                            rs.force_sync = true;
                        }
                    }
                }
            }
        }
    }

    /// `StopInstructorMode` for every radio (EAM / TCP disconnect).
    fn stop_all_instructor(&mut self) {
        let saved: Vec<usize> = self.instructor_saved.keys().copied().collect();
        for radio in saved {
            self.handle_radio_actions(vec![RadioAction::StopInstructor(radio)]);
        }
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        self.profile_row(ui);
        ui.separator();
        let names = |list: &[srs_audio::effects::CachedEffect]| -> Vec<String> {
            list.iter()
                .filter(|e| e.loaded())
                .map(|e| e.file_name.clone())
                .collect()
        };
        let (rs, re, is, ie) = {
            let engine = self.net.engine.lock();
            let c = &engine.cache;
            (
                names(&c.radio_start),
                names(&c.radio_end),
                names(&c.intercom_start),
                names(&c.intercom_end),
            )
        };
        let lists = settings::SettingsLists {
            radio_start: &rs,
            radio_end: &re,
            intercom_start: &is,
            intercom_end: &ie,
        };
        let change = settings::settings_tab(ui, &mut self.settings, &mut self.profile, &lists);
        if change.reset_overlay {
            self.close_overlay();
            self.overlay_geometry = overlay::OverlayGeometry::default();
            self.save_overlay_geometry();
            self.push_log("Radio overlay reset");
        }
        if change.changed {
            apply_theme(ui.ctx(), self.settings.get_bool(GlobalSettingsKey::DarkMode));
            self.apply_all_settings();
        }
    }

    /// Profile combo + Create / Copy / Rename / Delete.
    fn profile_row(&mut self, ui: &mut egui::Ui) {
        let profiles = self.settings.profiles();
        let current = self.profile.name().to_string();
        let mut selected = current.clone();
        ui.horizontal(|ui| {
            ui.label("Profile:");
            egui::ComboBox::from_id_salt("profile")
                .selected_text(&current)
                .show_ui(ui, |ui| {
                    for p in &profiles {
                        ui.selectable_value(&mut selected, p.clone(), p);
                    }
                });
            if ui.button("Create").clicked() {
                self.profile_dialog = Some(ProfileDialog {
                    mode: ProfileDialogMode::Create,
                    text: String::new(),
                });
            }
            if ui.button("Copy").clicked() {
                self.profile_dialog = Some(ProfileDialog {
                    mode: ProfileDialogMode::Copy,
                    text: String::new(),
                });
            }
            if ui.button("Rename").clicked() {
                if current == "default" {
                    self.push_log("Cannot rename the default profile");
                } else {
                    self.profile_dialog = Some(ProfileDialog {
                        mode: ProfileDialogMode::Rename,
                        text: current.clone(),
                    });
                }
            }
            if ui.button("Delete").clicked() {
                if current == "default" {
                    self.push_log("Cannot delete the default profile");
                } else {
                    self.delete_confirm = true;
                }
            }
        });
        if selected != current {
            self.switch_profile(&selected);
        }
        // Modal: new name.
        let mut close = false;
        let mut submit: Option<(ProfileDialogMode, String)> = None;
        if let Some(d) = &mut self.profile_dialog {
            let title = match d.mode {
                ProfileDialogMode::Create => "Create profile",
                ProfileDialogMode::Copy => "Copy profile",
                ProfileDialogMode::Rename => "Rename profile",
            };
            let modal = egui::Modal::new(egui::Id::new("profile-dialog")).show(ui.ctx(), |ui| {
                ui.heading(title);
                ui.text_edit_singleline(&mut d.text);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        submit = Some((d.mode, d.text.clone()));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if modal.should_close() {
                close = true;
            }
        }
        if close {
            self.profile_dialog = None;
        }
        if let Some((mode, text)) = submit {
            let name = srs_settings::profiles::clean_profile_name(&text);
            if !name.trim().is_empty() {
                match mode {
                    ProfileDialogMode::Create => {
                        srs_settings::profiles::add_profile(&mut self.settings, &name);
                        self.push_log(format!("Profile '{name}' created"));
                    }
                    ProfileDialogMode::Copy => {
                        let _ = self.profile.save();
                        match srs_settings::profiles::copy_profile(&mut self.settings, &current, &name) {
                            Ok(()) => self.push_log(format!("Profile '{current}' copied to '{name}'")),
                            Err(e) => self.push_log(format!("Copy failed: {e}")),
                        }
                        self.switch_profile("default");
                    }
                    ProfileDialogMode::Rename => {
                        let _ = self.profile.save();
                        match srs_settings::profiles::rename_profile(&mut self.settings, &current, &name) {
                            Ok(()) => self.push_log(format!("Profile '{current}' renamed to '{name}'")),
                            Err(e) => self.push_log(format!("Rename failed: {e}")),
                        }
                        self.switch_profile("default");
                    }
                }
            }
        }
        if self.delete_confirm {
            let mut decided: Option<bool> = None;
            let modal = egui::Modal::new(egui::Id::new("profile-delete")).show(ui.ctx(), |ui| {
                ui.label(format!("Are you sure you want to delete {current}?"));
                ui.horizontal(|ui| {
                    if ui.button("Yes").clicked() {
                        decided = Some(true);
                    }
                    if ui.button("No").clicked() {
                        decided = Some(false);
                    }
                });
            });
            if modal.should_close() {
                decided = Some(false);
            }
            if let Some(yes) = decided {
                self.delete_confirm = false;
                if yes {
                    srs_settings::profiles::remove_profile(&mut self.settings, &current);
                    self.push_log(format!("Profile '{current}' deleted"));
                    self.switch_profile("default");
                }
            }
        }
    }

    fn controls_tab(&mut self, ui: &mut egui::Ui) {
        let bindings = self.profile.bindings();
        match controls::controls_panel(ui, &bindings, self.pending_assign, &self.input_devices) {
            Some(controls::ControlsAction::Assign(b)) => {
                self.pending_assign = Some(b);
                self.net.assign_button();
            }
            Some(controls::ControlsAction::Clear(b)) => {
                self.profile.remove_binding(b);
                if let Some(p) = self.net.input_poller() {
                    p.set_bindings(self.profile.bindings());
                }
            }
            Some(controls::ControlsAction::Rescan) => {
                if let Some(p) = self.net.input_poller() {
                    p.rescan();
                }
            }
            None => {}
        }
    }

    /// Server settings that drive audio (`IRL_RADIO_RX_INTERFERENCE`).
    fn apply_server_settings(&mut self) {
        let irl_rx = self
            .net
            .shared
            .server_settings
            .read()
            .map(|s| s.get_bool(srs_protocol::server_settings::ServerSettingsKey::IRL_RADIO_RX_INTERFERENCE))
            .unwrap_or(false);
        self.net.engine.lock().effects.irl_radio_rx_interference = irl_rx;
    }

    fn push_log(&mut self, line: impl Into<String>) {
        let line = line.into();
        log::info!("{line}");
        self.log.push_back(line);
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }

    fn session_config(&mut self) -> SessionConfig {
        let idle = self.settings.get_i32(GlobalSettingsKey::IdleTimeOut).max(1) as u64;
        SessionConfig {
            client_version: crate::CLIENT_VERSION.to_string(),
            idle_timeout: Duration::from_secs(idle),
            ..Default::default()
        }
    }

    fn connect(&mut self) {
        let address = self.server_address.trim().to_string();
        if address.is_empty() {
            self.status_line = "Enter a server address".into();
            return;
        }
        self.net.state.lock().allow_record = self.settings.get_bool(GlobalSettingsKey::AllowRecording);
        self.net.state.lock().dis_entity_id = self.settings.get_i32(GlobalSettingsKey::DISEntityID);
        self.net.audio_input_device = Some(self.settings.get_raw(GlobalSettingsKey::AudioInputDeviceId));
        self.net.audio_output_device = Some(self.settings.get_raw(GlobalSettingsKey::AudioOutputDeviceId));
        let initial = self.net.state.lock().initial_client();
        let config = self.session_config();
        match self.net.connect(&address, initial, config) {
            Ok(()) => {
                self.settings.set_string(GlobalSettingsKey::LastServer, &address);
                self.status_line = format!("Connecting to {address}...");
                self.push_log(format!("Connecting to {address}"));
            }
            Err(e) => {
                self.status_line = format!("Invalid IP or Host Name! ({e})");
                self.push_log(format!("Connect failed: {e}"));
            }
        }
    }

    /// `Stop()`: disconnect beep (when it was connected), then disconnect.
    fn disconnect(&mut self) {
        if self.tcp_connected && self.settings.get_bool(GlobalSettingsKey::PlayConnectionSounds) {
            self.sounds.disconnected.play_detached();
        }
        self.net.disconnect();
        self.push_log("Disconnect requested");
    }

    /// Overlay hotkeys / buttons (single and AWACS are mutually exclusive).
    fn toggle_overlay(&mut self, awacs: bool) {
        if self
            .last_overlay_toggle
            .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
        {
            return;
        }
        self.last_overlay_toggle = Some(Instant::now());
        if awacs {
            self.show_awacs = !self.show_awacs;
            if self.show_awacs {
                self.close_overlay();
            }
        } else {
            self.show_overlay = !self.show_overlay;
            if self.show_overlay {
                self.show_awacs = false;
            } else {
                self.save_overlay_geometry();
            }
        }
    }

    fn close_overlay(&mut self) {
        if self.show_overlay {
            self.show_overlay = false;
            self.save_overlay_geometry();
        }
    }

    fn save_overlay_geometry(&mut self) {
        let g = self.overlay_geometry;
        self.settings.set_f64(GlobalSettingsKey::RadioX, f64::from(g.x));
        self.settings.set_f64(GlobalSettingsKey::RadioY, f64::from(g.y));
        self.settings
            .set_f64(GlobalSettingsKey::RadioWidth, f64::from(g.w));
        self.settings
            .set_f64(GlobalSettingsKey::RadioHeight, f64::from(g.h));
        self.settings
            .set_f64(GlobalSettingsKey::RadioOpacity, f64::from(g.opacity));
    }

    fn save_awacs_geometry(&mut self) {
        let g = self.awacs_geometry;
        self.settings.set_f64(GlobalSettingsKey::AwacsX, f64::from(g.x));
        self.settings.set_f64(GlobalSettingsKey::AwacsY, f64::from(g.y));
    }

    fn handle_events(&mut self) {
        for evt in self.net.poll() {
            let evt = match evt {
                AppEvent::Net(e) => e,
                AppEvent::AutoConnect(addr) => {
                    // C#: `AutoConnect` / `AutoConnectPrompt` are never read; only the mismatch prompt exists.
                    if self.net.is_connected() {
                        if self.server_address.trim() != addr {
                            if self
                                .settings
                                .get_bool(GlobalSettingsKey::AutoConnectMismatchPrompt)
                            {
                                self.mismatch_prompt = Some(addr);
                            } else {
                                self.switch_server(addr);
                            }
                        }
                    } else {
                        self.server_address = addr.clone();
                        self.push_log(format!("DCS auto-connect: connecting to {addr}"));
                        self.connect();
                    }
                    continue;
                }
                AppEvent::LastSeenNameChanged(name) => {
                    self.settings.set_string(GlobalSettingsKey::LastSeenName, &name);
                    self.eam_name = name;
                    continue;
                }
                AppEvent::NewUnitEntered { unit, seat } => {
                    if self
                        .settings
                        .get_bool(GlobalSettingsKey::AutoSelectSettingsProfile)
                    {
                        let profiles = self.settings.profiles();
                        let chosen = srs_settings::profiles::auto_select_profile(&profiles, &unit, seat);
                        self.push_log(format!("New unit {unit} seat {seat}: profile '{chosen}'"));
                        self.switch_profile(&chosen);
                    }
                    continue;
                }
                AppEvent::ReloadFixedChannels { radio, min, max } => {
                    let count = self.net.fixed_channels.lock().get(radio).map_or(0, Vec::len);
                    self.push_log(format!(
                        "Radio {radio}: {count} preset channels ({min:.0}-{max:.0} Hz)"
                    ));
                    continue;
                }
                AppEvent::ToggleOverlay => {
                    self.toggle_overlay(false);
                    continue;
                }
                AppEvent::ToggleAwacsOverlay => {
                    self.toggle_overlay(true);
                    continue;
                }
                AppEvent::ButtonAssigned(mut d) => {
                    if let Some(b) = self.pending_assign.take() {
                        d.bind = b;
                        self.push_log(format!("Bound {} to {}", b.as_str(), controls::describe(&d)));
                        self.profile.set_binding(&d);
                        if let Some(p) = self.net.input_poller() {
                            p.set_bindings(self.profile.bindings());
                        }
                    }
                    continue;
                }
                AppEvent::Voip(v) => {
                    match v {
                        VoipEvent::Ready(r) => {
                            self.voip_ready = r;
                            self.push_log(if r {
                                "VoIP ready (UDP ping answered)"
                            } else {
                                "VoIP not ready"
                            });
                        }
                        VoipEvent::Timeout => self.push_log("VoIP timeout: recreating UDP socket"),
                        VoipEvent::Stopped => self.voip_ready = false,
                    }
                    continue;
                }
            };
            match evt {
                NetEvent::Connected(addr) => {
                    self.tcp_connected = true;
                    self.status_line = format!("Connected to {addr}, waiting for SYNC");
                    self.push_log(format!("TCP connected to {addr}"));
                    if self.settings.get_bool(GlobalSettingsKey::PlayConnectionSounds) {
                        self.sounds.connected.play_detached();
                    }
                    let guid = self.net.state.lock().guid;
                    self.net.start_audio(guid);
                    self.net.start_dcs();
                    self.push_log("Audio started (UDP voice, decoder, devices); DCS listeners started");
                }
                NetEvent::ConnectFailed(e) => {
                    self.tcp_connected = false;
                    self.status_line = format!("Connection failed: {e}");
                    self.push_log(format!("Connection failed: {e}"));
                    // C# `Stop()` after the optimistic IsConnected=true also beeps.
                    if self.settings.get_bool(GlobalSettingsKey::PlayConnectionSounds) {
                        self.sounds.disconnected.play_detached();
                    }
                }
                NetEvent::Synced {
                    server_version,
                    client_count,
                } => {
                    self.synced = true;
                    self.server_version = Some(server_version.clone());
                    self.status_line = format!("Connected (server {server_version}, {client_count} clients)");
                    self.push_log(format!(
                        "SYNC ok: server {server_version}, {client_count} clients"
                    ));
                    self.apply_server_settings();
                }
                NetEvent::ClientUpdated { guid, full } => {
                    if full && guid == self.net.state.lock().guid.as_str().as_ref() {
                        self.push_log("Server echoed our RADIO_UPDATE (metadata valid)");
                    }
                }
                NetEvent::ClientRemoved(guid) => self.push_log(format!("Client left: {guid}")),
                NetEvent::ServerSettingsUpdated => {
                    self.apply_server_settings();
                    let allow_instructor = self
                        .net
                        .shared
                        .server_settings
                        .read()
                        .map(|s| s.get_bool(srs_protocol::ServerSettingsKey::ALLOW_INSTRUCTOR_MODE))
                        .unwrap_or(false);
                    if !allow_instructor {
                        self.stop_all_instructor();
                    }
                    self.push_log("Server settings updated");
                }
                NetEvent::EamResult { coalition } => {
                    if coalition == 0 {
                        self.eam_coalition = None;
                        self.push_log("External AWACS mode: authentication failed");
                    } else {
                        self.eam_coalition = Some(coalition);
                        {
                            let mut st = self.net.state.lock();
                            st.coalition = coalition;
                            st.last_seen_name = self.eam_name.clone();
                        }
                        self.settings
                            .set_string(GlobalSettingsKey::LastSeenName, &self.eam_name);
                        self.push_log(format!("External AWACS mode: connected as coalition {coalition}"));
                        let allow_server_preset = self
                            .profile
                            .get_bool(ProfileSettingsKey::AllowServerEAMRadioPreset);
                        let preset = self
                            .net
                            .shared
                            .server_settings
                            .read()
                            .map(|s| s.custom_eam_radios.clone())
                            .unwrap_or_default();
                        let folder = std::path::PathBuf::from(
                            self.settings.get_raw(GlobalSettingsKey::LastPresetsFolder),
                        );
                        let folder = if folder.as_os_str().is_empty() {
                            std::env::current_dir().unwrap_or_default()
                        } else {
                            folder
                        };
                        let radios = srs_client::eam::resolve_radios(allow_server_preset, &preset, &folder);
                        self.push_log(format!("EAM radios: {}", srs_client::eam::describe(&radios)));
                        self.net.start_eam(coalition, radios);
                        self.reload_all_channels();
                        self.tab = Tab::Radios;
                    }
                }
                NetEvent::EamDisconnected => {
                    self.stop_all_instructor();
                    if self.eam_coalition.take().is_some() {
                        self.push_log("External AWACS mode disabled by server");
                    }
                }
                NetEvent::Disconnected(reason) => {
                    let was_connected = self.tcp_connected;
                    self.stop_all_instructor();
                    self.tcp_connected = false;
                    self.voip_ready = false;
                    self.synced = false;
                    self.eam_coalition = None;
                    self.server_version = None;
                    self.status_line = match &reason {
                        DisconnectReason::Requested => "Disconnected".into(),
                        DisconnectReason::IdleTimeout => "Disconnected: idle timeout".into(),
                        DisconnectReason::ConnectionLost(e) => format!("Connection lost: {e}"),
                        DisconnectReason::VersionMismatch { server_version } => format!(
                            "SRS server incompatible (server {server_version}, client {})",
                            crate::CLIENT_VERSION
                        ),
                        DisconnectReason::UnversionedServer => "Disconnected: unversioned server".into(),
                    };
                    let status = self.status_line.clone();
                    self.push_log(status);
                    if was_connected
                        && !matches!(reason, DisconnectReason::Requested)
                        && self.settings.get_bool(GlobalSettingsKey::PlayConnectionSounds)
                    {
                        self.sounds.disconnected.play_detached();
                    }
                }
            }
        }
    }

    /// Auto-connect server switch: `Stop(); delay 2 s; ConnectAsync()`.
    fn switch_server(&mut self, addr: String) {
        self.push_log(format!("Switching to {addr} (DCS auto-connect)"));
        self.disconnect();
        self.server_address = addr.clone();
        self.pending_switch = Some((addr, Instant::now() + Duration::from_secs(2)));
    }

    fn device_combo(
        ui: &mut egui::Ui,
        settings: &mut GlobalSettings,
        key: GlobalSettingsKey,
        label: &str,
        default_label: &str,
        list: &[srs_audio::devices::DeviceInfo],
        enabled: bool,
    ) {
        let stored = settings.get_raw(key);
        let stored = stored.trim().to_string();
        let current = list
            .iter()
            .find(|d| !d.id.is_empty() && d.id == stored)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| default_label.to_string());
        let mut pick: Option<String> = None;
        ui.add_enabled_ui(enabled, |ui| {
            egui::ComboBox::from_label(label)
                .selected_text(&current)
                .width(240.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current == default_label, default_label)
                        .clicked()
                    {
                        pick = Some("default".to_string());
                    }
                    for d in list {
                        if ui.selectable_label(d.id == stored, &d.name).clicked() {
                            pick = Some(d.id.clone());
                        }
                    }
                });
        });
        if let Some(id) = pick {
            settings.set_raw(key, &id);
        }
    }

    fn general_tab(&mut self, ui: &mut egui::Ui) {
        let connected = self.net.is_connected();
        // Audio devices (disabled while connected, like the C# client).
        let mic_available = !self.input_list.is_empty();
        Self::device_combo(
            ui,
            &mut self.settings,
            GlobalSettingsKey::AudioInputDeviceId,
            "Microphone",
            "Default Microphone",
            &self.input_list,
            !connected && mic_available,
        );
        if !mic_available {
            ui.colored_label(egui::Color32::YELLOW, "No microphone found");
        }
        Self::device_combo(
            ui,
            &mut self.settings,
            GlobalSettingsKey::AudioOutputDeviceId,
            "Speakers",
            "Default Speakers",
            &self.output_list,
            !connected,
        );
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Server:");
            ui.add_enabled(
                !connected,
                egui::TextEdit::singleline(&mut self.server_address).desired_width(200.0),
            );
            let mut picked: Option<usize> = None;
            ui.add_enabled_ui(!connected, |ui| {
                egui::ComboBox::from_id_salt("fav-pick")
                    .selected_text("Favourites")
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for (i, f) in self.favourites.servers.iter().enumerate() {
                            if ui
                                .selectable_label(false, format!("{} ({})", f.name, f.address))
                                .clicked()
                            {
                                picked = Some(i);
                            }
                        }
                    });
            });
            if let Some(i) = picked {
                self.use_favourite(i);
            }
            if connected {
                if ui.button("Disconnect").clicked() {
                    self.disconnect();
                }
            } else if ui.button("Connect").clicked() {
                self.connect();
            }
        });
        ui.horizontal(|ui| {
            status_dot(ui, self.tcp_connected && self.synced, "Server");
            status_dot(ui, self.voip_ready, "VoIP");
            status_dot(ui, self.net.radio_state.lock().is_game_connected(), "DCS");
            ui.label(&self.status_line);
        });
        ui.horizontal(|ui| {
            let mut single = self.show_overlay;
            if ui.toggle_value(&mut single, "Radio overlay").clicked() {
                self.last_overlay_toggle = None;
                self.toggle_overlay(false);
            }
            let mut awacs = self.show_awacs;
            if ui.toggle_value(&mut awacs, "AWACS overlay").clicked() {
                self.last_overlay_toggle = None;
                self.toggle_overlay(true);
            }
            if ui
                .add_enabled(connected, egui::Button::new("Server settings"))
                .clicked()
            {
                self.tab = Tab::Server;
            }
            if ui
                .add_enabled(connected, egui::Button::new("Client list"))
                .clicked()
            {
                self.tab = Tab::Clients;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Presets folder:");
            let eam = self.eam_coalition.is_some();
            let resp = ui.add_enabled(
                !eam,
                egui::TextEdit::singleline(&mut self.presets_folder)
                    .desired_width(220.0)
                    .hint_text("(default: working directory)"),
            );
            if resp.lost_focus() {
                self.settings
                    .set_raw(GlobalSettingsKey::LastPresetsFolder, self.presets_folder.trim());
                self.apply_all_settings();
                self.reload_all_channels();
            }
            if ui.add_enabled(!eam, egui::Button::new("Reset")).clicked() {
                self.presets_folder.clear();
                self.settings.set_raw(GlobalSettingsKey::LastPresetsFolder, "");
                self.apply_all_settings();
                self.reload_all_channels();
            }
        });
        ui.separator();
        self.eam_block(ui);
        ui.separator();
        if ui
            .add(egui::Slider::new(&mut self.speaker_boost, 0.0..=1.0).text("Speaker boost"))
            .changed()
        {
            self.net.engine.lock().master.volume = srs_audio::volume::slider_to_scale(self.speaker_boost);
            self.settings
                .set_f64(GlobalSettingsKey::SpeakerBoost, f64::from(self.speaker_boost));
        }
        let scale = srs_audio::volume::slider_to_scale(self.speaker_boost);
        ui.small(format!("{:.0} dB", 20.0 * f64::from(scale).log10()));
        let peak = self.net.engine.lock().master.peak;
        ui.add(
            egui::ProgressBar::new(peak)
                .text(format!("speaker {:.0} dB", srs_audio::volume::float_to_db(peak))),
        );
        ui.horizontal(|ui| {
            let held = ui
                .add(
                    egui::Button::new(egui::RichText::new("PTT (hold)").strong())
                        .min_size(egui::vec2(110.0, 32.0)),
                )
                .is_pointer_button_down_on();
            let space =
                ui.input(|i| i.key_down(egui::Key::Space)) && ui.ctx().memory(|m| m.focused().is_none());
            self.net
                .tx
                .ui_ptt
                .store(held || space, std::sync::atomic::Ordering::Relaxed);
            let sending = self.net.radio_state.lock().sending.clone();
            if sending.is_sending {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    format!("TX on radio {:?}", sending.sending_on),
                );
            } else {
                ui.label("idle (hold the button or Space to talk)");
            }
        });
        let mic_db = self.net.tx.mic_level_db();
        let mic_ok = self
            .net
            .tx
            .mic_available
            .load(std::sync::atomic::Ordering::Relaxed);
        ui.add(
            egui::ProgressBar::new(((mic_db + 60.0) / 60.0).clamp(0.0, 1.0)).text(if mic_ok {
                format!("mic {mic_db:.0} dB")
            } else {
                "no microphone".to_string()
            }),
        );
        ui.separator();
        // Footer (profile, unit, clients, position).
        let (unit, pos) = {
            let rs = self.net.radio_state.lock();
            let unit = if rs.dcs.is_current() {
                rs.dcs.unit.clone()
            } else {
                "-".into()
            };
            let p = rs.side.position;
            (unit, format!("{:.5}, {:.5} alt {:.0}", p.lat, p.lng, p.alt))
        };
        ui.small(format!(
            "Profile: {}   Unit: {}   Clients: {}   Position: {}",
            self.profile.name(),
            unit,
            self.net.shared.client_count(),
            pos
        ));
    }

    fn use_favourite(&mut self, i: usize) {
        if let Some(f) = self.favourites.servers.get(i) {
            self.server_address = f.address.clone();
            self.eam_password = f.eam_password.clone().unwrap_or_default();
            self.settings
                .set_string(GlobalSettingsKey::LastServer, &f.address);
        }
    }

    fn eam_block(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("External AWACS Mode", |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.add_enabled(
                    self.eam_coalition.is_none(),
                    egui::TextEdit::singleline(&mut self.eam_name).desired_width(160.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Password:");
                ui.add_enabled(
                    self.eam_coalition.is_none(),
                    egui::TextEdit::singleline(&mut self.eam_password)
                        .password(true)
                        .desired_width(160.0),
                );
            });
            let eam_available = self.synced
                && self
                    .net
                    .shared
                    .server_settings
                    .read()
                    .map(|s| s.get_bool(srs_protocol::ServerSettingsKey::EXTERNAL_AWACS_MODE))
                    .unwrap_or(false);
            match self.eam_coalition {
                Some(c) => {
                    ui.label(format!("Connected as {}", coalition_name(c)));
                    if ui.button("Disconnect EAM").clicked() {
                        self.stop_all_instructor();
                        self.eam_coalition = None;
                        self.net.state.lock().coalition = 0;
                        self.net.stop_eam();
                        let client =
                            srs_client::build_client(&self.net.radio_state.lock(), &self.net.state.lock());
                        self.net.send(NetCommand::SendRadioUpdate(client));
                    }
                }
                None => {
                    if ui
                        .add_enabled(eam_available, egui::Button::new("Connect EAM"))
                        .clicked()
                    {
                        let name = self.eam_name.trim().to_string();
                        self.net.send(NetCommand::EamLogin {
                            name,
                            password: self.eam_password.clone(),
                        });
                    }
                    if !eam_available {
                        ui.small("(server has EAM disabled or not connected)");
                    }
                }
            }
        });
    }

    fn log_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            for line in &self.log {
                ui.monospace(line);
            }
        });
    }

    fn help_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("DCS-SRS Client (Rust) {}", crate::CLIENT_VERSION));
        ui.label("Drop-in replacement for SR-ClientRadio.exe: no .NET runtime needed.");
        ui.hyperlink_to(
            "SRS on GitHub",
            "https://github.com/ciribob/DCS-SimpleRadioStandalone",
        );
        ui.hyperlink_to("Support Ciribob (Patreon)", "https://www.patreon.com/ciribob");
        ui.separator();
        ui.label(format!("Config directory: {}", self.settings.dir().display()));
        ui.label(format!("Client GUID: {}", self.net.state.lock().guid));
        ui.label("Logs: clientlog.txt in the working directory (RUST_LOG=debug for more).");
        ui.separator();
        ui.label("Not implemented in this build: audio recording, LotATC, VAICOM, auto-update, minimise to tray, RefocusDCS, RequireAdmin, mic passthrough.");
    }

    fn radio_ui_ctx(&self) -> (bool, i32, bool, bool) {
        let (node_limit, allow_instructor) = self
            .net
            .shared
            .server_settings
            .read()
            .map(|s| {
                (
                    s.retransmit_node_limit,
                    s.get_bool(srs_protocol::ServerSettingsKey::ALLOW_INSTRUCTOR_MODE),
                )
            })
            .unwrap_or((0, false));
        let rotary = self.net.rotary_style.load(std::sync::atomic::Ordering::Relaxed);
        let instructor = allow_instructor && self.net.merge_ctx.lock().instructor_mode;
        (rotary, node_limit, instructor, self.net.is_connected())
    }

    /// `"{AircraftType}: {pilot names}"` per unitId (excluding self).
    fn instructor_units(&self, my_guid: &str) -> Vec<(u32, String)> {
        let mut by_unit: std::collections::BTreeMap<u32, (String, Vec<String>)> = Default::default();
        for c in self.net.shared.snapshot() {
            if c.client.client_guid == my_guid {
                continue;
            }
            let Some(ri) = &c.client.radio_info else { continue };
            if ri.unit_id == 0 {
                continue;
            }
            let e = by_unit
                .entry(ri.unit_id)
                .or_insert_with(|| (ri.unit.clone(), Vec::new()));
            e.1.push(c.client.name.clone());
        }
        by_unit
            .into_iter()
            .map(|(id, (unit, names))| (id, format!("{unit}: {}", names.join(" "))))
            .collect()
    }

    fn mismatch_modal(&mut self, ctx: &egui::Context) {
        let Some(addr) = self.mismatch_prompt.clone() else {
            return;
        };
        let mut decided: Option<bool> = None;
        let modal = egui::Modal::new(egui::Id::new("mismatch")).show(ctx, |ui| {
            ui.heading("Different server");
            ui.label(format!("DCS asks to connect to {addr}"));
            ui.label(format!("You are connected to {}", self.server_address));
            ui.label("Switch server?");
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    decided = Some(true);
                }
                if ui.button("No").clicked() {
                    decided = Some(false);
                }
            });
        });
        if modal.should_close() {
            decided = Some(false);
        }
        if let Some(yes) = decided {
            self.mismatch_prompt = None;
            if yes {
                self.switch_server(addr);
            }
        }
    }
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
}

fn status_dot(ui: &mut egui::Ui, on: bool, label: &str) {
    let color = if on {
        egui::Color32::from_rgb(40, 200, 60)
    } else {
        egui::Color32::from_rgb(200, 50, 50)
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
    ui.label(label);
}

fn coalition_name(c: i32) -> &'static str {
    match c {
        1 => "Red",
        2 => "Blue",
        _ => "Spectator",
    }
}

impl eframe::App for App {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        let c = visuals.window_fill();
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some((x, y)) = self.main_pos {
            self.settings.set_f64(GlobalSettingsKey::ClientX, f64::from(x));
            self.settings.set_f64(GlobalSettingsKey::ClientY, f64::from(y));
        }
        if self.show_overlay {
            self.save_overlay_geometry();
        }
        if self.show_awacs {
            self.save_awacs_geometry();
        }
        let _ = self.profile.save();
        self.net.disconnect();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            if self.settings.get_bool(GlobalSettingsKey::StartMinimised) {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }
        if let Some(r) = ui.ctx().input(|i| i.viewport().outer_rect) {
            self.main_pos = Some((r.min.x, r.min.y));
        }
        if self.auto_connect_at.is_some_and(|t| Instant::now() >= t) {
            self.auto_connect_at = None;
            self.connect();
        }
        if self
            .pending_switch
            .as_ref()
            .is_some_and(|(_, t)| Instant::now() >= *t)
        {
            let (addr, _) = self.pending_switch.take().unwrap();
            self.server_address = addr;
            self.connect();
        }
        self.handle_events();
        let my_guid = self.net.state.lock().guid.to_string();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(format!("DCS-SRS Client {}", crate::CLIENT_VERSION));
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.tab, Tab::General, "General");
                ui.selectable_value(&mut self.tab, Tab::Radios, "Radios");
                ui.selectable_value(&mut self.tab, Tab::Controls, "Controls");
                ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
                ui.selectable_value(&mut self.tab, Tab::Favourites, "Favourites");
                ui.selectable_value(&mut self.tab, Tab::Clients, "Clients");
                ui.selectable_value(&mut self.tab, Tab::Server, "Server");
                ui.selectable_value(&mut self.tab, Tab::Log, "Log");
                ui.selectable_value(&mut self.tab, Tab::Help, "Help");
            });
            ui.separator();
            match self.tab {
                Tab::General => self.general_tab(ui),
                Tab::Radios => {
                    let (rotary, node_limit, instructor, connected) = self.radio_ui_ctx();
                    let active: Vec<usize> = self.instructor_saved.keys().copied().collect();
                    let units = if instructor {
                        self.instructor_units(&my_guid)
                    } else {
                        Vec::new()
                    };
                    let actions = {
                        let ctx = awacs::AwacsCtx {
                            base: RadioUiCtx {
                                net: &self.net,
                                my_guid: &my_guid,
                                connected,
                                rotary_style: rotary,
                                node_limit,
                            },
                            eam_active: self.eam_coalition.is_some(),
                            instructor_allowed: instructor,
                            instructor_active: &active,
                            units: &units,
                        };
                        awacs::awacs_panel(ui, &ctx, false)
                    };
                    self.handle_radio_actions(actions);
                }
                Tab::Controls => self.controls_tab(ui),
                Tab::Settings => self.settings_tab(ui),
                Tab::Favourites => {
                    let last = self.settings.get_string(GlobalSettingsKey::LastServer);
                    if let Some(i) =
                        favourites::favourites_tab(ui, &mut self.favourites_ui, &mut self.favourites, &last)
                    {
                        self.use_favourite(i);
                        self.tab = Tab::General;
                    }
                }
                Tab::Clients => clients::clients_tab(ui, &self.net, &my_guid),
                Tab::Server => {
                    server_settings::server_settings_tab(ui, &self.net, self.server_version.as_deref())
                }
                Tab::Log => self.log_tab(ui),
                Tab::Help => self.help_tab(ui),
            }
        });
        self.mismatch_modal(ui.ctx());
        let taskbar_hide = self.settings.get_bool(GlobalSettingsKey::RadioOverlayTaskbarHide);
        if self.show_overlay {
            let (rotary, node_limit, _, connected) = self.radio_ui_ctx();
            let out = {
                let ctx = RadioUiCtx {
                    net: &self.net,
                    my_guid: &my_guid,
                    connected,
                    rotary_style: rotary,
                    node_limit,
                };
                overlay::radio_overlay(ui.ctx(), &ctx, self.overlay_geometry, taskbar_hide)
            };
            self.overlay_geometry = out.geometry;
            self.handle_radio_actions(out.actions);
            if out.close {
                self.close_overlay();
            }
        }
        if self.show_awacs {
            let (rotary, node_limit, instructor, connected) = self.radio_ui_ctx();
            let active: Vec<usize> = self.instructor_saved.keys().copied().collect();
            let units = if instructor {
                self.instructor_units(&my_guid)
            } else {
                Vec::new()
            };
            let out = {
                let ctx = awacs::AwacsCtx {
                    base: RadioUiCtx {
                        net: &self.net,
                        my_guid: &my_guid,
                        connected,
                        rotary_style: rotary,
                        node_limit,
                    },
                    eam_active: self.eam_coalition.is_some(),
                    instructor_allowed: instructor,
                    instructor_active: &active,
                    units: &units,
                };
                awacs::awacs_viewport(ui.ctx(), &ctx, self.awacs_geometry, taskbar_hide)
            };
            self.awacs_geometry = out.geometry;
            self.handle_radio_actions(out.actions);
            if out.close {
                self.show_awacs = false;
                self.save_awacs_geometry();
            }
        }
        // Keep polling network events and RX indicators even without user input.
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

/// Push the settings the engine needs (`MergeContext`, mic, ports) into the client.
fn apply_settings_to_client(net: &mut Client, settings: &mut GlobalSettings, profile: &mut ProfileSettings) {
    {
        let mut m = net.merge_ctx.lock();
        m.allow_radio_encryption = true; // refreshed from server settings after SYNC
        m.always_allow_hotas_controls = profile.get_bool(ProfileSettingsKey::AlwaysAllowHotasControls);
        m.allow_dcs_ptt = profile.get_bool(ProfileSettingsKey::AllowDCSPTT);
        m.always_allow_transponder_overlay =
            profile.get_bool(ProfileSettingsKey::AlwaysAllowTransponderOverlay);
        m.auto_select_preset_channel = profile.get_bool(ProfileSettingsKey::AutoSelectPresetChannel);
        m.auto_select_settings_profile = settings.get_bool(GlobalSettingsKey::AutoSelectSettingsProfile);
        m.expansion_allowed = !profile.get_bool(ProfileSettingsKey::DisableExpansionRadios);
        let folder = settings.get_raw(GlobalSettingsKey::LastPresetsFolder);
        m.presets_folder = if folder.trim().is_empty() {
            std::path::PathBuf::from(".")
        } else {
            std::path::PathBuf::from(folder)
        };
        m.preset_source = srs_dcs::presets::PresetSource::from_setting(
            &profile.get_string(ProfileSettingsKey::ServerPresetSelection),
        );
        m.instructor_mode = profile.get_bool(ProfileSettingsKey::InstructorMode);
    }
    net.rotary_style.store(
        profile.get_bool(ProfileSettingsKey::RotaryStyleIncrement),
        std::sync::atomic::Ordering::Relaxed,
    );
    net.tx.vox_minimum_time_ms.store(
        settings.get_i32(GlobalSettingsKey::VOXMinimumTime).max(0) as u32,
        std::sync::atomic::Ordering::Relaxed,
    );
    net.mic_settings = srs_audio::MicSettings {
        denoise: settings.get_bool(GlobalSettingsKey::Denoise),
        denoise_attenuation: settings.get_i32(GlobalSettingsKey::DenoiseAttenuation),
        vox: settings.get_bool(GlobalSettingsKey::VOX),
        vox_mode: settings.get_i32(GlobalSettingsKey::VOXMode).clamp(0, 3) as u8,
        vox_minimum_db: settings.get_f64(GlobalSettingsKey::VOXMinimumDB),
    };
    {
        let mut engine = net.engine.lock();
        engine.incoming = srs_audio::IncomingAudioSettings {
            agc: settings.get_bool(GlobalSettingsKey::IncomingAudioAGC),
            agc_target: settings.get_i32(GlobalSettingsKey::IncomingAudioAGCTarget),
            agc_decrement: settings.get_i32(GlobalSettingsKey::IncomingAudioAGCDecrement),
            agc_max_gain: settings.get_i32(GlobalSettingsKey::IncomingAudioAGCLevelMax),
            denoise: settings.get_bool(GlobalSettingsKey::IncomingAudioDenoise),
            denoise_attenuation: settings.get_i32(GlobalSettingsKey::IncomingAudioDenoiseAttenuation),
            ambient_cockpit_effect: profile.get_bool(ProfileSettingsKey::AmbientCockpitNoiseEffect),
            ambient_cockpit_effect_volume: srs_audio::volume::radio_volume_slider(
                profile.get_f32(ProfileSettingsKey::AmbientCockpitNoiseEffectVolume),
                true,
            ),
            ambient_cockpit_intercom_effect: profile
                .get_bool(ProfileSettingsKey::AmbientCockpitIntercomNoiseEffect),
        };
        let irl_rx = engine.effects.irl_radio_rx_interference;
        engine.effects = srs_audio::EffectSettings {
            per_radio_model_effects: profile.get_bool(ProfileSettingsKey::PerRadioModelEffects),
            radio_effects_ratio: profile.get_f32(ProfileSettingsKey::RadioEffectsRatio),
            radio_encryption_effects: profile.get_bool(ProfileSettingsKey::RadioEncryptionEffects),
            clipping: profile.get_bool(ProfileSettingsKey::RadioEffectsClipping),
            background_noise: profile.get_bool(ProfileSettingsKey::RadioBackgroundNoiseEffect),
            noise_gain_db: profile.get_f32(ProfileSettingsKey::NoiseGainDB),
            hf_noise_gain_db: profile.get_f32(ProfileSettingsKey::HFNoiseGainDB),
            nato_tone: profile.get_bool(ProfileSettingsKey::NATOTone),
            nato_tone_volume: profile.get_f32(ProfileSettingsKey::NATOToneVolume),
            havequick_tone: profile.get_bool(ProfileSettingsKey::HAVEQUICKTone),
            hq_tone_volume: profile.get_f32(ProfileSettingsKey::HQToneVolume),
            rx_effects_start: profile.get_bool(ProfileSettingsKey::RadioRxEffects_Start),
            rx_effects_end: profile.get_bool(ProfileSettingsKey::RadioRxEffects_End),
            tx_effects_start: profile.get_bool(ProfileSettingsKey::RadioTxEffects_Start),
            tx_effects_end: profile.get_bool(ProfileSettingsKey::RadioTxEffects_End),
            mids_radio_effect: profile.get_bool(ProfileSettingsKey::MIDSRadioEffect),
            radio_start_selection: profile.get_string(ProfileSettingsKey::RadioTransmissionStartSelection),
            radio_end_selection: profile.get_string(ProfileSettingsKey::RadioTransmissionEndSelection),
            intercom_start_selection: profile
                .get_string(ProfileSettingsKey::IntercomTransmissionStartSelection),
            intercom_end_selection: profile.get_string(ProfileSettingsKey::IntercomTransmissionEndSelection),
            irl_radio_rx_interference: irl_rx,
        };
        let channels = [
            ProfileSettingsKey::IntercomChannel,
            ProfileSettingsKey::Radio1Channel,
            ProfileSettingsKey::Radio2Channel,
            ProfileSettingsKey::Radio3Channel,
            ProfileSettingsKey::Radio4Channel,
            ProfileSettingsKey::Radio5Channel,
            ProfileSettingsKey::Radio6Channel,
            ProfileSettingsKey::Radio7Channel,
            ProfileSettingsKey::Radio8Channel,
            ProfileSettingsKey::Radio9Channel,
            ProfileSettingsKey::Radio10Channel,
        ];
        for (mixer, key) in engine.master.radios.iter_mut().zip(channels) {
            mixer.balance = profile.get_f32(key);
        }
    }
    net.dcs_ports = srs_client::DcsPorts {
        export_in: settings.get_port(GlobalSettingsKey::DCSIncomingUDP),
        gamegui_in: settings.get_port(GlobalSettingsKey::DCSIncomingGameGUIUDP),
        los_in: settings.get_port(GlobalSettingsKey::DCSLOSIncomingUDP),
        los_out: settings.get_port(GlobalSettingsKey::DCSLOSOutgoingUDP),
        autoconnect_in: settings.get_port(GlobalSettingsKey::DCSAutoConnectUDP),
        command_in: settings.get_port(GlobalSettingsKey::CommandListenerUDP),
        info_out: settings.get_port(GlobalSettingsKey::OutgoingDCSUDPInfo),
        other_out: settings.get_port(GlobalSettingsKey::OutgoingDCSUDPOther),
    };
}

/// Native window handle for DirectInput (`SetCooperativeLevel`). 0 off Windows.
fn window_hwnd(cc: &eframe::CreationContext<'_>) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}
