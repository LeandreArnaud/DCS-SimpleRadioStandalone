//! AWACS overlay (`AwacsOverlay`, 10 radios + intercom + global SIMUL +
//! instructor mode), usable inline (Radios tab) or as an always-on-top
//! viewport.

use eframe::egui;
use srs_client::RadioState;
use srs_dcs::radio_helper as rh;
use srs_dcs::{FreqMode, SimultaneousTransmissionControl};
use srs_protocol::Modulation;

use super::radio_widgets::{self as rw, RadioAction, RadioUiCtx};

pub const MIN_W: f32 = 805.0;
pub const MIN_H: f32 = 220.0;
/// `MaxSimultaneousTransmissions`.
const MAX_SIMUL: usize = 3;

/// Extra per-frame inputs for the AWACS panel.
pub struct AwacsCtx<'a> {
    pub base: RadioUiCtx<'a>,
    pub eam_active: bool,
    /// Server `ALLOW_INSTRUCTOR_MODE` AND profile `InstructorMode`.
    pub instructor_allowed: bool,
    /// Radios currently replaced by instructor mode.
    pub instructor_active: &'a [usize],
    /// `(unit_id, label)` candidates for instructor mode.
    pub units: &'a [(u32, String)],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AwacsGeometry {
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
}

impl Default for AwacsGeometry {
    fn default() -> Self {
        Self {
            x: 300.0,
            y: 300.0,
            opacity: 1.0,
        }
    }
}

pub struct AwacsOut {
    pub close: bool,
    pub geometry: AwacsGeometry,
    pub actions: Vec<RadioAction>,
}

pub fn awacs_viewport(
    egui_ctx: &egui::Context,
    ctx: &AwacsCtx<'_>,
    geometry: AwacsGeometry,
    taskbar_hide: bool,
) -> AwacsOut {
    let builder = egui::ViewportBuilder::default()
        .with_title("SRS AWACS Overlay")
        .with_always_on_top()
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(true)
        .with_taskbar(!taskbar_hide)
        .with_position([geometry.x, geometry.y])
        .with_inner_size([MIN_W, MIN_H])
        .with_min_inner_size([MIN_W, MIN_H]);
    let mut out = AwacsOut {
        close: false,
        geometry,
        actions: Vec::new(),
    };
    egui_ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("awacs-overlay"),
        builder,
        |ui, _class| {
            if let Some(r) = ui.ctx().input(|i| i.viewport().outer_rect) {
                out.geometry.x = r.min.x;
                out.geometry.y = r.min.y;
            }
            let fill = egui::Color32::from_black_alpha(200);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(fill).inner_margin(4.0))
                .show(ui, |ui| {
                    ui.set_opacity(out.geometry.opacity.clamp(0.05, 1.0));
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("SRS AWACS").strong().color(rw::WHITE));
                        ui.add(egui::Slider::new(&mut out.geometry.opacity, 0.05..=1.0).show_value(false))
                            .on_hover_text("Opacity");
                        if !taskbar_hide && ui.small_button("_").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if ui.small_button("x").clicked() {
                            out.close = true;
                        }
                    });
                    out.actions = awacs_panel(ui, ctx, true);
                    if ui
                        .interact(ui.max_rect(), ui.id().with("drag"), egui::Sense::drag())
                        .dragged()
                    {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                });
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                out.close = true;
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(80));
        },
    );
    out
}

