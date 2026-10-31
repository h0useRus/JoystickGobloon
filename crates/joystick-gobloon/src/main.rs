#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod action_editor;
mod activation;
mod app;
mod apps_view;
mod calibration_view;
mod condition_editor;
mod curve_editor;
mod device_access;
mod devices_view;
mod hidhide_view;
mod key_capture;
mod leds;
mod logfile;
mod macro_editor;
mod modes_view;
mod modifiers_view;
mod output_marks;
mod platform;
mod profile_view;
mod settings;
mod settings_view;
mod style;
mod substitute_view;
mod system_part_view;
mod tray;
mod update;
mod viewer_view;
mod virtual_setup;
mod vjoy_config_view;
mod vjoy_view;
mod wizard_view;

pub const APP_NAME: &str = "Joystick Gobloon";
/// The window's app id (Wayland) and class (X11); on Linux also the menu entry's name.
pub const APP_ID: &str = "joystick-gobloon";
/// Version shown to users, from build.rs: what CI sets in `JOYSTICK_GOBLOON_VERSION`
/// (`0.2.0-alpha.12`), else the latest git tag and the commits since it (`0.9.0-alpha.45+3.g728daf5`),
/// else the crate version.
pub const VERSION: &str = env!("JOYSTICK_GOBLOON_VERSION");

/// The built-in UI languages; more `lang/*.ftl` load from the exe's folder.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", include_str!("../../../lang/en.ftl")),
    ("ru", include_str!("../../../lang/ru.ftl")),
    ("de", include_str!("../../../lang/de.ftl")),
    ("es", include_str!("../../../lang/es.ftl")),
    ("fr", include_str!("../../../lang/fr.ftl")),
    ("ko", include_str!("../../../lang/ko.ftl")),
    ("zh", include_str!("../../../lang/zh.ftl")),
];

/// Command-line switch for the uninstaller: take this exe off the device hiding's application list
/// (Joystick Gobloon adds itself on start) and quit without a window.
const HIDHIDE_UNREGISTER: &str = "--hidhide-unregister";

fn main() -> eframe::Result {
    // Elevated copy started by the virtual device configuration: apply one change and quit,
    // before the single-instance check and without touching the log.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = gob_virtual::admin_entry(&args) {
        std::process::exit(code);
    }
    // Restarted by a portable update: let the old copy exit before the single-instance check.
    update::wait_for_previous();
    if std::env::args().any(|a| a == HIDHIDE_UNREGISTER) {
        if let (Ok(hh), Ok(exe)) = (gob_hide::load(), std::env::current_exe()) {
            let _ = hh.set_app(&exe, false);
        }
        return Ok(());
    }
    let settings = settings::Settings::load();
    if let Some(dir) = settings::data_dir() {
        gob_virtual::set_data_dir(dir);
        gob_hide::set_data_dir(dir);
    }
    // Already running: that copy was woken and comes to the front; this one quits.
    // The hidden `multi_instance` setting lets copies run side by side.
    let instance = if settings.multi_instance {
        None
    } else {
        match platform::claim_instance() {
            Some(event) => Some(event),
            None => return Ok(()),
        }
    };
    logfile::init();
    // Started by Windows at logon: hidden in the tray, or minimized without one.
    let minimized = std::env::args().any(|a| a == tray::autostart::ARG);
    let to_tray = minimized && settings.show_tray;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_visible(!to_tray)
            .with_title(APP_NAME)
            .with_app_id(APP_ID)
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([640.0, 400.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../assets/icon/JoystickGobloon-256.png"
                ))
                .expect("embedded icon is a valid PNG"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::JoystickGobloonApp::new(cc, instance)))),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_language_has_every_english_key() {
        assert_eq!(gob_i18n::check_complete(super::LANGUAGES), Ok(()));
    }

    #[test]
    fn languages_load() {
        let mut i18n = gob_i18n::I18n::new(super::LANGUAGES, None);
        i18n.set_language("ru");
        assert_eq!(i18n.tr("nav-settings"), "Настройки");
        assert_eq!(i18n.languages().len(), super::LANGUAGES.len());
    }
}
