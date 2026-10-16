#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod action_editor;
mod app;
mod condition_editor;
mod curve_editor;
mod devices_view;
mod hidhide_view;
mod instance;
mod key_capture;
mod logfile;
mod macro_editor;
mod modes_view;
mod profile_view;
mod settings;
mod settings_view;
mod style;
mod substitute_view;
mod tray;
mod viewer_view;
mod vjoy_config_view;
mod vjoy_view;

pub const APP_NAME: &str = "Joystick Gobloon";
/// Version shown to users: CI sets `JOYGOB_VERSION` for alpha builds (`0.2.0-alpha.12`),
/// otherwise the crate version.
pub const VERSION: &str = match option_env!("JOYGOB_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

fn main() -> eframe::Result {
    // Already running: that copy was woken and comes to the front; this one quits.
    let Some(instance) = instance::claim() else {
        return Ok(());
    };
    logfile::init();
    // Started by Windows at logon: hidden in the tray, or minimized without one.
    let minimized = std::env::args().any(|a| a == tray::autostart::ARG);
    let to_tray = minimized && settings::Settings::load().show_tray;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_visible(!to_tray)
            .with_title(APP_NAME)
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([640.0, 400.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../assets/icon/joygob-256.png"
                ))
                .expect("embedded icon is a valid PNG"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::JoyGobApp::new(cc, instance)))),
    )
}
