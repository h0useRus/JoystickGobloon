//! Map Gobloon: shows a picture of a game controller and lets the user drag the buttons, axes and
//! hats the connected controller sends onto its controls. The result is a device plugin
//! (gob-devices) that Joystick Gobloon reads; plugins can be shared with other players.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod dialogs;
mod glyphs;
mod panel;
mod platform;
mod settings;
mod store;
#[cfg(test)]
mod ui_tests;

pub const APP_NAME: &str = "Map Gobloon";
/// The window's app id (Wayland) and class (X11).
pub const APP_ID: &str = "map-gobloon";
/// Version shown to users: the same as Joystick Gobloon's (build.rs).
pub const VERSION: &str = env!("JOYSTICK_GOBLOON_VERSION");

/// The built-in UI languages; more `lang/map-gobloon/*.ftl` load from the exe's folder.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", include_str!("../lang/en.ftl")),
    ("ru", include_str!("../lang/ru.ftl")),
    ("de", include_str!("../lang/de.ftl")),
    ("es", include_str!("../lang/es.ftl")),
    ("fr", include_str!("../lang/fr.ftl")),
    ("ko", include_str!("../lang/ko.ftl")),
    ("zh", include_str!("../lang/zh.ftl")),
];

/// Command-line switch for the uninstaller: take this exe off the device hiding's list of
/// programs and quit without a window.
const HIDHIDE_UNREGISTER: &str = "--hidhide-unregister";

fn main() -> eframe::Result {
    if std::env::args().any(|a| a == HIDHIDE_UNREGISTER) {
        if let (Ok(hiding), Ok(exe)) = (gob_hide::load(), std::env::current_exe()) {
            let _ = hiding.set_app(&exe, false);
        }
        return Ok(());
    }
    // Device hiding: hidden controllers are read only by listed programs, and the driver
    // decides that when a program starts. Map Gobloon lists itself like Joystick Gobloon and,
    // the first time, starts again to see them.
    if list_in_device_hiding()
        && let Ok(exe) = std::env::current_exe()
        && std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .spawn()
            .is_ok()
    {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(format!("{APP_NAME} {VERSION}"))
            .with_app_id(APP_ID)
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
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
        Box::new(|cc| Ok(Box::new(app::MapApp::new(cc)))),
    )
}

/// Adds this program to the device hiding's list of programs that see hidden devices, unless
/// it is there already or the list is inverted. Returns whether it was added.
fn list_in_device_hiding() -> bool {
    if !gob_hide::capabilities().app_list {
        return false;
    }
    let Ok(hiding) = gob_hide::load() else {
        return false;
    };
    if hiding.apps_inverted().unwrap_or(true) {
        return false;
    }
    std::env::current_exe()
        .ok()
        .is_some_and(|exe| hiding.ensure_app(&exe).unwrap_or(false))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_language_has_every_english_key() {
        assert_eq!(gob_i18n::check_complete(super::LANGUAGES), Ok(()));
    }

    /// Every key the code passes to `tr` or `trf` as a string literal is in en.ftl.
    #[test]
    fn english_has_every_key_the_code_uses() {
        let en = super::LANGUAGES[0].1;
        let defined: std::collections::HashSet<&str> = en
            .lines()
            .filter_map(|l| l.split_once(" ="))
            .map(|(k, _)| k.trim())
            .collect();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(src).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for call in [".tr(\"", ".trf(\""] {
                for (at, _) in text.match_indices(call) {
                    let rest = &text[at + call.len()..];
                    let key = &rest[..rest.find('"').unwrap()];
                    if !defined.contains(key) {
                        missing.push(format!("{}: {key}", path.display()));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "keys missing in en.ftl: {missing:#?}");
    }
}
