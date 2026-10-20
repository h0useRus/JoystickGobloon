//! Platforms without their own module; Linux's module takes the double-click time, the tray
//! icon size and zip unpacking from here.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gob_core::Key;

pub(crate) struct Desktop;

impl super::Desktop for Desktop {
    type InstanceEvent = ();
    type Window = ();

    fn default_data_dir() -> Option<PathBuf> {
        None
    }

    fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
        Vec::new()
    }

    fn open_text_file(_path: &Path) {}

    fn program_extensions() -> &'static [&'static str] {
        &[]
    }

    fn local_time() -> String {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}.{:03}", d.as_secs(), d.subsec_millis())
    }

    fn claim_instance() -> Option<()> {
        Some(())
    }

    fn listen_instance(_event: (), _show: Box<dyn Fn() + Send>) {}

    fn keyboard_bindings() -> bool {
        false
    }

    fn held_keys() -> Option<Vec<Key>> {
        None
    }

    fn key_name(key: Key) -> String {
        super::key_code(key)
    }

    fn main_window(_cc: &eframe::CreationContext<'_>) -> Option<()> {
        None
    }

    fn hide_to_tray(_window: ()) {}

    fn show_window(_window: ()) {}

    fn on_tray_thread(f: Box<dyn FnOnce() + Send>) {
        f();
    }

    fn minimize_window(_window: ()) {}

    fn in_tray(_window: ()) -> bool {
        false
    }

    fn window_shown(_window: ()) -> bool {
        true
    }

    fn double_click_time() -> Duration {
        Duration::from_millis(500)
    }

    fn tray_icon_size() -> u32 {
        16
    }

    fn set_autostart(_enabled: bool, _arg: &str) -> Result<(), String> {
        Ok(())
    }

    fn device_access_missing() -> bool {
        false
    }

    fn grant_device_access() -> Result<(), String> {
        Err("not supported on this system".into())
    }

    fn installed_copy(_exe_dir: &Path) -> bool {
        false
    }

    fn update_asset(_installed: bool) -> Option<&'static str> {
        None
    }

    fn exe_name() -> &'static str {
        "JoystickGobloon"
    }

    fn run_installer(_setup: &Path) -> Result<(), String> {
        Err("no installer on this system".into())
    }

    fn unpack_zip(_zip: &Path, _to: &Path) -> Result<(), String> {
        Err("unpacking is not supported on this system".into())
    }

    fn wait_for_process(_pid: u32, _timeout: Duration) {}
}
