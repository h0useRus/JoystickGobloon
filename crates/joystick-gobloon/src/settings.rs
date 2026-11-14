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

/// What goes into the log file (`logfile`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Everything, debug messages too.
    All,
    /// Warnings, errors and notable events (a profile loaded, a device back, an update).
    Important,
    /// Warnings and errors only.
    #[default]
    Critical,
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
    /// Copies of unsaved profile changes (`autosave`).
    pub autosave: Vec<crate::autosave::Entry>,
    /// Folder of the last profile opened or saved through a file dialog, where those dialogs
    /// start (else the documents folder).
    pub profile_dir: Option<PathBuf>,
    /// Icon in the notification area (tray).
    pub show_tray: bool,
    /// The close button hides the window to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Minimizing hides the window to the tray (off the taskbar).
    pub minimize_to_tray: bool,
    /// A double click on the tray icon activates / deactivates the profile.
    pub tray_double_click_activates: bool,
    /// Windows starts Joystick Gobloon at logon (minimized).
    pub start_with_windows: bool,
    /// Devices page: using an input selects its device tab and scrolls the list to it.
    pub focus_used_input: bool,
    /// ... except for the profile's modifier buttons and keys.
    pub focus_skips_modifiers: bool,
    /// Read controllers when DirectInput signals a change instead of polling them.
    pub input_events: bool,
    /// Milliseconds between two reads of a polled controller (all of them without
    /// `input_events`, the ones that cannot signal a change with it).
    pub poll_interval_ms: u8,
    /// Activate the reopened profile on startup.
    pub activate_on_start: bool,
    /// Look for a new release on startup and install it.
    pub check_updates: bool,
    /// Response curve editor: edits never make a rising (or falling) curve turn back.
    pub monotonic_curves: bool,
    /// Response curve editor: a right click on empty space inverts the curve.
    pub curve_right_click_invert: bool,
    /// Ask before saving makes a profile Joystick Gremlin cannot open.
    pub warn_gremlin_format: bool,
    /// Language voice commands are spoken in (a speech model's); empty = the program's.
    pub voice_language: String,
    /// Microphone gain of voice commands (1.0 = as recorded).
    pub voice_gain: f32,
    /// Voice commands heard less sure than this (0..1) count as not recognized.
    pub voice_min_confidence: f32,
    /// Keep the microphones of voice commands open while a profile runs (some are silent for a
    /// moment after they open); only what is said while listening is recognized.
    pub voice_keep_open: bool,
    /// What the log file records.
    pub log_level: LogLevel,
    /// Main window size (logical points) when it was last resized; the next start opens it so.
    pub window_size: [f32; 2],
    /// The main window was maximized when last seen.
    pub window_maximized: bool,
    /// Experimental features (joystick LEDs): hidden and not run while off. Not in the UI:
    /// `experimental = true` in settings.gob turns them on.
    pub experimental: bool,
    /// Axis calibrations: device GUID -> axis slot ("1".."8") -> values.
    pub calibration: BTreeMap<String, BTreeMap<String, AxisCalibration>>,
    /// Button and hat debounce per device GUID, in milliseconds (see
    /// `gob_input::InputService::set_debounce`).
    pub debounce: BTreeMap<String, u16>,
    /// Default LED colors: device GUID -> LEDs, set when a profile that changed that
    /// device's LEDs stops (VKB keeps written colors until the stick restarts).
    pub leds: BTreeMap<String, DeviceLeds>,
    /// Hidden option (not in the UI, written only when on): allow several running copies.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub multi_instance: bool,
}

/// A program on the Applications page; options per program (e.g. its profile) go here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppEntry {
    pub path: PathBuf,
    /// Profile that runs while this program is in front (switching profiles by program).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<PathBuf>,
    /// Mode of `profile` to start in; `None`: the profile's starting mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl AppEntry {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            profile: None,
            mode: None,
        }
    }

    /// The "any other program" entry: its profile runs while no program of the page with a
    /// profile is in front.
    pub fn other() -> Self {
        Self::new(PathBuf::from(OTHER_PROGRAMS))
    }

    pub fn is_other(&self) -> bool {
        self.path.as_os_str() == OTHER_PROGRAMS
    }

    /// What programs are matched by: the file name without case, so every copy of a game
    /// (other install folders, test builds) counts.
    pub fn key(&self) -> String {
        // Both separators: a Wine program may be kept by its Windows path on Linux.
        let path = self.path.to_string_lossy();
        program_key(path.rsplit(['\\', '/']).next().unwrap_or(&path))
    }
}

/// Path of the [`AppEntry::other`] entry in `apps.gob`.
pub const OTHER_PROGRAMS: &str = "*";

/// [`AppEntry::key`] of a program's file name.
pub fn program_key(name: &str) -> String {
    name.to_lowercase()
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

/// Default LED colors of one device.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceLeds {
    /// Device name when saved, for reading the file.
    pub name: String,
    pub led: Vec<LedDefault>,
}

/// One default LED; values as light-led stores them.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LedDefault {
    pub id: u8,
    /// 0: the whole LED; 1, 2: one element (VKB base: blue, red).
    pub element: u8,
    /// `#RRGGBB`.
    pub color: String,
    pub color2: String,
    /// light-led's `color-mode` names (`color1`, `alternate`, ...).
    pub blend: String,
    /// light-led's `led-mode` names (`on`, `slow-blink`, ...).
    pub mode: String,
}

impl LedDefault {
    pub fn value(&self) -> crate::leds::LedValue {
        crate::leds::LedValue {
            id: self.id,
            element: self.element,
            color: gob_core::parse_color(&self.color).unwrap_or_default(),
            color2: gob_core::parse_color(&self.color2).unwrap_or_default(),
            blend: gob_core::LedBlend::parse(&self.blend).unwrap_or_default(),
            mode: gob_core::LedMode::parse(&self.mode).unwrap_or_default(),
        }
    }

