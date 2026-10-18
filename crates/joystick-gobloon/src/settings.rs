//! Settings and other user data. They live in `%APPDATA%\JoystickGobloon`, unless the exe's own
//! folder has a `settings.gob`: then that folder is used (a development copy that must not touch
//! the settings of the installed one). The `.gob` files are TOML text.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    /// Follows the Windows light/dark setting.
    #[default]
    System,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub language: String,
    /// Reopen the last profile on startup.
    pub reopen_last: bool,
    /// Add Joystick Gobloon to the HidHide whitelist on startup so it can read hidden devices.
    pub hidhide_whitelist_self: bool,
    /// Device hiding window: leave out disconnected devices.
    pub hidhide_hide_disconnected: bool,
    /// Device hiding window: only game controllers.
    pub hidhide_gaming_only: bool,
    pub last_profile: Option<PathBuf>,
    /// Icon in the notification area (tray).
    pub show_tray: bool,
    /// The close button hides the window to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Minimizing hides the window to the tray (off the taskbar).
    pub minimize_to_tray: bool,
    /// Windows starts Joystick Gobloon at logon (minimized).
    pub start_with_windows: bool,
    /// Activate the reopened profile on startup.
    pub activate_on_start: bool,
    /// Look for a new release on startup and install it.
    pub check_updates: bool,
    /// Axis calibrations: device GUID -> axis slot ("1".."8") -> values.
    pub calibration: BTreeMap<String, BTreeMap<String, AxisCalibration>>,
    /// Hidden option (not in the UI, written only when on): allow several running copies.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub multi_instance: bool,
}

/// A program on the Applications page; options per program (e.g. its profile) go here.
#[derive(Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub path: PathBuf,
}

/// Programs on the Applications page, in `apps.gob` next to the settings.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppList {
    #[serde(rename = "app")]
    pub apps: Vec<AppEntry>,
}

impl AppList {
    pub fn load() -> Self {
        load_toml("apps.gob")
    }

    pub fn save(&self) {
        save_toml("apps.gob", self);
    }
}

/// One axis calibration in raw DirectInput units (see `gob_input::Calibration`).
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct AxisCalibration {
    pub low: i32,
    pub center_low: i32,
    pub center_high: i32,
    pub high: i32,
    pub with_center: bool,
}

impl From<gob_input::Calibration> for AxisCalibration {
    fn from(c: gob_input::Calibration) -> Self {
        Self {
            low: c.low,
            center_low: c.center_low,
            center_high: c.center_high,
            high: c.high,
            with_center: c.with_center,
        }
    }
}

impl From<AxisCalibration> for gob_input::Calibration {
    fn from(c: AxisCalibration) -> Self {
        Self {
            low: c.low,
            center_low: c.center_low,
            center_high: c.center_high,
            high: c.high,
            with_center: c.with_center,
        }
    }
}

impl Settings {
    /// Saved calibrations by (device, slot).
    pub fn calibrations(&self) -> gob_input::Calibrations {
        let mut map = gob_input::Calibrations::new();
        for (device, axes) in &self.calibration {
            for (slot, c) in axes {
                if let Ok(slot) = slot.parse() {
                    map.insert((device.to_ascii_lowercase(), slot), (*c).into());
                }
            }
        }
        map
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            language: gob_i18n::DEFAULT_LANG.into(),
            reopen_last: true,
            hidhide_whitelist_self: true,
            hidhide_hide_disconnected: true,
            hidhide_gaming_only: true,
            last_profile: None,
            show_tray: true,
            close_to_tray: false,
            minimize_to_tray: false,
            start_with_windows: false,
            activate_on_start: false,
            check_updates: true,
            calibration: BTreeMap::new(),
            multi_instance: false,
        }
    }
}

impl Settings {
    /// Reads `settings.gob`; the first time, the old `settings.toml` instead.
    pub fn load() -> Self {
        let exists = data_dir().is_some_and(|dir| dir.join("settings.gob").is_file());
        load_toml(if exists {
            "settings.gob"
        } else {
            "settings.toml"
        })
    }

    pub fn save(&self) {
        save_toml("settings.gob", self);
    }
}

/// Folder of settings, the app list and the log (see the module comment).
pub fn data_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let local = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .filter(|dir| dir.join("settings.gob").is_file());
        local.or_else(|| {
            std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("JoystickGobloon"))
        })
    })
    .as_deref()
}

fn load_toml<T: Default + for<'de> Deserialize<'de>>(name: &str) -> T {
    data_dir()
        .and_then(|dir| std::fs::read_to_string(dir.join(name)).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_toml(name: &str, value: &impl Serialize) {
    let Some(dir) = data_dir() else { return };
    let _ = std::fs::create_dir_all(dir);
    if let Ok(text) = toml::to_string_pretty(value) {
        let _ = std::fs::write(dir.join(name), text);
    }
}
