//! Single radio overlay (`RadioOverlayWindow`): always on top, borderless,
//! transparent, draggable; 3 radios (4/5 shown when available), intercom
//! and transponder. Position, size and opacity persist in `global.cfg`.

use eframe::egui;
use srs_client::RadioState;
use srs_dcs::FreqMode;
use srs_protocol::Modulation;

use super::radio_widgets::{self as rw, RadioAction, RadioUiCtx};

pub const MIN_W: f32 = 170.0;
pub const MIN_H: f32 = 355.0;

/// Persisted geometry (`RadioX/Y/Width/Height/Opacity`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayGeometry {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub opacity: f32,
}

impl Default for OverlayGeometry {
    fn default() -> Self {
        Self {
            x: 300.0,
            y: 300.0,
            w: 122.0,
            h: 270.0,
            opacity: 1.0,
        }
    }
}

pub struct OverlayOut {
    pub close: bool,
    pub geometry: OverlayGeometry,
    pub actions: Vec<RadioAction>,
}

pub fn radio_overlay(
    egui_ctx: &egui::Context,
    ctx: &RadioUiCtx<'_>,
    geometry: OverlayGeometry,
    taskbar_hide: bool,
) -> OverlayOut {
    let builder = egui::ViewportBuilder::default()
        .with_title("SRS Radio Overlay")
        .with_always_on_top()
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(true)
        .with_taskbar(!taskbar_hide)
        .with_position([geometry.x, geometry.y])
        .with_inner_size([geometry.w.max(MIN_W), geometry.h.max(MIN_H)])
        .with_min_inner_size([MIN_W, MIN_H]);
    let mut out = OverlayOut {
        close: false,
        geometry,
        actions: Vec::new(),
    };
    egui_ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("radio-overlay"),
        builder,
        |ui, _class| {
            let (outer, inner) = ui
                .ctx()
                .input(|i| (i.viewport().outer_rect, i.viewport().inner_rect));
            if let Some(r) = outer {
                out.geometry.x = r.min.x;
                out.geometry.y = r.min.y;
            }
            if let Some(r) = inner {
                out.geometry.w = r.width();
                out.geometry.h = r.height();
            }
            let scale = (out.geometry.w / MIN_W)
                .min(out.geometry.h / MIN_W)
                .clamp(0.1, 6.0);
            let fill = egui::Color32::from_black_alpha(200);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(fill).inner_margin(4.0))
                .show(ui, |ui| {
                    ui.set_opacity(out.geometry.opacity.clamp(0.05, 1.0));
                    rw::scale_ui(ui, scale);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("SRS").strong().color(rw::WHITE));
                        ui.add(egui::Slider::new(&mut out.geometry.opacity, 0.05..=1.0).show_value(false))
                            .on_hover_text("Opacity");
                        if !taskbar_hide && ui.small_button("_").on_hover_text("Minimise").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if ui.small_button("x").clicked() {
                            out.close = true;
                        }
                    });
                    let mut rs = ctx.net.radio_state.lock();
                    let count = rs.dcs.radios.len();
                    // 4th / 5th radio shown when enabled (RadioRefresh rule).
                    let enabled = |rs: &RadioState, i: usize| {
                        rs.dcs
                            .radios
                            .get(i)
                            .is_some_and(|r| r.modulation != Modulation::Disabled)
                    };
                    let show5 = enabled(&rs, 5) || count > 6;
                    let show4 = show5 || enabled(&rs, 4) || count > 5;
                    let visible: Vec<usize> = (1..=5)
                        .filter(|&i| i <= 3 || (i == 4 && show4) || (i == 5 && show5))
                        .collect();
                    if rs
                        .dcs
                        .radios
                        .iter()
                        .filter(|r| r.modulation != Modulation::Disabled)
                        .count()
                        > 1
                    {
                        let text = match rs.dcs.control {
                            srs_dcs::SwitchControls::Hotas => "HOTAS controls",
                            srs_dcs::SwitchControls::InCockpit => "Cockpit controls",
                        };
                        ui.small(egui::RichText::new(text).color(rw::WHITE));
                    }
                    for i in visible {
                        radio_group(ui, ctx, &mut rs, i, &mut out.actions);
                        ui.separator();
                    }
                    intercom_group(ui, &mut rs);
                    ui.separator();
                    rw::transponder_panel(ui, &mut rs);
                    drop(rs);
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

fn radio_group(
    ui: &mut egui::Ui,
    ctx: &RadioUiCtx<'_>,
    rs: &mut RadioState,
    i: usize,
    actions: &mut Vec<RadioAction>,
) {
    let current = rs.dcs.is_current();
    let Some(radio) = rs.dcs.radios.get(i).cloned() else {
        return;
    };
    if !current || radio.modulation == Modulation::Disabled {
        ui.horizontal(|ui| {
            rw::indicator(ui, rw::RED, 12.0);
            ui.label(egui::RichText::new("No Radio").color(rw::WHITE));
        });
        ui.label(egui::RichText::new("---").monospace().color(rw::RX_GREEN));
        return;
    }
    let (rx, secondary, sent_by) = rw::receiving(rs, i);
    ui.horizontal(|ui| {
        if rw::indicator(ui, rw::indicator_color(rs, i, false), 12.0) {
            rw::select(rs, i);
        }
        ui.label(egui::RichText::new(&radio.name).small().color(rw::WHITE));
        let tuned = rw::tuned_count(ctx, rs, &radio);
        if tuned > 0 {
            ui.label(egui::RichText::new(format!("👤{tuned}")).small().color(rw::WHITE));
        }
    });
    let text = if rx && !sent_by.is_empty() {
        egui::RichText::new(sent_by)
            .monospace()
            .color(if secondary { rw::RED } else { rw::WHITE })
    } else {
        egui::RichText::new(rw::freq_text(&radio))
            .monospace()
            .color(rw::RX_GREEN)
    };
    rw::freq_label(ui, rs, i, text);
    rw::frequency_buttons(ui, rs, i, ctx.rotary_style);
    ui.horizontal(|ui| {
        rw::volume_slider(ui, rs, i, true);
        rw::rt_button(ui, rs, i, ctx.node_limit);
    });
    if radio.freq_mode == FreqMode::Overlay {
        let channels = ctx.net.fixed_channels.lock().get(i).cloned().unwrap_or_default();
        rw::channel_controls(ui, rs, i, &channels, actions);
    }
    rw::encryption_controls(ui, rs, i);
}

fn intercom_group(ui: &mut egui::Ui, rs: &mut RadioState) {
    let is_intercom = rs.dcs.is_current()
        && rs
            .dcs
            .radios
            .first()
            .is_some_and(|r| r.modulation == Modulation::Intercom);
    ui.horizontal(|ui| {
        let (rx, _, _) = rw::receiving(rs, 0);
        let color = if !is_intercom {
            rw::RED
        } else if rx {
            rw::GREEN_TX
        } else {
            rw::indicator_color(rs, 0, false)
        };
        if rw::indicator(ui, color, 12.0) && is_intercom {
            rw::select(rs, 0);
        }
        ui.label(egui::RichText::new(if is_intercom { "Intercom" } else { "No Intercom" }).color(rw::WHITE));
        if is_intercom {
            rw::volume_slider(ui, rs, 0, true);
        }
    });
}
