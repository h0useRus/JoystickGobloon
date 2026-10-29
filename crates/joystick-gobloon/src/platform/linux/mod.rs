//! Linux desktop integration: XDG folders, a socket for the single instance, XDG autostart,
//! CJK fonts through fontconfig, light/dark from the desktop portal, the window and the tray
//! (GTK thread), device access (the udev
//! rule, gob-uinput's `access`), updates of the AppImage. The rest is shared with `none.rs`.

mod autostart;
mod fonts;
mod instance;
mod keys;
mod theme;
mod update;
mod window;

use std::path::{Path, PathBuf};
use std::time::Duration;

use gob_core::Key;

use super::none::Desktop as Fallback;

pub(crate) struct Desktop;

/// `$XDG_CONFIG_HOME`, by default `~/.config`.
fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// `key` in `[section]` of an INI-style file like `kdeglobals`.
fn ini_value<'a>(text: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = name == section;
        } else if inside
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim());
        }
    }
    None
}

impl super::Desktop for Desktop {
    type InstanceEvent = instance::Event;
    type Window = window::Window;

    /// `$XDG_CONFIG_HOME/JoystickGobloon`, where gob-virtual and gob-hide look without one too.
    fn default_data_dir() -> Option<PathBuf> {
        Some(config_home()?.join("JoystickGobloon"))
    }

    fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
        fonts::cjk()
    }

    /// In the user's editor for text files.
    fn open_text_file(path: &Path) {
        if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
            log::warn!("xdg-open {}: {e}", path.display());
        }
    }

    /// Programs have no extension here: any file.
    fn program_extensions() -> &'static [&'static str] {
        &[]
    }

    fn local_time() -> String {
        // SAFETY: plain libc calls into locally owned structs.
        unsafe {
            let mut now: libc::timespec = std::mem::zeroed();
            libc::clock_gettime(libc::CLOCK_REALTIME, &mut now);
            let mut t: libc::tm = std::mem::zeroed();
            libc::localtime_r(&now.tv_sec, &mut t);
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
                t.tm_year + 1900,
                t.tm_mon + 1,
                t.tm_mday,
                t.tm_hour,
                t.tm_min,
                t.tm_sec,
                now.tv_nsec / 1_000_000
            )
        }
    }

    fn claim_instance() -> Option<Self::InstanceEvent> {
        instance::claim()
    }

    fn listen_instance(event: Self::InstanceEvent, show: Box<dyn Fn() + Send>) {
        instance::listen(event, show);
    }

    /// Keyboards are read through evdev, which needs access the main udev rule leaves out (any
    /// program of the user could read what is typed): the user allows it on the Keyboard tab.
    fn keyboard_bindings() -> bool {
        true
    }

    fn keyboard_access() -> super::KeyboardAccess {
        use gob_uinput::access;
        if access::keyboard_rule_installed() {
            super::KeyboardAccess::Allowed
        } else if access::keyboards_readable() {
            super::KeyboardAccess::Open
        } else {
            super::KeyboardAccess::Missing
        }
    }

    fn set_keyboard_access(allow: bool) -> Result<(), String> {
        if allow {
            gob_uinput::access::install_keyboard()
        } else {
            gob_uinput::access::remove_keyboard()
        }
    }

    fn held_keys() -> Option<Vec<Key>> {
        keys::held()
    }

    fn key_name(key: Key) -> String {
        keys::name(key).unwrap_or_else(|| super::key_code(key))
    }

    fn dark_mode() -> Option<bool> {
        theme::dark_mode()
    }

    /// winit reports no theme changes on X11: the portal's do.
    fn watch_dark_mode(changed: Box<dyn Fn(Option<bool>) + Send>) {
        theme::watch(changed);
    }

    fn main_window(cc: &eframe::CreationContext<'_>) -> Option<Self::Window> {
        window::main_window(cc)
    }

    fn hide_to_tray(w: Self::Window) {
        window::hide_to_tray(w);
    }

    fn show_window(w: Self::Window) {
        window::show(w);
    }

    fn on_tray_thread(f: Box<dyn FnOnce() + Send>) {
        window::on_tray_thread(f);
    }

    fn tray_supported() -> Result<(), String> {
        window::tray_supported()
    }

    fn minimize_window(w: Self::Window) {
        window::minimize(w);
    }

    fn in_tray(w: Self::Window) -> bool {
        window::in_tray(w)
    }

    fn window_shown(w: Self::Window) -> bool {
        window::shown(w)
    }

    /// The desktop's setting: KDE's `kdeglobals`, else GNOME's (GTK's) through gsettings,
    /// else 400 ms, the default of both.
    fn double_click_time() -> Duration {
        let kde = || {
            let text = std::fs::read_to_string(config_home()?.join("kdeglobals")).ok()?;
            ini_value(&text, "KDE", "DoubleClickInterval")?.parse().ok()
        };
        let gnome = || {
            let out = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.peripherals.mouse", "double-click"])
                .output()
                .ok()?;
            // "400" or "uint32 400".
            let text = String::from_utf8(out.stdout).ok()?;
            text.split_whitespace().last()?.parse().ok()
        };
        let ms: u64 = kde().or_else(gnome).filter(|&ms| ms > 0).unwrap_or(400);
        Duration::from_millis(ms)
    }

    fn tray_icon_size() -> u32 {
        Fallback::tray_icon_size()
    }

    fn set_autostart(enabled: bool, arg: &str) -> Result<(), String> {
        autostart::set(enabled, arg)
    }

    fn device_access_missing() -> bool {
        gob_uinput::access::missing()
    }

    fn grant_device_access() -> Result<(), String> {
        gob_uinput::access::install()
    }

    /// The stick's hidraw node: buttons past 80 come from there.
    fn grant_all_buttons(vendor_id: u16, product_id: u16) -> Result<(), String> {
        gob_uinput::access::install_raw(vendor_id, product_id)
    }

    /// Started from an AppImage (it updates as one file).
    fn installed_copy(_exe_dir: &Path) -> bool {
        update::installed_copy()
    }

    /// Only AppImage copies update; a build or a package does not.
    fn update_asset(installed: bool) -> Option<&'static str> {
        installed.then_some("-x86_64.AppImage")
    }

    fn exe_name() -> &'static str {
        "JoystickGobloon"
    }

    /// The new AppImage replaces the running one and starts.
    fn run_installer(setup: &Path) -> Result<(), String> {
        update::replace_appimage(setup)
    }

    fn unpack_zip(zip: &Path, to: &Path) -> Result<(), String> {
        Fallback::unpack_zip(zip, to)
    }

    fn wait_for_process(pid: u32, timeout: Duration) {
        update::wait_for_process(pid, timeout);
    }
}

#[cfg(test)]
mod tests {
    use super::ini_value;

    #[test]
    fn kdeglobals_double_click() {
        let text = "[General]\nDoubleClickInterval=1\n\n[KDE]\nSingleClick=false\nDoubleClickInterval = 550\n";
        assert_eq!(ini_value(text, "KDE", "DoubleClickInterval"), Some("550"));
        assert_eq!(ini_value(text, "KDE", "Missing"), None);
        assert_eq!(ini_value("", "KDE", "DoubleClickInterval"), None);
    }
}
