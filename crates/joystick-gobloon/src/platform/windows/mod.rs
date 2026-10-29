//! Windows desktop integration (Win32).

mod instance;
mod keys;
mod update;
mod window;

use std::path::{Path, PathBuf};
use std::time::Duration;

use gob_core::Key;

pub(crate) struct Desktop;

impl super::Desktop for Desktop {
    type InstanceEvent = instance::Event;
    type Window = window::Window;

    fn default_data_dir() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("JoystickGobloon"))
    }

    /// Fonts every Windows 10/11 has: Microsoft YaHei, Malgun Gothic.
    fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
        let windows = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
        let dir = Path::new(&windows).join("Fonts");
        vec![
            ("cjk-zh", dir.join("msyh.ttc")),
            ("cjk-ko", dir.join("malgun.ttf")),
        ]
    }

    fn open_text_file(path: &Path) {
        let _ = std::process::Command::new("notepad.exe").arg(path).spawn();
    }

    fn program_extensions() -> &'static [&'static str] {
        &["exe"]
    }

    fn local_time() -> String {
        // SAFETY: GetLocalTime only writes the returned struct.
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
        )
    }

    fn claim_instance() -> Option<instance::Event> {
        instance::claim()
    }

    fn listen_instance(event: instance::Event, show: Box<dyn Fn() + Send>) {
        instance::listen(event, show);
    }

    /// The low-level keyboard hook sees every key.
    fn keyboard_bindings() -> bool {
        true
    }

    fn keyboard_access() -> super::KeyboardAccess {
        super::KeyboardAccess::Open
    }

    fn set_keyboard_access(_allow: bool) -> Result<(), String> {
        Ok(())
    }

    fn held_keys() -> Option<Vec<Key>> {
        Some(keys::held())
    }

    fn key_name(key: Key) -> String {
        keys::name(key)
    }

    fn dark_mode() -> Option<bool> {
        window::dark_mode()
    }

    /// winit reports Windows' theme changes itself.
    fn watch_dark_mode(_changed: Box<dyn Fn(Option<bool>) + Send>) {}

    fn main_window(cc: &eframe::CreationContext<'_>) -> Option<window::Window> {
        window::main_window(cc)
    }

    fn hide_to_tray(w: window::Window) {
        window::hide_to_tray(w);
    }

    fn show_window(w: window::Window) {
        window::show(w);
    }

    /// The UI thread: the tray's hidden window gets its messages from winit's loop there.
    fn on_tray_thread(f: Box<dyn FnOnce() + Send>) {
        f();
    }

    fn tray_supported() -> Result<(), String> {
        Ok(())
    }

    fn minimize_window(w: window::Window) {
        window::minimize(w);
    }

    fn in_tray(w: window::Window) -> bool {
        window::in_tray(w)
    }

    fn window_shown(w: window::Window) -> bool {
        window::visible(w) && !window::in_tray(w)
    }

    fn double_click_time() -> Duration {
        window::double_click_time()
    }

    fn tray_icon_size() -> u32 {
        window::small_icon_size()
    }

    fn set_autostart(enabled: bool, arg: &str) -> Result<(), String> {
        window::set_autostart(enabled, arg)
    }

    fn installed_copy(exe_dir: &Path) -> bool {
        update::installed_copy(exe_dir)
    }

    fn update_asset(installed: bool) -> Option<&'static str> {
        Some(update::update_asset(installed))
    }

    fn exe_name() -> &'static str {
        "JoystickGobloon.exe"
    }

    fn run_installer(setup: &Path) -> Result<(), String> {
        update::run_installer(setup)
    }

    fn unpack_zip(zip: &Path, to: &Path) -> Result<(), String> {
        update::unpack_zip(zip, to)
    }

    fn wait_for_process(pid: u32, timeout: Duration) {
        update::wait_for_process(pid, timeout);
    }

    /// The drivers' installers give access; nothing for the user to grant.
    fn device_access_missing() -> bool {
        false
    }

    fn grant_device_access() -> Result<(), String> {
        Ok(())
    }
}
