//! Settings tab: global (`global.cfg`) and profile (`<profile>.cfg`)
//! options, written through the stores immediately like the C# client.

use eframe::egui;
use srs_settings::{GlobalSettings, GlobalSettingsKey as G, ProfileSettings, ProfileSettingsKey as P};

/// What the caller must refresh after the tab ran.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SettingsChange {
    /// Any global or profile value changed: push settings to the client.
    pub changed: bool,
    /// Audio devices / mic chain changed: restart audio if connected.
    pub audio_restart: bool,
    /// "Reset Radio Overlay" pressed.
    pub reset_overlay: bool,
}

fn toggle_g(ui: &mut egui::Ui, s: &mut GlobalSettings, key: G, label: &str, hint: &str) -> bool {
    let mut v = s.get_bool(key);
    let r = ui.checkbox(&mut v, label).on_hover_text(hint);
    if r.changed() {
        s.set_bool(key, v);
    }
    r.changed()
}

fn toggle_p(ui: &mut egui::Ui, p: &mut ProfileSettings, key: P, label: &str, hint: &str) -> bool {
    let mut v = p.get_bool(key);
    let r = ui.checkbox(&mut v, label).on_hover_text(hint);
    if r.changed() {
        p.set_bool(key, v);
    }
    r.changed()
}

fn slider_p(
    ui: &mut egui::Ui,
    p: &mut ProfileSettings,
    key: P,
    label: &str,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let mut v = p.get_f32(key);
    let r = ui.add(egui::Slider::new(&mut v, range).text(label));
    if r.changed() {
        p.set_f32(key, v);
    }
    r.changed()
}

fn int_g(
    ui: &mut egui::Ui,
    s: &mut GlobalSettings,
    key: G,
    label: &str,
    range: std::ops::RangeInclusive<i32>,
) -> bool {
    let mut v = s.get_i32(key);
    let r = ui.add(egui::Slider::new(&mut v, range).text(label));
    if r.changed() {
        s.set_i32(key, v);
    }
    r.changed()
}

fn float_g(
    ui: &mut egui::Ui,
    s: &mut GlobalSettings,
    key: G,
    label: &str,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    let mut v = s.get_f64(key);
    let r = ui.add(egui::Slider::new(&mut v, range).text(label));
    if r.changed() {
        s.set_f64(key, v);
    }
    r.changed()
}

/// Left / Both / Right selector stored as a balance (-1, 0, 1).
fn channel_p(ui: &mut egui::Ui, p: &mut ProfileSettings, key: P, label: &str) -> bool {
    let current = p.get_f32(key);
    let mut choice = if current < -0.5 {
        0
    } else if current > 0.5 {
        2
    } else {
        1
    };
    let before = choice;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.selectable_value(&mut choice, 0, "Left");
        ui.selectable_value(&mut choice, 1, "Both");
        ui.selectable_value(&mut choice, 2, "Right");
    });
    if choice != before {
        p.set_f32(key, [-1.0, 0.0, 1.0][choice]);
        true
    } else {
        false
    }
}

/// Dropdown over the effect WAV file names found in `AudioEffects/`.
fn selection_p(ui: &mut egui::Ui, p: &mut ProfileSettings, key: P, label: &str, options: &[String]) -> bool {
    let mut current = p.get_string(key);
    let mut changed = false;
    egui::ComboBox::from_label(label)
        .selected_text(current.clone())
        .show_ui(ui, |ui| {
            for o in options {
                if ui.selectable_value(&mut current, o.clone(), o).changed() {
                    changed = true;
                }
            }
        });
    if changed {
        p.set_string(key, &current);
    }
    changed
}

pub struct SettingsLists<'a> {
    pub radio_start: &'a [String],
    pub radio_end: &'a [String],
    pub intercom_start: &'a [String],
    pub intercom_end: &'a [String],
}

