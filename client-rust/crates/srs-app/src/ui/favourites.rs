//! Favourites tab (`FavouriteServersView`): editable list backed by
//! `FavouriteServers.csv`.

use eframe::egui;
use srs_settings::FavouriteStore;

#[derive(Default)]
pub struct FavouritesUi {
    pub new_name: String,
    pub new_address: String,
    pub new_password: String,
}

/// Returns `Some(index)` when the user picked a favourite to use.
pub fn favourites_tab(
    ui: &mut egui::Ui,
    state: &mut FavouritesUi,
    store: &mut FavouriteStore,
    last_server: &str,
) -> Option<usize> {
    let mut picked = None;
    let mut dirty = false;
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut state.new_name).desired_width(120.0));
        ui.label("Address");
        ui.add(egui::TextEdit::singleline(&mut state.new_address).desired_width(160.0));
        ui.label("EAM password");
        ui.add(egui::TextEdit::singleline(&mut state.new_password).desired_width(100.0));
        if ui.button("Add").clicked() && !state.new_address.trim().is_empty() {
            let name = if state.new_name.trim().is_empty() {
                state.new_address.clone()
            } else {
                state.new_name.clone()
            };
            store.add(name.trim(), state.new_address.trim(), state.new_password.trim());
            state.new_name.clear();
            state.new_address.clear();
            state.new_password.clear();
            dirty = true;
        }
    });
    ui.separator();
    let mut remove = None;
    let mut make_default = None;
    let mut move_up = None;
    let mut move_down = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("favourites").striped(true).show(ui, |ui| {
            ui.strong("Name");
            ui.strong("Address");
            ui.strong("EAM password");
            ui.strong("Default");
            ui.strong("");
            ui.end_row();
            let len = store.servers.len();
            for (i, s) in store.servers.iter_mut().enumerate() {
                dirty |= ui
                    .add(egui::TextEdit::singleline(&mut s.name).desired_width(120.0))
                    .lost_focus();
                dirty |= ui
                    .add(egui::TextEdit::singleline(&mut s.address).desired_width(160.0))
                    .lost_focus();
                let mut pw = s.eam_password.clone().unwrap_or_default();
                if ui
                    .add(egui::TextEdit::singleline(&mut pw).desired_width(100.0))
                    .lost_focus()
                {
                    s.eam_password = if pw.trim().is_empty() { None } else { Some(pw) };
                    dirty = true;
                } else if s.eam_password.as_deref().unwrap_or("") != pw {
                    s.eam_password = if pw.is_empty() { None } else { Some(pw) };
                }
                if ui
                    .add(egui::Button::new(if s.is_default { "★" } else { "☆" }))
                    .clicked()
                {
                    make_default = Some(i);
                }
                ui.horizontal(|ui| {
                    if ui.button("Use").clicked() {
                        picked = Some(i);
                    }
                    if i > 0 && ui.small_button("▲").clicked() {
                        move_up = Some(i);
                    }
                    if i + 1 < len && ui.small_button("▼").clicked() {
                        move_down = Some(i);
                    }
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
                ui.end_row();
            }
        });
    });
    if let Some(i) = make_default {
        store.set_default(i);
        dirty = true;
    }
    if let Some(i) = move_up {
        store.move_item(i, i - 1);
        dirty = true;
    }
    if let Some(i) = move_down {
        store.move_item(i, i + 1);
        dirty = true;
    }
    if let Some(i) = remove {
        store.remove(i, last_server);
        dirty = true;
    }
    if dirty {
        if let Err(e) = store.save() {
            log::error!("Failed to save favourite servers: {e}");
        }
    }
    picked
}