/// The 10 radio groups + intercom + global SIMUL. `dark` = overlay colours.
pub fn awacs_panel(ui: &mut egui::Ui, ctx: &AwacsCtx<'_>, dark: bool) -> Vec<RadioAction> {
    let mut actions = Vec::new();
    let mut rs = ctx.base.net.radio_state.lock();
    let text_color = if dark {
        rw::WHITE
    } else {
        ui.visuals().text_color()
    };
    // Global simultaneous transmission button.
    ui.horizontal(|ui| {
        let enabled = ctx.base.connected
            && rs.dcs.is_current()
            && rs.dcs.simultaneous_transmission_control
                == SimultaneousTransmissionControl::EnabledInternalSrsControls;
        let on = rs.dcs.simultaneous_transmission;
        let (label, color) = if on {
            ("SIMUL ON", rw::ORANGE)
        } else {
            ("SIMUL OFF", text_color)
        };
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(egui::RichText::new(label).color(color)),
            )
            .clicked()
        {
            rh::toggle_global_simultaneous_transmission(&mut rs.dcs);
            rs.force_sync = true;
        }
        rw::intercom_spinner(ui, &mut rs);
        let unit = rs.dcs.unit.clone();
        ui.label(
            egui::RichText::new(format!("{unit} ({})", rs.dcs.unit_id))
                .small()
                .color(text_color),
        );
    });
    let connected = ctx.base.connected && rs.dcs.is_current();
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            intercom_group(ui, &mut rs, connected, text_color);
            for i in 1..rs.dcs.radios.len().min(11) {
                ui.separator();
                radio_group(ui, ctx, &mut rs, i, connected, text_color, &mut actions);
            }
        });
    });
    actions
}

