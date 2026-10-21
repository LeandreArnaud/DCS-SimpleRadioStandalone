//! Client list (`ClientListWindow`): blue, then red, then spectators, each
//! alphabetical; "R" when the client allows recording; local mute.

use eframe::egui;
use srs_client::Client;

use super::theme::{self, palette as pal};

pub fn clients_tab(ui: &mut egui::Ui, net: &Client, my_guid: &str) {
    let mut clients = net.shared.snapshot();
    clients.sort_by(|a, b| {
        b.client
            .coalition
            .cmp(&a.client.coalition)
            .then_with(|| a.client.name.to_lowercase().cmp(&b.client.name.to_lowercase()))
    });
    ui.label(
        egui::RichText::new(format!("{} clients", clients.len()))
            .small()
            .color(pal::TEXT_DIM),
    );
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("clients")
            .striped(true)
            .min_col_width(30.0)
            .show(ui, |ui| {
                ui.strong("Name");
                ui.strong("Rec");
                ui.strong("Side");
                ui.strong("Unit");
                ui.strong("Mute");
                ui.strong("");
                ui.end_row();
                for c in &clients {
                    let me = c.client.client_guid == my_guid;
                    ui.label(if me {
                        format!("{} (me)", c.client.name)
                    } else {
                        c.client.name.clone()
                    });
                    ui.label(if c.client.allow_record { "R" } else { "-" });
                    let color = match c.client.coalition {
                        1 => pal::COALITION_RED,
                        2 => pal::COALITION_BLUE,
                        _ => pal::GRAY,
                    };
                    theme::led(ui, color, 12.0);
                    ui.label(
                        c.client
                            .radio_info
                            .as_ref()
                            .map(|r| r.unit.clone())
                            .unwrap_or_default(),
                    );
                    ui.label(if c.local.muted { "Muted" } else { "" });
                    if !me && ui.small_button("Toggle Mute").clicked() {
                        net.shared.set_muted(&c.client.client_guid, !c.local.muted);
                    }
                    ui.end_row();
                }
            });
    });
}
