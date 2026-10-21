//! "Night cockpit" theme: dark blue-grey panels, amber accent, B612 fonts
//! (the Airbus cockpit display typeface, SIL OFL 1.1) and a few drawing
//! helpers (LEDs, readouts, cards, pills, VU meter, tab bar). Rendering only:
//! no widget here changes what a click does.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle};

pub mod palette {
    use eframe::egui::Color32;

    pub const BG: Color32 = Color32::from_rgb(0x0F, 0x12, 0x15);
    pub const PANEL: Color32 = Color32::from_rgb(0x16, 0x1B, 0x20);
    pub const CARD: Color32 = Color32::from_rgb(0x1C, 0x23, 0x29);
    pub const RAISED: Color32 = Color32::from_rgb(0x25, 0x2E, 0x36);
    pub const HOVER: Color32 = Color32::from_rgb(0x2F, 0x3A, 0x44);
    pub const INSET: Color32 = Color32::from_rgb(0x08, 0x0A, 0x0C);
    pub const STROKE: Color32 = Color32::from_rgb(0x2A, 0x33, 0x3C);
    pub const STROKE_STRONG: Color32 = Color32::from_rgb(0x3D, 0x49, 0x55);
    pub const TEXT: Color32 = Color32::from_rgb(0xD6, 0xDC, 0xE1);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(0x87, 0x92, 0xA0);
    pub const AMBER: Color32 = Color32::from_rgb(0xFF, 0xB0, 0x00);
    /// Amber-tinted dark fill for pressed widgets.
    pub const AMBER_DARK: Color32 = Color32::from_rgb(0x3A, 0x2C, 0x0A);

    // SRS semantics (C# `RadioOverlayWindow` colours), softened for dark panels.
    pub const GREEN_TX: Color32 = Color32::from_rgb(0x9C, 0xFF, 0x7A);
    pub const GREEN: Color32 = Color32::from_rgb(0x2F, 0xA8, 0x4F);
    pub const RX_GREEN: Color32 = Color32::from_rgb(0x35, 0xE8, 0x6B);
    pub const ORANGE: Color32 = Color32::from_rgb(0xFF, 0xA5, 0x00);
    pub const DARK_BLUE: Color32 = Color32::from_rgb(0x3B, 0x63, 0xC4);
    pub const SIMUL_BLUE: Color32 = Color32::from_rgb(0x4F, 0x86, 0xFF);
    pub const RED: Color32 = Color32::from_rgb(0xE0, 0x45, 0x45);
    pub const WHITE: Color32 = Color32::from_rgb(0xF2, 0xF4, 0xF6);
    pub const GRAY: Color32 = Color32::from_rgb(0x6B, 0x75, 0x80);
    pub const COALITION_RED: Color32 = Color32::from_rgb(0xDC, 0x3C, 0x3C);
    pub const COALITION_BLUE: Color32 = Color32::from_rgb(0x4C, 0x8A, 0xFF);
}

use palette as p;

const B612_REGULAR: &[u8] = include_bytes!("../../assets/fonts/B612-Regular.ttf");
const B612_BOLD: &[u8] = include_bytes!("../../assets/fonts/B612-Bold.ttf");
const B612_MONO: &[u8] = include_bytes!("../../assets/fonts/B612Mono-Regular.ttf");

/// Bold family name (egui has no weight axis, so bold is its own family).
pub fn bold() -> FontFamily {
    FontFamily::Name("B612-Bold".into())
}

