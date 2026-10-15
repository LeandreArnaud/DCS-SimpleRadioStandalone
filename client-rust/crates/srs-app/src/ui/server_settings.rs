//! Server settings (`ServerSettingsWindow`): read-only view of the synced
//! server settings.

use eframe::egui;
use srs_client::Client;
use srs_protocol::ServerSettingsKey as K;

pub fn server_settings_tab(ui: &mut egui::Ui, net: &Client, server_version: Option<&str>) {
    let Ok(s) = net.shared.server_settings.read() else {
        return;
    };
    let on_off = |b: bool| if b { "ON" } else { "OFF" };
    let en = |b: bool| if b { "ENABLED" } else { "DISABLED" };
    let rows: [(&str, String); 16] = [
        (
            "Spectator audio",
            en(!s.get_bool(K::SPECTATORS_AUDIO_DISABLED)).into(),
        ),
        (
            "Coalition security",
            on_off(s.get_bool(K::COALITION_AUDIO_SECURITY)).into(),
        ),
        ("Line of sight", on_off(s.get_bool(K::LOS_ENABLED)).into()),
        ("Distance", on_off(s.get_bool(K::DISTANCE_ENABLED)).into()),
        ("Real radio TX", on_off(s.get_bool(K::IRL_RADIO_TX)).into()),
        (
            "Radio RX interference",
            on_off(s.get_bool(K::IRL_RADIO_RX_INTERFERENCE)).into(),
        ),
        ("Radio expansion", on_off(s.get_bool(K::RADIO_EXPANSION)).into()),
        (
            "External AWACS mode",
            on_off(s.get_bool(K::EXTERNAL_AWACS_MODE)).into(),
        ),
        (
            "Allow radio encryption",
            on_off(s.get_bool(K::ALLOW_RADIO_ENCRYPTION)).into(),
        ),
        (
            "Strict radio encryption",
            on_off(s.get_bool(K::STRICT_RADIO_ENCRYPTION)).into(),
        ),
        (
            "Tuned client count",
            on_off(s.get_bool(K::SHOW_TUNED_COUNT)).into(),
        ),
        (
            "Show transmitter name",
            on_off(s.get_bool(K::SHOW_TRANSMITTER_NAME)).into(),
        ),
        ("Server presets", en(s.get_bool(K::SERVER_PRESETS_ENABLED)).into()),
        (
            "Server EAM radio preset",
            en(s.get_bool(K::SERVER_EAM_RADIO_PRESET_ENABLED)).into(),
        ),
        ("Instructor mode", en(s.get_bool(K::ALLOW_INSTRUCTOR_MODE)).into()),
        ("Retransmit node limit", s.retransmit_node_limit.to_string()),
    ];
    egui::Grid::new("server-settings").striped(true).show(ui, |ui| {
        ui.strong("Server version");
        ui.label(server_version.unwrap_or("-"));
        ui.end_row();
        for (label, value) in rows {
            ui.label(label);
            ui.label(value);
            ui.end_row();
        }
        ui.label("Global frequencies (MHz)");
        ui.label(
            s.global_frequencies_hz
                .iter()
                .map(|f| format!("{:.3}", f / 1e6))
                .collect::<Vec<_>>()
                .join(", "),
        );
        ui.end_row();
        ui.label("Test frequencies (MHz)");
        ui.label(
            s.test_frequencies_hz()
                .iter()
                .map(|f| format!("{:.3}", f / 1e6))
                .collect::<Vec<_>>()
                .join(", "),
        );
        ui.end_row();
    });
}