fn intercom_group(ui: &mut egui::Ui, rs: &mut RadioState, connected: bool, text_color: egui::Color32) {
    ui.vertical(|ui| {
        ui.set_width(70.0);
        let is_intercom = connected
            && rs
                .dcs
                .radios
                .first()
                .is_some_and(|r| r.modulation == Modulation::Intercom);
        let (rx, _, _) = rw::receiving(rs, 0);
        let color = if !is_intercom {
            rw::RED
        } else if rx {
            rw::GREEN_TX
        } else {
            rw::indicator_color(rs, 0, true)
        };
        if rw::indicator(ui, color, 12.0) && is_intercom {
            rw::select(rs, 0);
        }
        ui.label(egui::RichText::new(if is_intercom { "Intercom" } else { "No Intercom" }).color(text_color));
        if is_intercom {
            rw::volume_slider(ui, rs, 0, true);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn radio_group(
    ui: &mut egui::Ui,
    ctx: &AwacsCtx<'_>,
    rs: &mut RadioState,
    i: usize,
    connected: bool,
    text_color: egui::Color32,
    actions: &mut Vec<RadioAction>,
) {
    ui.vertical(|ui| {
        ui.set_width(74.0);
        let radio = rs.dcs.radios[i].clone();
        if !connected || radio.modulation == Modulation::Disabled {
            ui.horizontal(|ui| {
                rw::indicator(ui, rw::RED, 12.0);
                ui.label(egui::RichText::new("No Radio").small().color(text_color));
            });
            ui.label(egui::RichText::new("---").monospace().color(rw::RX_GREEN));
            return;
        }
        ui.horizontal(|ui| {
            if rw::indicator(ui, rw::indicator_color(rs, i, true), 12.0) {
                rw::select(rs, i);
            }
            let name = if radio.modulation == Modulation::Intercom {
                format!("INSTR. INT. {i}")
            } else {
                radio.name.clone()
            };
            ui.label(egui::RichText::new(name).small().color(text_color));
        });
        let (rx, secondary, sent_by) = rw::receiving(rs, i);
        if rx && !sent_by.is_empty() {
            let t =
                egui::RichText::new(sent_by)
                    .monospace()
                    .color(if secondary { rw::RED } else { rw::WHITE });
            rw::freq_label(ui, rs, i, t);
        } else if radio.modulation == Modulation::Intercom {
            ui.label(
                egui::RichText::new(format!("INT. {i}"))
                    .monospace()
                    .color(rw::RX_GREEN),
            );
        } else {
            editable_frequency(ui, ctx, rs, i, &radio);
            let tuned = rw::tuned_count(&ctx.base, rs, &radio);
            let meta = egui::RichText::new(rw::awacs_metadata(&radio, tuned))
                .small()
                .color(rw::RX_GREEN);
            rw::freq_label(ui, rs, i, meta);
        }
        if radio.modulation != Modulation::Intercom {
            rw::frequency_buttons(ui, rs, i, ctx.base.rotary_style);
            ui.horizontal(|ui| {
                rw::volume_slider(ui, rs, i, true);
                rw::rt_button(ui, rs, i, ctx.base.node_limit);
            });
            // Per-radio SIMUL.
            let others = rs
                .dcs
                .radios
                .iter()
                .enumerate()
                .filter(|(j, r)| *j != i && r.simul)
                .count();
            let enabled = rs.dcs.simultaneous_transmission_control
                == SimultaneousTransmissionControl::EnabledInternalSrsControls
                && !radio.rx_only
                && rs.dcs.simultaneous_transmission
                && others < MAX_SIMUL;
            let color = if radio.simul { rw::ORANGE } else { text_color };
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(egui::RichText::new("SIMUL").small().color(color)),
                )
                .clicked()
                && rh::toggle_simultaneous(&mut rs.dcs, i)
            {
                rs.force_sync = true;
            }
            if radio.freq_mode == FreqMode::Overlay {
                let channels = ctx
                    .base
                    .net
                    .fixed_channels
                    .lock()
                    .get(i)
                    .cloned()
                    .unwrap_or_default();
                rw::channel_controls(ui, rs, i, &channels, actions);
            }
            rw::encryption_controls(ui, rs, i);
        }
        if ctx.instructor_allowed && ctx.eam_active {
            instructor_controls(ui, ctx, i, actions);
        }
    });
}

/// Editable frequency text (MHz, or MIDS channel), commit on Enter / focus loss.
fn editable_frequency(
    ui: &mut egui::Ui,
    ctx: &AwacsCtx<'_>,
    rs: &mut RadioState,
    i: usize,
    radio: &srs_dcs::DcsRadio,
) {
    let id = ui.id().with(("awacs-freq", i));
    let shown = if radio.modulation == Modulation::Mids {
        let ch = srs_protocol::radio_calc::link16::frequency_to_channel(radio.freq);
        if ch > 0 {
            ch.to_string()
        } else {
            "OFF".into()
        }
    } else {
        format!("{:.3}", radio.freq / 1e6)
    };
    let editing: Option<String> = ui.data_mut(|d| d.get_temp::<String>(id));
    let mut text = editing.clone().unwrap_or(shown);
    let can_edit = radio.freq_mode == FreqMode::Overlay && rs.dcs.is_current();
    let resp = ui.add_enabled(
        can_edit,
        egui::TextEdit::singleline(&mut text)
            .desired_width(64.0)
            .char_limit(7),
    );
    if resp.changed() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    let commit = resp.lost_focus() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
    if commit {
        if let Ok(v) = text.replace(',', ".").trim().parse::<f64>() {
            let (changed, _) = rh::update_frequency(&mut rs.dcs, v, i, false, true, ctx.base.rotary_style);
            if changed {
                rs.force_sync = true;
            }
        }
        ui.data_mut(|d| d.remove::<String>(id));
        if resp.has_focus() {
            resp.surrender_focus();
        }
    } else if !resp.has_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    if resp.secondary_clicked() {
        if rh::toggle_guard(&mut rs.dcs, i) {
            rs.force_sync = true;
        }
        rw::select(rs, i);
    }
}

fn instructor_controls(ui: &mut egui::Ui, ctx: &AwacsCtx<'_>, i: usize, actions: &mut Vec<RadioAction>) {
    let active = ctx.instructor_active.contains(&i);
    ui.horizontal(|ui| {
        let label = if active { "Instr. ON" } else { "Instructor" };
        egui::ComboBox::from_id_salt(("instr", i))
            .selected_text(label)
            .width(70.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(!active, "None: Disabled").clicked() && active {
                    actions.push(RadioAction::StopInstructor(i));
                }
                for (unit_id, name) in ctx.units {
                    if ui.selectable_label(false, name).clicked() {
                        actions.push(RadioAction::StartInstructor {
                            radio: i,
                            unit_id: *unit_id,
                        });
                    }
                }
            });
        if active && ui.small_button("Stop").clicked() {
            actions.push(RadioAction::StopInstructor(i));
        }
    });
}
