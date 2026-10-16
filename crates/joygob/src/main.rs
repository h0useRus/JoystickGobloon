#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod action_editor;
mod app;
mod condition_editor;
mod devices_view;
mod hidhide_view;
mod key_capture;
mod logfile;
mod macro_editor;
mod modes_view;
mod profile_view;
mod settings;
mod settings_view;
mod style;
mod substitute_view;
mod vjoy_view;

pub const APP_NAME: &str = "Joystick Gobloon";

fn main() -> eframe::Result {
    logfile::init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
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
        Box::new(|cc| Ok(Box::new(app::JoyGobApp::new(cc)))),
    )
}
