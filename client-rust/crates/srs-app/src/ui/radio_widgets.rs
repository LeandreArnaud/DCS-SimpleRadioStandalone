//! Radio widgets shared by the single radio overlay and the AWACS overlay
//! (`RadioControlGroup`, `AwacsRadioControlGroup`, `IntercomControlGroup`,
//! `TransponderPanel`, `PresetChannelsView`).

use eframe::egui;
use srs_client::{Client, RadioState};
use srs_dcs::radio_helper as rh;
use srs_dcs::{
    DcsRadio, EncryptionMode, FreqMode, RetransmitMode, SwitchControls, VolumeMode, UNIT_ID_OFFSET,
};
use srs_protocol::{Modulation, Transponder};

pub const GREEN_TX: egui::Color32 = egui::Color32::from_rgb(0x96, 0xFF, 0x6D);
pub const GREEN: egui::Color32 = egui::Color32::from_rgb(0x00, 0x80, 0x00);
pub const RX_GREEN: egui::Color32 = egui::Color32::from_rgb(0x00, 0xFF, 0x00);
pub const ORANGE: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xA5, 0x00);
pub const DARK_BLUE: egui::Color32 = egui::Color32::from_rgb(0x00, 0x00, 0x8B);
pub const SIMUL_BLUE: egui::Color32 = egui::Color32::from_rgb(0x4F, 0x86, 0xFF);
pub const RED: egui::Color32 = egui::Color32::from_rgb(0xE0, 0x30, 0x30);
pub const WHITE: egui::Color32 = egui::Color32::WHITE;

/// Per-frame read-only context for the radio widgets.
pub struct RadioUiCtx<'a> {
    pub net: &'a Client,
    pub my_guid: &'a str,
    pub connected: bool,
    pub rotary_style: bool,
    /// Server `RETRANSMISSION_NODE_LIMIT`.
    pub node_limit: i32,
}

/// Things the widgets cannot do by themselves.
#[derive(Debug, Clone, PartialEq)]
pub enum RadioAction {
    ReloadChannels(usize),
    StartInstructor { radio: usize, unit_id: u32 },
    StopInstructor(usize),
}

/// `IsReceiving` within 350 ms + the transmitter name.
pub fn receiving(rs: &RadioState, i: usize) -> (bool, bool, String) {
    match rs.receiving.get(i) {
        Some(r) if r.is_receiving() => (true, r.is_secondary, r.sent_by.clone()),
        _ => (false, false, String::new()),
    }
}

/// Indicator colour of a radio (`RepaintRadioStatus`).
pub fn indicator_color(rs: &RadioState, i: usize, awacs: bool) -> egui::Color32 {
    let info = &rs.dcs;
    let selected = info.selected_index() == Some(i);
    let sending = rs.sending.is_sending;
    let sending_on = rs.sending.sending_on;
    let radio = &info.radios[i];
    if awacs && sending {
        if sending_on == Some(i) {
            return GREEN_TX;
        }
        if radio.simul {
            return SIMUL_BLUE;
        }
    }
    if selected {
        if sending && sending_on == Some(i) {
            GREEN_TX
        } else {
            GREEN
        }
    } else if radio.simul && info.simultaneous_transmission {
        DARK_BLUE
    } else {
        ORANGE
    }
}

fn modulation_suffix(m: Modulation) -> &'static str {
    match m {
        Modulation::Am => "AM",
        Modulation::Fm => "FM",
        Modulation::Sincgars => "SG",
        Modulation::HaveQuick => "HQ",
        Modulation::Satcom => "SATCOM",
        Modulation::Mids => "MIDS",
        Modulation::Intercom => "INT",
        Modulation::Disabled => "",
    }
}

/// Single overlay frequency text (`RepaintRadioStatus`).
pub fn freq_text(radio: &DcsRadio) -> String {
    match radio.modulation {
        Modulation::Intercom => "Intercom".to_string(),
        Modulation::Mids => {
            let ch = srs_protocol::radio_calc::link16::frequency_to_channel(radio.freq);
            if ch > 0 {
                format!("{ch} MIDS")
            } else {
                "OFF".to_string()
            }
        }
        m => {
            let mut s = format!("{:.3} {}", radio.freq / 1e6, modulation_suffix(m));
            s.push_str(&metadata_suffix(radio));
            s
        }
    }
}

