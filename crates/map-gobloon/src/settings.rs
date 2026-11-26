//! Where Map Gobloon keeps things: Joystick Gobloon's data folder (its language and theme are
//! used here too) and a small file of its own with the device last open.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const OWN_FILE: &str = "map-gobloon.gob";

/// Joystick Gobloon's data folder: the exe's folder when it has a `settings.gob` (a portable
/// copy, and debug builds), else the system's.
pub fn data_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let local = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .filter(|dir| dir.join("settings.gob").is_file());
        local.or_else(crate::platform::default_data_dir)
    })
    .as_deref()
}

/// What Map Gobloon reads from Joystick Gobloon's settings.
#[derive(Debug, Default, Deserialize)]
pub struct Shared {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub theme: Theme,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

impl Shared {
    pub fn load() -> Self {
        data_dir()
            .and_then(|d| std::fs::read_to_string(d.join("settings.gob")).ok())
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }
}

/// Map Gobloon's own state.
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct Own {
    /// Id of the device plugin last open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_device: Option<String>,
}

impl Own {
    pub fn load() -> Self {
        data_dir()
            .and_then(|d| std::fs::read_to_string(d.join(OWN_FILE)).ok())
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(dir) = data_dir() else {
            return;
        };
        if let Ok(text) = toml::to_string(self) {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(dir.join(OWN_FILE), text);
        }
    }
}
