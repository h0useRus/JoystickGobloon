//! Linux desktop integration. Device access (the udev rule, gob-uinput's `access`) is Linux's
//! own; the rest still comes from `none.rs` until phase 5 (docs/LINUX_PLAN.md): XDG paths, a
//! socket for the single instance, fontconfig, the tray.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gob_core::Key;

use super::none::Desktop as Fallback;

pub(crate) struct Desktop;

impl super::Desktop for Desktop {
    type InstanceEvent = <Fallback as super::Desktop>::InstanceEvent;
    type Window = <Fallback as super::Desktop>::Window;

    fn default_data_dir() -> Option<PathBuf> {
        Fallback::default_data_dir()
    }

    fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
        Fallback::cjk_fonts()
    }

    fn open_text_file(path: &Path) {
        Fallback::open_text_file(path);
    }

    fn program_extensions() -> &'static [&'static str] {
        Fallback::program_extensions()
    }

    fn local_time() -> String {
        Fallback::local_time()
    }

    fn claim_instance() -> Option<Self::InstanceEvent> {
        Fallback::claim_instance()
    }

    fn listen_instance(event: Self::InstanceEvent, show: Box<dyn Fn() + Send>) {
        Fallback::listen_instance(event, show);
    }

    fn held_keys() -> Vec<Key> {
        Fallback::held_keys()
    }

    fn key_name(key: Key) -> String {
        Fallback::key_name(key)
    }

    fn main_window(cc: &eframe::CreationContext<'_>) -> Option<Self::Window> {
        Fallback::main_window(cc)
    }

    fn hide_to_tray(window: Self::Window) {
        Fallback::hide_to_tray(window);
    }

    fn show_window(window: Self::Window) {
        Fallback::show_window(window);
    }

    fn minimize_window(window: Self::Window) {
        Fallback::minimize_window(window);
    }

    fn in_tray(window: Self::Window) -> bool {
        Fallback::in_tray(window)
    }

    fn window_shown(window: Self::Window) -> bool {
        Fallback::window_shown(window)
    }

    fn double_click_time() -> Duration {
        Fallback::double_click_time()
    }

    fn tray_icon_size() -> u32 {
        Fallback::tray_icon_size()
    }

    fn set_autostart(enabled: bool, arg: &str) -> Result<(), String> {
        Fallback::set_autostart(enabled, arg)
    }

    fn device_access_missing() -> bool {
        gob_uinput::access::missing()
    }

    fn grant_device_access() -> Result<(), String> {
        gob_uinput::access::install()
    }

    fn installed_copy(exe_dir: &Path) -> bool {
        Fallback::installed_copy(exe_dir)
    }

    fn update_asset(installed: bool) -> Option<&'static str> {
        Fallback::update_asset(installed)
    }

    fn exe_name() -> &'static str {
        Fallback::exe_name()
    }

    fn run_installer(setup: &Path) -> Result<(), String> {
        Fallback::run_installer(setup)
    }

    fn unpack_zip(zip: &Path, to: &Path) -> Result<(), String> {
        Fallback::unpack_zip(zip, to)
    }

    fn wait_for_process(pid: u32, timeout: Duration) {
        Fallback::wait_for_process(pid, timeout);
    }
}