/// ` G`, ` C{n}`, ` E{key}`, ` RX` suffixes.
pub fn metadata_suffix(radio: &DcsRadio) -> String {
    let mut s = String::new();
    if radio.sec_freq > 100.0 {
        s.push_str(" G");
    }
    if radio.channel >= 0 {
        s.push_str(&format!(" C{}", radio.channel));
    }
    if radio.enc && radio.enc_key > 0 {
        s.push_str(&format!(" E{}", radio.enc_key));
    }
    if radio.rx_only {
        s.push_str(" RX");
    }
    s
}

/// AWACS metadata text (modulation + suffixes without RX).
pub fn awacs_metadata(radio: &DcsRadio, tuned: i32) -> String {
    let mut s = modulation_suffix(radio.modulation).to_string();
    if radio.sec_freq > 100.0 {
        s.push_str(" G");
    }
    if radio.channel > -1 {
        s.push_str(&format!(" C{}", radio.channel));
    }
    if radio.enc && radio.enc_key > 0 {
        s.push_str(&format!(" E{}", radio.enc_key));
    }
    if tuned > 0 {
        s.push_str(&format!(" 👤{tuned}"));
    }
    s
}

/// `ClientsOnFreq` (0 unless the server shows tuned counts).
pub fn tuned_count(ctx: &RadioUiCtx<'_>, rs: &RadioState, radio: &DcsRadio) -> i32 {
    srs_client::combined_state::clients_on_freq(
        &ctx.net.shared,
        rs,
        ctx.my_guid,
        radio.freq,
        radio.modulation,
    )
}

/// Clickable indicator circle. Returns true when clicked.
pub fn indicator(ui: &mut egui::Ui, color: egui::Color32, size: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    ui.painter().circle_filled(rect.center(), size / 2.0, color);
    ui.painter().circle_stroke(
        rect.center(),
        size / 2.0,
        egui::Stroke::new(1.0, egui::Color32::BLACK),
    );
    resp.clicked()
}

/// Left click = select, right click = toggle guard (both also select).
pub fn freq_label(ui: &mut egui::Ui, rs: &mut RadioState, i: usize, text: egui::RichText) {
    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
    if resp.secondary_clicked() && rh::toggle_guard(&mut rs.dcs, i) {
        rs.force_sync = true;
    }
    if resp.clicked() || resp.secondary_clicked() {
        select(rs, i);
    }
}

pub fn select(rs: &mut RadioState, i: usize) {
    if rs.dcs.control == SwitchControls::Hotas && rh::select_radio(&mut rs.dcs, i) {
        rs.force_sync = true;
    }
}

/// ±10 / ±1 / ±0.1 / ±0.01 / ±0.001 MHz buttons (MIDS: first three).
pub fn frequency_buttons(ui: &mut egui::Ui, rs: &mut RadioState, i: usize, rotary: bool) {
    let radio = &rs.dcs.radios[i];
    if radio.freq_mode != FreqMode::Overlay {
        return;
    }
    let steps: &[(f64, &str)] = if radio.modulation == Modulation::Mids {
        &[(10.0, "10"), (1.0, "1"), (0.1, ".1")]
    } else {
        &[
            (10.0, "10"),
            (1.0, "1"),
            (0.1, ".1"),
            (0.01, ".01"),
            (0.001, ".001"),
        ]
    };
    let mut delta = None;
    ui.horizontal(|ui| {
        for (d, label) in steps {
            if ui.small_button(format!("+{label}")).clicked() {
                delta = Some(*d);
            }
        }
    });
    ui.horizontal(|ui| {
        for (d, label) in steps {
            if ui.small_button(format!("-{label}")).clicked() {
                delta = Some(-*d);
            }
        }
    });
    if let Some(d) = delta {
        let (changed, _) = rh::update_frequency(&mut rs.dcs, d, i, true, true, rotary);
        if changed {
            rs.force_sync = true;
        }
    }
}

/// Volume slider 0..100, enabled only with overlay volume control.
pub fn volume_slider(ui: &mut egui::Ui, rs: &mut RadioState, i: usize, enabled: bool) {
    let radio = &mut rs.dcs.radios[i];
    let editable = enabled && radio.vol_mode == VolumeMode::Overlay;
    let mut v = radio.volume * 100.0;
    let resp = ui.add_enabled(editable, egui::Slider::new(&mut v, 0.0..=100.0).show_value(false));
    if resp.changed() {
        radio.volume = v / 100.0;
    }
}