pub fn settings_tab(
    ui: &mut egui::Ui,
    s: &mut GlobalSettings,
    p: &mut ProfileSettings,
    lists: &SettingsLists<'_>,
) -> SettingsChange {
    let mut c = SettingsChange::default();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.collapsing("General (global)", |ui| {
            c.changed |= toggle_g(
                ui,
                s,
                G::AutoConnect,
                "Auto connect",
                "Connect to the server announced by the DCS server",
            );
            c.changed |= toggle_g(
                ui,
                s,
                G::AutoConnectPrompt,
                "Auto connect prompt",
                "Ask before auto-connecting",
            );
            c.changed |= toggle_g(
                ui,
                s,
                G::AutoConnectMismatchPrompt,
                "Auto connect mismatch prompt",
                "Ask when already connected to another server",
            );
            if ui
                .button("Reset radio overlay")
                .on_hover_text("Position 300/300, size 122x270, opacity 1")
                .clicked()
            {
                c.reset_overlay = true;
            }
            c.changed |= toggle_g(ui, s, G::RadioOverlayTaskbarHide, "Hide overlay taskbar item", "");
            c.changed |= toggle_g(
                ui,
                s,
                G::RefocusDCS,
                "Auto refocus DCS",
                "Not implemented in the Rust client",
            );
            c.changed |= toggle_g(
                ui,
                s,
                G::ExpandControls,
                "Allow more input devices",
                "Accept almost any DirectInput device (restart to apply)",
            );
            c.changed |= toggle_g(
                ui,
                s,
                G::AllowXInputController,
                "Allow XInput controllers",
                "Restart to apply",
            );
            c.changed |= toggle_g(
                ui,
                s,
                G::AutoSelectSettingsProfile,
                "Auto select profile for aircraft",
                "Profile named after the aircraft (and _2 for the second seat)",
            );
            c.changed |= toggle_g(ui, s, G::PlayConnectionSounds, "Play connection sounds", "");
            c.changed |= toggle_g(
                ui,
                s,
                G::ShowTransmitterName,
                "Show transmitter name",
                "When the server allows it",
            );
            c.changed |= toggle_g(ui, s, G::AllowMultipleInstances, "Allow multiple instances", "");
            c.changed |= toggle_g(
                ui,
                s,
                G::AllowRecording,
                "Allow recording",
                "Lets other clients record you (recording itself is not implemented)",
            );
            c.changed |= toggle_g(
                ui,
                s,
                G::MinimiseToTray,
                "Minimise to tray",
                "Not implemented in the Rust client",
            );
            c.changed |= toggle_g(ui, s, G::StartMinimised, "Start minimised", "");
            c.changed |= toggle_g(ui, s, G::DarkMode, "Dark mode", "");
            c.changed |= int_g(ui, s, G::IdleTimeOut, "Idle timeout (s)", 60..=3600);
        });
        ui.collapsing("Microphone (global)", |ui| {
            let mut mic = false;
            mic |= toggle_g(
                ui,
                s,
                G::Denoise,
                "Noise suppression",
                "Speex denoise on the microphone",
            );
            mic |= int_g(ui, s, G::DenoiseAttenuation, "Denoise attenuation (dB)", -60..=0);
            mic |= toggle_g(
                ui,
                s,
                G::VOX,
                "VOX (voice activation)",
                "Transmit without PTT when speaking",
            );
            mic |= int_g(ui, s, G::VOXMode, "VOX aggressiveness", 0..=3);
            mic |= int_g(ui, s, G::VOXMinimumTime, "VOX hold time (ms)", 0..=3000);
            mic |= float_g(ui, s, G::VOXMinimumDB, "VOX minimum level (dB)", -80.0..=0.0);
            mic |= float_g(ui, s, G::MicBoost, "Mic boost", 0.0..=1.0);
            c.changed |= mic;
        });
        ui.collapsing("Incoming audio (global)", |ui| {
            c.changed |= toggle_g(ui, s, G::IncomingAudioAGC, "Automatic gain control", "");
            c.changed |= int_g(ui, s, G::IncomingAudioAGCTarget, "AGC target", 1000..=32000);
            c.changed |= int_g(ui, s, G::IncomingAudioAGCDecrement, "AGC decrement", -100..=0);
            c.changed |= int_g(ui, s, G::IncomingAudioAGCLevelMax, "AGC max gain", 0..=60);
            c.changed |= toggle_g(ui, s, G::IncomingAudioDenoise, "Denoise incoming audio", "");
            c.changed |= int_g(
                ui,
                s,
                G::IncomingAudioDenoiseAttenuation,
                "Incoming denoise attenuation (dB)",
                -60..=0,
            );
        });
        ui.collapsing("Cockpit integration (profile)", |ui| {
            c.changed |= toggle_p(ui, p, P::RadioSwitchIsPTT, "Radio switch works as PTT", "");
            c.changed |= toggle_p(
                ui,
                p,
                P::RadioSwitchIsPTTOnlyWhenValid,
                "... only when the radio is valid",
                "",
            );
            c.changed |= toggle_p(
                ui,
                p,
                P::AutoSelectPresetChannel,
                "Auto select first channel preset",
                "",
            );
            c.changed |= toggle_p(
                ui,
                p,
                P::AlwaysAllowHotasControls,
                "ALWAYS allow SRS hotkeys",
                "Even when the aircraft controls its radios",
            );
            c.changed |= toggle_p(
                ui,
                p,
                P::AlwaysAllowTransponderOverlay,
                "ALWAYS allow transponder overlay controls",
                "",
            );
            c.changed |= toggle_p(ui, p, P::AllowDCSPTT, "Allow in-cockpit DCS controlled PTT", "");
            c.changed |= toggle_p(
                ui,
                p,
                P::RotaryStyleIncrement,
                "Rotary style frequency increment",
                "123.900 + 0.1 → 123.000",
            );
            c.changed |= toggle_p(ui, p, P::DisableExpansionRadios, "Disable expansion radios", "");
            c.changed |= toggle_p(
                ui,
                p,
                P::InstructorMode,
                "Instructor mode",
                "Intercom follows the chosen unit",
            );
            c.changed |= toggle_p(
                ui,
                p,
                P::AllowServerEAMRadioPreset,
                "Allow server EAM radio preset",
                "",
            );
            c.changed |= slider_p(ui, p, P::PTTStartDelay, "PTT start delay (ms)", 0.0..=2000.0);
            c.changed |= slider_p(ui, p, P::PTTReleaseDelay, "PTT release delay (ms)", 0.0..=2000.0);
            let mut sel = p.get_string(P::ServerPresetSelection);
            let before = sel.clone();
            egui::ComboBox::from_label("Server presets")
                .selected_text(sel.clone())
                .show_ui(ui, |ui| {
                    for o in [
                        "USE_CLIENT_AND_SERVER_IF_SET",
                        "USE_CLIENT_ONLY",
                        "USE_SERVER_ONLY_IF_SET",
                    ] {
                        ui.selectable_value(&mut sel, o.to_string(), o);
                    }
                });
            if sel != before {
                p.set_string(P::ServerPresetSelection, &sel);
                c.changed = true;
            }
        });
        ui.collapsing("Radio effects (profile)", |ui| {
            c.changed |= toggle_p(ui, p, P::RadioRxEffects_Start, "RX start effect", "");
            c.changed |= toggle_p(ui, p, P::RadioRxEffects_End, "RX end effect", "");
            c.changed |= toggle_p(ui, p, P::RadioTxEffects_Start, "TX start effect", "");
            c.changed |= toggle_p(ui, p, P::RadioTxEffects_End, "TX end effect", "");
            c.changed |= selection_p(
                ui,
                p,
                P::RadioTransmissionStartSelection,
                "Radio start sound",
                lists.radio_start,
            );
            c.changed |= selection_p(
                ui,
                p,
                P::RadioTransmissionEndSelection,
                "Radio end sound",
                lists.radio_end,
            );
            c.changed |= selection_p(
                ui,
                p,
                P::IntercomTransmissionStartSelection,
                "Intercom start sound",
                lists.intercom_start,
            );
            c.changed |= selection_p(
                ui,
                p,
                P::IntercomTransmissionEndSelection,
                "Intercom end sound",
                lists.intercom_end,
            );
            c.changed |= toggle_p(
                ui,
                p,
                P::RadioEncryptionEffects,
                "Encryption effects",
                "KY-58 tones + CVSD",
            );
            c.changed |= toggle_p(ui, p, P::MIDSRadioEffect, "MIDS TX & RX effects", "");
            c.changed |= slider_p(
                ui,
                p,
                P::RadioEffectsRatio,
                "Radio voice effect amount",
                0.0..=1.0,
            );
            c.changed |= toggle_p(ui, p, P::PerRadioModelEffects, "Per radio model effects", "");
            c.changed |= toggle_p(ui, p, P::RadioEffectsClipping, "Clipping", "");
            c.changed |= toggle_p(ui, p, P::NATOTone, "FM radio tone", "");
            c.changed |= slider_p(ui, p, P::NATOToneVolume, "FM tone volume", 0.0..=2.0);
            c.changed |= toggle_p(ui, p, P::HAVEQUICKTone, "HAVEQUICK effect", "");
            c.changed |= slider_p(ui, p, P::HQToneVolume, "HAVEQUICK tone volume", 0.0..=2.0);
            c.changed |= toggle_p(ui, p, P::RadioBackgroundNoiseEffect, "Background radio noise", "");
            c.changed |= slider_p(ui, p, P::NoiseGainDB, "V/UHF noise gain (dB)", -30.0..=30.0);
            c.changed |= slider_p(ui, p, P::HFNoiseGainDB, "HF noise gain (dB)", -30.0..=30.0);
            c.changed |= toggle_p(ui, p, P::AmbientCockpitNoiseEffect, "Ambient cockpit noise", "");
            c.changed |= slider_p(
                ui,
                p,
                P::AmbientCockpitNoiseEffectVolume,
                "Ambient cockpit volume",
                0.0..=1.5,
            );
            c.changed |= toggle_p(
                ui,
                p,
                P::AmbientCockpitIntercomNoiseEffect,
                "Ambient cockpit noise on intercom",
                "",
            );
        });
        ui.collapsing("Audio channels (profile)", |ui| {
            c.changed |= channel_p(ui, p, P::IntercomChannel, "Intercom");
            for (i, key) in [
                P::Radio1Channel,
                P::Radio2Channel,
                P::Radio3Channel,
                P::Radio4Channel,
                P::Radio5Channel,
                P::Radio6Channel,
                P::Radio7Channel,
                P::Radio8Channel,
                P::Radio9Channel,
                P::Radio10Channel,
            ]
            .into_iter()
            .enumerate()
            {
                c.changed |= channel_p(ui, p, key, &format!("Radio {}", i + 1));
            }
        });
    });
    c
}
