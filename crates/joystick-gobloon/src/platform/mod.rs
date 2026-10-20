//! Desktop integration per platform behind one interface, so the pages and the app logic stay the
//! same everywhere (docs/LINUX_PLAN.md, "Архитектура"). `windows/`: Win32; `linux/`: device
//! access, the rest from `none.rs` until phase 5; `none.rs`: what other platforms get.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gob_core::Key;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(windows))]
mod none;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux::Desktop as Current;
#[cfg(not(any(windows, target_os = "linux")))]
use none::Desktop as Current;
#[cfg(windows)]
use windows::Desktop as Current;

/// One platform's desktop integration.
pub(crate) trait Desktop {
    /// Wakes a running copy (see [`claim_instance`]).
    type InstanceEvent: Send + 'static;
    /// The main window, as the tray and the app's hiding logic address it.
    type Window: Copy + Send + Sync + 'static;

    /// Folder for settings, the app list and the log when the program has no portable one.
    fn default_data_dir() -> Option<PathBuf>;
    /// Font files with Chinese and Korean glyphs: (egui font name, file).
    fn cjk_fonts() -> Vec<(&'static str, PathBuf)>;
    /// Opens a text file (the log) in the system's editor.
    fn open_text_file(path: &Path);
    /// File extensions of programs, for the file picker; empty: any file.
    fn program_extensions() -> &'static [&'static str];
    /// Local date and time for log lines.
    fn local_time() -> String;
    /// `Some` for the first running copy; `None` when another copy runs (it was woken).
    fn claim_instance() -> Option<Self::InstanceEvent>;
    /// Calls `show` whenever a later start wakes this copy.
    fn listen_instance(event: Self::InstanceEvent, show: Box<dyn Fn() + Send>);
    /// Keyboard keys can be bound as inputs (the Keyboard tab): the program may read every
    /// keyboard.
    fn keyboard_bindings() -> bool;
    /// Keyboard keys held right now.
    fn held_keys() -> Vec<Key>;
    /// A key's name in the current keyboard layout, e.g. "Right Ctrl".
    fn key_name(key: Key) -> String;

    /// The main window; `None` where the platform cannot address it (the app then hides and
    /// shows it through eframe only).
    fn main_window(cc: &eframe::CreationContext<'_>) -> Option<Self::Window>;
    /// Puts the window into the tray. It must keep getting frames there (the app's logic runs
    /// in them) without being drawn on screen or in the task switcher. May be called from inside
    /// the window's event handling.
    fn hide_to_tray(window: Self::Window);
    /// Shows the window again, from the tray or minimized.
    fn show_window(window: Self::Window);
    fn minimize_window(window: Self::Window);
    fn in_tray(window: Self::Window) -> bool;
    /// On screen: visible, not minimized, not in the tray.
    fn window_shown(window: Self::Window) -> bool;
    /// The user's double-click time (a tray click waits for it).
    fn double_click_time() -> Duration;
    /// Size of a tray icon in pixels at the current display scaling.
    fn tray_icon_size() -> u32;
    /// Starts the running program with `arg` when the user logs on, or stops doing so.
    fn set_autostart(enabled: bool, arg: &str) -> Result<(), String>;
    /// The user lacks access to devices the program needs (controllers, virtual devices) that
    /// [`Desktop::grant_device_access`] can give.
    fn device_access_missing() -> bool;
    /// Gives the user that access once and for good, with administrator rights (the system
    /// asks). Blocking: call it off the GUI thread.
    fn grant_device_access() -> Result<(), String>;

    /// The copy in `exe_dir` is managed by the system's installer (it updates through a new one).
    fn installed_copy(exe_dir: &Path) -> bool;
    /// End of the release asset name this copy updates from; `None`: no updates here yet.
    fn update_asset(installed: bool) -> Option<&'static str>;
    /// File name of the program in a release.
    fn exe_name() -> &'static str;
    /// Runs a downloaded installer; it closes this copy and starts the new one.
    fn run_installer(setup: &Path) -> Result<(), String>;
    /// Unpacks a downloaded zip into `to`.
    fn unpack_zip(zip: &Path, to: &Path) -> Result<(), String>;
    /// Waits until process `pid` exits, at most `timeout`.
    fn wait_for_process(pid: u32, timeout: Duration);
}

/// What a later start signals to wake this copy.
pub(crate) type InstanceEvent = <Current as Desktop>::InstanceEvent;
/// The main window (an opaque handle).
pub(crate) type Window = <Current as Desktop>::Window;

pub(crate) fn default_data_dir() -> Option<PathBuf> {
    Current::default_data_dir()
}

pub(crate) fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
    Current::cjk_fonts()
}

pub(crate) fn open_text_file(path: &Path) {
    Current::open_text_file(path);
}

pub(crate) fn program_extensions() -> &'static [&'static str] {
    Current::program_extensions()
}

pub(crate) fn local_time() -> String {
    Current::local_time()
}

pub(crate) fn claim_instance() -> Option<InstanceEvent> {
    Current::claim_instance()
}

pub(crate) fn listen_instance(event: InstanceEvent, show: impl Fn() + Send + 'static) {
    Current::listen_instance(event, Box::new(show));
}

pub(crate) fn keyboard_bindings() -> bool {
    Current::keyboard_bindings()
}

pub(crate) fn held_keys() -> Vec<Key> {
    Current::held_keys()
}

pub(crate) fn key_name(key: Key) -> String {
    Current::key_name(key)
}

pub(crate) fn main_window(cc: &eframe::CreationContext<'_>) -> Option<Window> {
    Current::main_window(cc)
}

pub(crate) fn hide_to_tray(window: Window) {
    Current::hide_to_tray(window);
}

pub(crate) fn show_window(window: Window) {
    Current::show_window(window);
}

pub(crate) fn minimize_window(window: Window) {
    Current::minimize_window(window);
}

pub(crate) fn in_tray(window: Window) -> bool {
    Current::in_tray(window)
}

pub(crate) fn window_shown(window: Window) -> bool {
    Current::window_shown(window)
}

pub(crate) fn double_click_time() -> Duration {
    Current::double_click_time()
}

pub(crate) fn tray_icon_size() -> u32 {
    Current::tray_icon_size()
}

pub(crate) fn set_autostart(enabled: bool, arg: &str) -> Result<(), String> {
    Current::set_autostart(enabled, arg)
}

pub(crate) fn device_access_missing() -> bool {
    Current::device_access_missing()
}

pub(crate) fn grant_device_access() -> Result<(), String> {
    Current::grant_device_access()
}

pub(crate) fn installed_copy(exe_dir: &Path) -> bool {
    Current::installed_copy(exe_dir)
}

pub(crate) fn update_asset(installed: bool) -> Option<&'static str> {
    Current::update_asset(installed)
}

pub(crate) fn exe_name() -> &'static str {
    Current::exe_name()
}

pub(crate) fn run_installer(setup: &Path) -> Result<(), String> {
    Current::run_installer(setup)
}

pub(crate) fn unpack_zip(zip: &Path, to: &Path) -> Result<(), String> {
    Current::unpack_zip(zip, to)
}

pub(crate) fn wait_for_process(pid: u32, timeout: Duration) {
    Current::wait_for_process(pid, timeout);
}

/// A key as its code, when the system has no name for it: `0x1d`, `E0 0x1d`.
fn key_code(key: Key) -> String {
    format!(
        "{}{:#04x}",
        if key.extended { "E0 " } else { "" },
        key.scan_code
    )
}