/// RT (retransmit) button.
pub fn rt_button(ui: &mut egui::Ui, rs: &mut RadioState, i: usize, node_limit: i32) {
    let radio = &rs.dcs.radios[i];
    if node_limit <= 0 || radio.rt_mode == RetransmitMode::Disabled {
        return;
    }
    let color = if radio.retransmit { RED } else { WHITE };
    let enabled = radio.rt_mode != RetransmitMode::Cockpit;
    if ui
        .add_enabled(enabled, egui::Button::new(egui::RichText::new("RT").color(color)))
        .clicked()
        && rh::toggle_retransmit(&mut rs.dcs, i)
    {
        rs.force_sync = true;
    }
}

/// Encryption key spinner + enable/disable button (encryption tab).
pub fn encryption_controls(ui: &mut egui::Ui, rs: &mut RadioState, i: usize) {
    let radio = &rs.dcs.radios[i];
    if radio.modulation == Modulation::Intercom
        || matches!(
            radio.enc_mode,
            EncryptionMode::NoEncryption | EncryptionMode::EncryptionFull
        )
    {
        return;
    }
    let spinner_enabled = matches!(
        radio.enc_mode,
        EncryptionMode::EncryptionJustOverlay | EncryptionMode::EncryptionCockpitToggleOverlayCode
    );
    let button_enabled = radio.enc_mode == EncryptionMode::EncryptionJustOverlay;
    let mut key = i32::from(radio.enc_key).clamp(1, 252);
    let enc = radio.enc;
    ui.horizontal(|ui| {
        ui.label("Key");
        if ui
            .add_enabled(spinner_enabled, egui::DragValue::new(&mut key).range(1..=252))
            .changed()
            && rh::set_encryption_key(&mut rs.dcs, i, key)
        {
            rs.force_sync = true;
        }
        let text = if enc { "Disable" } else { "Enable" };
        if ui.add_enabled(button_enabled, egui::Button::new(text)).clicked()
            && rs.dcs.radios[i].modulation != Modulation::Mids
            && rh::toggle_encryption(&mut rs.dcs, i)
        {
            rs.force_sync = true;
        }
    });
}

/// Preset channel dropdown + reload (`PresetChannelsView`).
pub fn channel_controls(
    ui: &mut egui::Ui,
    rs: &mut RadioState,
    i: usize,
    channels: &[srs_dcs::radio_helper::PresetChannel],
    actions: &mut Vec<RadioAction>,
) {
    if rs.dcs.radios[i].freq_mode != FreqMode::Overlay {
        return;
    }
    let current = rs.dcs.radios[i].channel;
    let label = channels
        .iter()
        .find(|c| c.channel == current)
        .map(|c| c.text.clone())
        .unwrap_or_else(|| {
            if channels.is_empty() {
                "No presets".to_string()
            } else {
                "Channel".to_string()
            }
        });
    ui.horizontal(|ui| {
        let mut picked: Option<usize> = None;
        egui::ComboBox::from_id_salt(("chan", i))
            .selected_text(label)
            .width(110.0)
            .show_ui(ui, |ui| {
                for (idx, c) in channels.iter().enumerate() {
                    if ui.selectable_label(c.channel == current, &c.text).clicked() {
                        picked = Some(idx);
                    }
                }
            });
        if let Some(idx) = picked {
            if rh::select_channel(&mut rs.dcs, i, &channels[idx]) {
                rs.force_sync = true;
            }
        }
        if ui.small_button("⟳").on_hover_text("Reload presets").clicked() {
            actions.push(RadioAction::ReloadChannels(i));
        }
    });
}

/// Intercom channel spinner (AWACS, 1..30).
pub fn intercom_spinner(ui: &mut egui::Ui, rs: &mut RadioState) {
    let enabled = rs
        .dcs
        .radios
        .first()
        .is_some_and(|r| r.modulation == Modulation::Intercom)
        && rs.dcs.unit_id >= UNIT_ID_OFFSET;
    let mut n = if enabled {
        rs.intercom_offset.max(1) as i32
    } else {
        1
    };
    ui.horizontal(|ui| {
        ui.label("Intercom #");
        if ui
            .add_enabled(enabled, egui::DragValue::new(&mut n).range(1..=30))
            .changed()
        {
            if let Some(c) = rh::set_intercom(&mut rs.dcs, n as u32) {
                rs.intercom_offset = c;
                rs.force_sync = true;
            }
        }
    });
}

