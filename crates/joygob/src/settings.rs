use std::collections::BTreeMap;
use std::path::PathBuf;

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
    /// Add JoyGob to the HidHide whitelist on startup so it can read hidden devices.
    pub hidhide_whitelist_self: bool,
    /// Device hiding window: leave out disconnected devices.
    pub hidhide_hide_disconnected: bool,
    /// Device hiding window: only game controllers.
    pub hidhide_gaming_only: bool,
    /// Applications known to JoyGob (Applications page); per-app options go here.
    pub apps: Vec<AppEntry>,
    pub last_profile: Option<PathBuf>,
    /// Icon in the notification area (tray).
    pub show_tray: bool,
    /// The close button hides the window to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Windows starts JoyGob at logon (minimized).
    pub start_with_windows: bool,
    /// Activate the reopened profile on startup.
    pub activate_on_start: bool,
    /// Axis calibrations: device GUID -> axis slot ("1".."8") -> values.
    pub calibration: BTreeMap<String, BTreeMap<String, AxisCalibration>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub path: PathBuf,
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
            apps: Vec::new(),
            last_profile: None,
            show_tray: true,
            close_to_tray: false,
            start_with_windows: false,
            activate_on_start: false,
            calibration: BTreeMap::new(),
        }
    }
}

impl Settings {
    fn path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|p| {
            PathBuf::from(p)
                .join("JoystickGobloon")
                .join("settings.toml")
        })
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }
}