/// Install fonts and the dark style once. Dark is the only theme: the
/// `DarkMode` key stays in `global.cfg` for the C# client but is ignored here.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "B612".into(),
        std::sync::Arc::new(egui::FontData::from_static(B612_REGULAR)),
    );
    fonts.font_data.insert(
        "B612-Bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(B612_BOLD)),
    );
    fonts.font_data.insert(
        "B612Mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(B612_MONO)),
    );
    // B612 first, egui's own fonts stay as fallbacks (emoji, icons).
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "B612".into());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "B612Mono".into());
    let mut bold_family: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    bold_family.insert(0, "B612-Bold".into());
    fonts.families.insert(bold(), bold_family);
    ctx.set_fonts(fonts);

    let mut style = egui::Style {
        text_styles: [
            (TextStyle::Heading, FontId::new(18.0, bold())),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        ]
        .into(),
        visuals: visuals(),
        ..Default::default()
    };
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size = egui::vec2(40.0, 22.0);
    style.spacing.slider_width = 140.0;
    style.spacing.slider_rail_height = 6.0;
    style.spacing.combo_width = 120.0;
    style.spacing.indent = 14.0;
    style.spacing.icon_width = 16.0;
    style.spacing.icon_width_inner = 9.0;
    style.spacing.window_margin = Margin::same(10);
    style.spacing.menu_margin = Margin::same(6);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_style_of(egui::Theme::Dark, style);
}

fn visuals() -> egui::Visuals {
    use egui::style::{HandleShape, Selection, WidgetVisuals, Widgets};
    let cr = CornerRadius::same(3);
    let widget = |bg: Color32, stroke: Color32, fg: Color32, fg_w: f32| WidgetVisuals {
        weak_bg_fill: bg,
        bg_fill: bg,
        bg_stroke: Stroke::new(1.0, stroke),
        fg_stroke: Stroke::new(fg_w, fg),
        corner_radius: cr,
        expansion: 0.0,
    };
    let mut v = egui::Visuals::dark();
    v.panel_fill = p::PANEL;
    v.window_fill = p::PANEL;
    v.extreme_bg_color = p::INSET;
    v.faint_bg_color = Color32::from_rgb(0x1A, 0x20, 0x26);
    v.code_bg_color = p::INSET;
    v.weak_text_color = Some(p::TEXT_DIM);
    v.hyperlink_color = p::AMBER;
    v.warn_fg_color = p::ORANGE;
    v.error_fg_color = p::RED;
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v.window_stroke = Stroke::new(1.0, p::STROKE_STRONG);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 3],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    v.selection = Selection {
        bg_fill: Color32::from_rgba_unmultiplied(0xFF, 0xB0, 0x00, 0x48),
        stroke: Stroke::new(1.0, p::AMBER),
    };
    v.widgets = Widgets {
        noninteractive: widget(p::PANEL, p::STROKE, p::TEXT, 1.0),
        inactive: widget(p::RAISED, p::STROKE_STRONG, p::TEXT, 1.0),
        hovered: widget(p::HOVER, p::AMBER, p::WHITE, 1.5),
        active: widget(p::AMBER_DARK, p::AMBER, p::AMBER, 2.0),
        open: widget(p::CARD, p::STROKE_STRONG, p::TEXT, 1.0),
    };
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p::STROKE);
    v.slider_trailing_fill = true;
    v.handle_shape = HandleShape::Rect { aspect_ratio: 0.6 };
    v.collapsing_header_frame = false;
    v.striped = false;
    v.text_cursor.stroke = Stroke::new(2.0, p::AMBER);
    v
}

/// Frame of the two overlays (semi-transparent over the game).
pub fn overlay_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(0x0C, 0x0E, 0x11, 0xE1))
        .stroke(Stroke::new(1.0, p::STROKE_STRONG))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::same(6))
}

/// Status LED: core + soft halo. Clickable.
pub fn led(ui: &mut egui::Ui, color: Color32, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let c = rect.center();
        let r = size / 2.0;
        let painter = ui.painter();
        painter.circle_filled(c, r, color.gamma_multiply(0.28));
        painter.circle_filled(c, r * 0.62, color);
        painter.circle_stroke(c, r * 0.62, Stroke::new(1.0, Color32::from_black_alpha(150)));
    }
    resp
}

/// Instrument readout: inset dark box with monospace text. Whole box clickable.
pub fn readout(ui: &mut egui::Ui, text: egui::RichText) -> egui::Response {
    let inner = egui::Frame::NONE
        .fill(p::INSET)
        .stroke(Stroke::new(1.0, p::STROKE_STRONG))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().min(160.0));
            ui.add(
                egui::Label::new(text.monospace())
                    .selectable(false)
                    .sense(egui::Sense::click()),
            )
        });
    let frame_resp = ui.interact(
        inner.response.rect,
        inner.response.id.with("readout"),
        egui::Sense::click(),
    );
    inner.inner | frame_resp
}

