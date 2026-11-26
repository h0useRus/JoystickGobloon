//! The few things Map Gobloon asks of the operating system.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as current;
#[cfg(not(any(windows, target_os = "linux")))]
mod none;
#[cfg(not(any(windows, target_os = "linux")))]
use none as current;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as current;

/// Joystick Gobloon's data folder when no portable copy says otherwise.
pub fn default_data_dir() -> Option<PathBuf> {
    current::default_data_dir()
}

/// Font files with Chinese and Korean glyphs: (egui font name, file).
pub fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
    current::cjk_fonts()
}

/// Shows a folder in the system's file manager.
pub fn open_folder(dir: &Path) {
    current::open_folder(dir);
}
