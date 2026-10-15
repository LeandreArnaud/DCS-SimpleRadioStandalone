//! Controls tab: bindings of the current profile, assignment, rescan.

use std::collections::BTreeMap;

use eframe::egui;
use srs_input::backend::keys::SHARPDX_KEY_NAMES;
use srs_input::{InputBinding, InputDevice};

/// Human description of a stored binding.
pub fn describe(d: &InputDevice) -> String {
    let what = if d.device_name.eq_ignore_ascii_case("keyboard")
        || d.device_name.to_ascii_lowercase().contains("keyboard")
    {
        SHARPDX_KEY_NAMES
            .get(d.button as usize)
            .map(|n| format!("key {n}"))
            .unwrap_or_else(|| format!("key #{}", d.button))
    } else if d.button >= 128 {
        format!("POV {} @ {}°", d.button - 128, d.button_value / 100)
    } else {
        format!("button {}", d.button)
    };
    format!("{} — {what}", d.device_name)
}

/// Bindings shown in the tab (non-modifier first, modifier beside it).
pub fn rows() -> Vec<InputBinding> {
    InputBinding::ALL
        .iter()
        .copied()
        .filter(|b| !b.is_modifier())
        .collect()
}

pub enum ControlsAction {
    Assign(InputBinding),
    Clear(InputBinding),
    Rescan,
}

pub fn controls_panel(
    ui: &mut egui::Ui,
    bindings: &BTreeMap<InputBinding, InputDevice>,
    pending_assign: Option<InputBinding>,
    devices: &[String],
) -> Option<ControlsAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        if ui.button("Rescan controllers").clicked() {
            action = Some(ControlsAction::Rescan);
        }
        ui.label(format!("{} device(s)", devices.len()));
    });
    if let Some(b) = pending_assign {
        ui.colored_label(
            egui::Color32::YELLOW,
            format!("Press the input for {}…", b.as_str()),
        );
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("bindings")
            .striped(true)
            .min_col_width(60.0)
            .show(ui, |ui| {
                ui.strong("Action");
                ui.strong("Binding");
                ui.strong("");
                ui.strong("Modifier");
                ui.strong("");
                ui.end_row();
                for b in rows() {
                    ui.label(b.as_str());
                    for (bind, is_mod) in [(b, false), (b.modifier().unwrap_or(b), true)] {
                        if is_mod && bind == b {
                            ui.label("-");
                            ui.label("");
                            continue;
                        }
                        match bindings.get(&bind) {
                            Some(d) => {
                                ui.label(describe(d));
                            }
                            None => {
                                ui.label("");
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.small_button("Set").clicked() {
                                action = Some(ControlsAction::Assign(bind));
                            }
                            if bindings.contains_key(&bind) && ui.small_button("Clear").clicked() {
                                action = Some(ControlsAction::Clear(bind));
                            }
                        });
                    }
                    ui.end_row();
                }
            });
    });
    action
}