fn card_frame(fill: Color32, margin: i8) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, p::STROKE))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::same(margin))
}

/// Full-width section card with an optional small-caps title.
pub fn card<R>(ui: &mut egui::Ui, title: Option<&str>, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_frame(p::CARD, 10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(t) = title {
                ui.label(
                    egui::RichText::new(t.to_uppercase())
                        .family(bold())
                        .size(10.5)
                        .color(p::TEXT_DIM),
                );
                ui.add_space(4.0);
            }
            add_contents(ui)
        })
        .inner
}

/// Compact card for overlay radio groups (no forced width, tighter margin).
pub fn tile<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_frame(Color32::from_rgba_unmultiplied(0x1C, 0x23, 0x29, 0xD0), 5)
        .show(ui, add_contents)
        .inner
}

/// Inset box (log, read-only text areas).
pub fn inset<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(p::INSET)
        .stroke(Stroke::new(1.0, p::STROKE))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

/// Amber call-to-action button.
pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).family(bold()).color(p::INSET))
            .fill(p::AMBER)
            .stroke(Stroke::NONE)
            .min_size(egui::vec2(0.0, 26.0)),
    )
}

/// Dark red button (disconnect, delete).
pub fn danger_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(p::TEXT))
            .fill(Color32::from_rgb(0x6E, 0x22, 0x22))
            .stroke(Stroke::new(1.0, p::RED))
            .min_size(egui::vec2(0.0, 26.0)),
    )
}

/// Borderless icon button (overlay title row).
pub fn ghost_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(text).small().color(p::TEXT_DIM)).frame(false))
}

/// Small status badge.
pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::NONE
        .fill(color.gamma_multiply(0.18))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.7)))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).family(bold()).size(10.5).color(color));
        });
}

/// Segmented level meter, `level` in 0..=1 (green, amber above 0.6, red above 0.85).
pub fn vu_meter(ui: &mut egui::Ui, level: f32, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(2), p::INSET);
    painter.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, p::STROKE),
        egui::StrokeKind::Inside,
    );
    let segments = 24usize;
    let gap = 2.0;
    let inner = rect.shrink(3.0);
    let seg_w = (inner.width() - gap * (segments as f32 - 1.0)) / segments as f32;
    let level = level.clamp(0.0, 1.0);
    for i in 0..segments {
        let t = (i as f32 + 0.5) / segments as f32;
        let color = if t > 0.85 {
            p::RED
        } else if t > 0.6 {
            p::AMBER
        } else {
            p::RX_GREEN
        };
        let lit = t <= level;
        let x = inner.left() + i as f32 * (seg_w + gap);
        let seg = egui::Rect::from_min_size(egui::pos2(x, inner.top()), egui::vec2(seg_w, inner.height()));
        painter.rect_filled(
            seg,
            CornerRadius::ZERO,
            if lit { color } else { color.gamma_multiply(0.16) },
        );
    }
}

/// Underlined tab strip. Returns true when the selection changed.
pub fn tab_bar<T: Copy + PartialEq>(ui: &mut egui::Ui, current: &mut T, tabs: &[(T, &str)]) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (tab, label) in tabs {
            let selected = *tab == *current;
            let color = if selected { p::AMBER } else { p::TEXT_DIM };
            let resp = ui.add(
                egui::Button::new(egui::RichText::new(*label).family(bold()).size(13.0).color(color))
                    .frame(false)
                    .min_size(egui::vec2(0.0, 24.0)),
            );
            if selected {
                let r = resp.rect;
                ui.painter()
                    .hline(r.x_range(), r.bottom() + 1.0, Stroke::new(2.0, p::AMBER));
            }
            if resp.clicked() && !selected {
                *current = *tab;
                changed = true;
            }
        }
    });
    changed
}