    pub fn from_value(v: crate::leds::LedValue) -> Self {
        Self {
            id: v.id,
            element: v.element,
            color: gob_core::format_color(v.color),
            color2: gob_core::format_color(v.color2),
            blend: v.blend.name().into(),
            mode: v.mode.name().into(),
        }
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
    /// Jitter threshold, tenths of a percent of the travel; written only when set.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub jitter: u16,
}

fn is_zero(v: &u16) -> bool {
    *v == 0
}

impl From<gob_input::Calibration> for AxisCalibration {
    fn from(c: gob_input::Calibration) -> Self {
        Self {
            low: c.low,
            center_low: c.center_low,
            center_high: c.center_high,
            high: c.high,
            with_center: c.with_center,
            jitter: c.jitter,
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
            jitter: c.jitter,
        }
    }
}

impl Settings {
    /// Default LED colors for the engine (set when a profile stops); `None` while
    /// experimental features are off: no LED control at all.
    pub fn led_defaults(&self) -> Option<Vec<gob_engine::Light>> {
        self.experimental.then(|| {
            self.leds
                .iter()
                .flat_map(|(guid, d)| d.led.iter().map(move |l| l.value().light(guid)))
                .collect()
        })
    }

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

/// The main window's size at the first start, in logical points.
pub const DEFAULT_WINDOW_SIZE: [f32; 2] = [1240.0, 890.0];
/// The main window's smallest size.
pub const MIN_WINDOW_SIZE: [f32; 2] = [640.0, 400.0];

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
            autosave: Vec::new(),
            profile_dir: None,
            show_tray: true,
            close_to_tray: false,
            minimize_to_tray: false,
            tray_double_click_activates: true,
            start_with_windows: false,
            focus_used_input: true,
            focus_skips_modifiers: true,
            input_events: true,
            poll_interval_ms: gob_input::DEFAULT_POLL_INTERVAL.as_millis() as u8,
            activate_on_start: false,
            check_updates: true,
            monotonic_curves: true,
            curve_right_click_invert: true,
            warn_gremlin_format: true,
            voice_language: String::new(),
            voice_gain: 1.0,
            voice_min_confidence: 0.5,
            voice_keep_open: true,
            log_level: LogLevel::default(),
            window_size: DEFAULT_WINDOW_SIZE,
            window_maximized: false,
            calibration: BTreeMap::new(),
            debounce: BTreeMap::new(),
            leds: BTreeMap::new(),
            experimental: false,
            multi_instance: false,
        }
    }
}

impl Settings {
    /// The main window size to open with: the saved one, at least the smallest size (a
    /// hand-edited or broken value falls back to the default).
    pub fn window_size(&self) -> [f32; 2] {
        let [w, h] = self.window_size;
        if !(w.is_finite() && h.is_finite()) {
            return DEFAULT_WINDOW_SIZE;
        }
        [w.max(MIN_WINDOW_SIZE[0]), h.max(MIN_WINDOW_SIZE[1])]
    }

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
        local.or_else(crate::platform::default_data_dir)
    })
    .as_deref()
}

/// Reads `name` from the data folder; defaults when it is missing or does not parse (logged).
fn load_toml<T: Default + for<'de> Deserialize<'de>>(name: &str) -> T {
    let Some(path) = data_dir().map(|dir| dir.join(name)) else {
        return T::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return T::default();
    };
    toml::from_str(&text).unwrap_or_else(|e| {
        log::error!("{}: {e}; using defaults", path.display());
        T::default()
    })
}

/// Writes `path` so that a crash or power loss leaves either the old file or the new one, never
/// a cut one: the text goes to a file next to it, reaches the disk, then replaces it.
pub(crate) fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    let new = path.with_file_name(name);
    let mut file = std::fs::File::create(&new)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&new, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&new);
    })
}

fn save_toml(name: &str, value: &impl Serialize) {
    let Some(dir) = data_dir() else {
        log::error!("saving {name}: no data folder");
        return;
    };
    let path = dir.join(name);
    let written = std::fs::create_dir_all(dir).and_then(|()| {
        let text = toml::to_string_pretty(value).map_err(std::io::Error::other)?;
        write_whole(&path, text.as_bytes())
    });
    if let Err(e) = written {
        log::error!("saving {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_lists_keep_profiles_and_read_old_files() {
        let old: AppList = toml::from_str("[[app]]\npath = 'C:/Games/One/One.exe'\n").unwrap();
        assert_eq!(
            old.apps,
            [AppEntry::new(PathBuf::from("C:/Games/One/One.exe"))]
        );
        let mut list = old;
        list.apps[0].profile = Some(PathBuf::from("D:/Profiles/Flight.xml"));
        list.apps[0].mode = Some("Landing".into());
        list.apps.push(AppEntry::new(PathBuf::from("radio.exe")));
        let text = toml::to_string_pretty(&list).unwrap();
        assert_eq!(
            text.matches("profile").count(),
            1,
            "no empty fields:\n{text}"
        );
        let back: AppList = toml::from_str(&text).unwrap();
        assert_eq!(back.apps, list.apps);
    }

    #[test]
    fn whole_writes_replace_the_file() {
        let dir = std::env::temp_dir().join(format!("gobloon-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.gob");
        std::fs::write(&path, "old = true\n").unwrap();
        write_whole(&path, b"last_profile = 'a.xml'\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "last_profile = 'a.xml'\n"
        );
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["settings.gob"], "no file left aside");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
