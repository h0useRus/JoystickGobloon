//! Linux desktop integration: XDG folders, a socket for the single instance, XDG autostart,
//! CJK fonts through fontconfig, the window and the tray (GTK thread), device access (the udev
//! rule, gob-uinput's `access`), updates of the AppImage. The rest is shared with `none.rs`.

mod autostart;
mod fonts;
mod instance;
mod keys;
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

    /// Off for now: reading every keyboard needs the `input` group (the udev rule leaves
    /// keyboards out, any program of the user could read what is typed). Back if users ask.
    fn keyboard_bindings() -> bool {
        false
    }

    fn held_keys() -> Option<Vec<Key>> {
        keys::held()
    }

    fn key_name(key: Key) -> String {
        keys::name(key).unwrap_or_else(|| super::key_code(key))
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

    fn minimize_window(w: Self::Window) {
        window::minimize(w);
    }

    fn in_tray(w: Self::Window) -> bool {
        window::in_tray(w)
    }

    fn window_shown(w: Self::Window) -> bool {
        window::shown(w)
    }

    fn double_click_time() -> Duration {
        Fallback::double_click_time()
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