/// `TransponderPanel`.
pub fn transponder_panel(ui: &mut egui::Ui, rs: &mut RadioState) {
    let info = &rs.dcs;
    let fitted = info.is_current() && info.iff.control != Transponder::CONTROL_DISABLED;
    let overlay = fitted && info.iff.control == Transponder::CONTROL_OVERLAY;
    let on = fitted && info.iff.status != Transponder::STATUS_OFF;
    let iff = info.iff.clone();
    ui.horizontal(|ui| {
        let color = if !fitted {
            egui::Color32::GRAY
        } else if on {
            GREEN_TX
        } else {
            RED
        };
        if indicator(ui, color, 14.0) && overlay && srs_dcs::transponder::toggle_power(&mut rs.dcs) {
            rs.force_sync = true;
        }
        ui.label(egui::RichText::new("IFF").strong().color(WHITE));
        let m1 = if on && iff.mode1 >= 0 {
            format!("{:02}", iff.mode1)
        } else {
            "--".into()
        };
        let m3 = if on && iff.mode3 >= 0 {
            format!("{:04}", iff.mode3)
        } else {
            "--".into()
        };
        text_code(ui, rs, "M1", &m1, 2, overlay && on, |info, v| {
            srs_dcs::transponder::set_mode1(info, v)
        });
        if iff.mode2 > -1 {
            ui.label(egui::RichText::new(format!("M2 {:04}", iff.mode2)).color(WHITE));
        }
        text_code(ui, rs, "M3", &m3, 4, overlay && on, |info, v| {
            srs_dcs::transponder::set_mode3(info, v)
        });
        let m4_color = if iff.mode4 { RX_GREEN } else { WHITE };
        if ui
            .add_enabled(
                overlay && on,
                egui::Button::new(egui::RichText::new("M4").color(m4_color)),
            )
            .clicked()
        {
            srs_dcs::transponder::mode4_toggle(&mut rs.dcs);
            rs.force_sync = true;
        }
        let ident_color = if iff.status == Transponder::STATUS_IDENT {
            RX_GREEN
        } else {
            WHITE
        };
        if ui
            .add_enabled(
                overlay && on,
                egui::Button::new(egui::RichText::new("IDENT").color(ident_color)),
            )
            .clicked()
            && srs_dcs::transponder::toggle_ident(&mut rs.dcs)
        {
            rs.force_sync = true;
        }
    });
}

/// Editable mode code: commit on Enter / focus loss; invalid → -1.
fn text_code(
    ui: &mut egui::Ui,
    rs: &mut RadioState,
    label: &str,
    shown: &str,
    max_len: usize,
    enabled: bool,
    set: impl Fn(&mut srs_dcs::DcsPlayerRadioInfo, i32) -> bool,
) {
    let id = ui.id().with(("iff", label));
    let mut text: String = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| shown.to_string());
    ui.label(egui::RichText::new(label).color(WHITE));
    let resp = ui.add_enabled(
        enabled,
        egui::TextEdit::singleline(&mut text)
            .desired_width(max_len as f32 * 10.0)
            .char_limit(max_len),
    );
    if resp.changed() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    let commit = resp.lost_focus() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
    if commit {
        let v = text.trim().parse::<i32>().unwrap_or(-1);
        if set(&mut rs.dcs, v) {
            rs.force_sync = true;
        }
        ui.data_mut(|d| d.remove::<String>(id));
    } else if !resp.has_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
    }
}

/// Apply a size scale to the fonts and spacing of this `Ui` subtree.
pub fn scale_ui(ui: &mut egui::Ui, scale: f32) {
    if (scale - 1.0).abs() < 0.01 {
        return;
    }
    let mut style: egui::Style = (**ui.style()).clone();
    for f in style.text_styles.values_mut() {
        f.size *= scale;
    }
    style.spacing.item_spacing *= scale;
    style.spacing.button_padding *= scale;
    style.spacing.interact_size *= scale;
    style.spacing.slider_width *= scale;
    ui.set_style(style);
}

/// The clone `InstructorModeViewModel` publishes for a radio.
pub fn instructor_radio(unit_id: u32) -> DcsRadio {
    DcsRadio {
        intercom_unit_id: unit_id,
        rt_mode: RetransmitMode::Disabled,
        modulation: Modulation::Intercom,
        freq_mode: FreqMode::Cockpit,
        guard_freq_mode: FreqMode::Cockpit,
        sec_freq: -1.0,
        vol_mode: VolumeMode::Overlay,
        freq: 10000.0,
        freq_min: 1000.0,
        freq_max: 100000.0,
        channel: -1,
        model: "INTERCOM".into(),
        name: "INTERCOM".into(),
        enc_mode: EncryptionMode::NoEncryption,
        rx_only: false,
        ..DcsRadio::default()
    }
}
