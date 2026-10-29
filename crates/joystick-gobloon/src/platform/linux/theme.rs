//! The desktop's light/dark preference: the XDG desktop portal's `color-scheme`, else the GTK
//! theme (`GTK_THEME`, GTK's settings.ini, GNOME's gsettings) by a name ending in `-dark`.
//! winit cannot tell it on X11, so egui would start dark there.

use std::sync::mpsc;
use std::time::Duration;

use ashpd::desktop::settings::{ColorScheme, Settings};
use futures_util::StreamExt;

/// A missing portal can make D-Bus wait for its activation; the window does not wait that long.
const PORTAL_TIMEOUT: Duration = Duration::from_millis(500);

pub(super) fn dark_mode() -> Option<bool> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let scheme =
            pollster::block_on(async { Settings::new().await.ok()?.color_scheme().await.ok() });
        let _ = tx.send(scheme);
    });
    let portal = rx.recv_timeout(PORTAL_TIMEOUT).ok().flatten();
    portal.and_then(dark).or_else(gtk)
}

/// Calls `changed` from a thread of its own each time the portal reports a new color scheme.
pub(super) fn watch(changed: Box<dyn Fn(Option<bool>) + Send>) {
    std::thread::spawn(move || {
        pollster::block_on(async {
            let Ok(settings) = Settings::new().await else {
                return;
            };
            let Ok(mut schemes) = settings.receive_color_scheme_changed().await else {
                return;
            };
            while let Some(scheme) = schemes.next().await {
                changed(dark(scheme).or_else(gtk));
            }
        });
    });
}

fn dark(scheme: ColorScheme) -> Option<bool> {
    match scheme {
        ColorScheme::PreferDark => Some(true),
        ColorScheme::PreferLight => Some(false),
        ColorScheme::NoPreference => None,
    }
}

/// From the GTK theme: `Some(true)` for a dark one, `Some(false)` for any other named theme.
fn gtk() -> Option<bool> {
    if let Ok(theme) = std::env::var("GTK_THEME")
        && !theme.is_empty()
    {
        return Some(dark_name(&theme));
    }
    for version in ["gtk-4.0", "gtk-3.0"] {
        let Some(text) = super::config_home()
            .and_then(|d| std::fs::read_to_string(d.join(version).join("settings.ini")).ok())
        else {
            continue;
        };
        let prefer = super::ini_value(&text, "Settings", "gtk-application-prefer-dark-theme");
        if matches!(prefer, Some("1" | "true")) {
            return Some(true);
        }
        if let Some(name) = super::ini_value(&text, "Settings", "gtk-theme-name") {
            return Some(dark_name(name));
        }
    }
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
        .ok()?;
    // "'Adwaita-dark'"
    let name = String::from_utf8(out.stdout).ok()?;
    let name = name.trim().trim_matches('\'');
    (!name.is_empty()).then(|| dark_name(name))
}

/// `Adwaita-dark`, `Adwaita:dark`.
fn dark_name(theme: &str) -> bool {
    let theme = theme.to_ascii_lowercase();
    theme.ends_with("-dark") || theme.ends_with(":dark")
}

#[cfg(test)]
mod tests {
    use super::dark_name;

    #[test]
    fn dark_theme_names() {
        assert!(dark_name("Adwaita-dark"));
        assert!(dark_name("Adwaita:dark"));
        assert!(dark_name("Breeze-Dark"));
        assert!(!dark_name("Adwaita"));
        assert!(!dark_name("Darkly-light"));
    }
}
