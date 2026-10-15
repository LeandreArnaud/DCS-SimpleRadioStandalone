//! `SR-ClientRadio` — Rust client for DCS SimpleRadio Standalone.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod args;
mod logging;
mod single_instance;
mod ui;

use std::path::PathBuf;

/// Four-part version sent to the server in every TCP message.
pub const CLIENT_VERSION: &str = env!("SRS_CLIENT_VERSION");

fn main() -> eframe::Result<()> {
    logging::init();
    let args = args::Args::parse(std::env::args().skip(1));
    log::info!("SR-ClientRadio {CLIENT_VERSION} starting, args: {args:?}");

    // `-cfg=<dir>` or the working directory, like `GlobalSettingsStore`.
    let cfg_dir = args
        .cfg_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut settings = match srs_settings::GlobalSettings::load(&cfg_dir) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Cannot load settings from {}: {e}", cfg_dir.display());
            srs_settings::GlobalSettings::in_memory()
        }
    };

    // `App.IsClientRunning`: one instance unless `-allowMultiple` or the setting says so.
    let _instance = match single_instance::acquire() {
        Ok(g) => Some(g),
        Err(()) => {
            if args.allow_multiple
                || settings.get_bool(srs_settings::GlobalSettingsKey::AllowMultipleInstances)
            {
                log::warn!("Another SRS instance is already running, allowing multiple instances due to config setting");
                None
            } else {
                log::warn!("Another SRS instance is already running, preventing second instance startup");
                single_instance::show_already_running();
                return Ok(());
            }
        }
    };

    let client_x = settings.get_f64(srs_settings::GlobalSettingsKey::ClientX) as f32;
    let client_y = settings.get_f64(srs_settings::GlobalSettingsKey::ClientY) as f32;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("DCS-SRS Client - {CLIENT_VERSION}"))
            .with_inner_size([640.0, 700.0])
            .with_min_inner_size([400.0, 300.0])
            .with_position([client_x, client_y]),
        ..Default::default()
    };
    eframe::run_native(
        "DCS-SRS Client",
        options,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, args, settings)))),
    )
}
