//! Virtual joysticks: the common interface the engine and the GUI use, whatever provides the
//! devices. Each platform has its own backend (docs/LINUX_PLAN.md, "Архитектура"): vJoy on
//! Windows (`windows.rs`, over gob-vjoy), uinput on Linux (`linux.rs`, over gob-uinput),
//! `unsupported.rs` elsewhere.
//! A new backend (e.g. our own Windows driver) only implements [`Backend`] and [`Device`].
//!
//! Device ids are 1-based like Gremlin's `vjoy-device`; axes use slots 1..=8 (X, Y, Z, Rx, Ry, Rz,
//! Slider 0, Slider 1) like gob-input.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(windows, target_os = "linux")))]
mod unsupported;
#[cfg(windows)]
mod windows;

/// Number of axis slots: X, Y, Z, Rx, Ry, Rz, Slider 0, Slider 1.
pub const AXIS_SLOTS: usize = 8;
/// Device ids a profile can name (Gremlin's `vjoy-device`, 1-based), whatever the backend has.
pub const PROFILE_DEVICES: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Acquired by this process.
    Own,
    Free,
    /// Acquired by another process.
    Busy,
    /// Not configured.
    Missing,
    Unknown,
}

/// A configured virtual device and its layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: u32,
    pub status: Status,
    /// Present axes as slot ids (1 = X ... 8 = Slider 1), as in Gremlin profiles.
    pub axes: Vec<u8>,
    pub buttons: u32,
    pub cont_hats: u32,
    pub disc_hats: u32,
}

impl DeviceInfo {
    pub fn hats(&self) -> u32 {
        self.cont_hats.max(self.disc_hats)
    }
}

#[derive(Debug)]
pub enum Error {
    /// No backend on this system (e.g. the vJoy driver is not installed).
    NotInstalled(String),
    /// The backend is there but switched off.
    Disabled,
    /// The device is missing, or owned by another program (e.g. Joystick Gremlin).
    Unavailable { id: u32, status: Status },
    /// No permission to create virtual devices (e.g. no udev rule for them yet).
    NoAccess(String),
    /// This platform has no virtual devices yet.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled(e) => write!(f, "virtual devices are not available: {e}"),
            Self::Disabled => write!(f, "virtual devices are switched off"),
            Self::Unavailable { id, status } => write!(f, "virtual device {id} is {status:?}"),
            Self::NoAccess(e) => write!(f, "no permission to create virtual devices: {e}"),
            Self::Unsupported => write!(f, "virtual devices are not supported on this system"),
        }
    }
}

impl std::error::Error for Error {}

/// What a backend can do; the GUI adapts to it instead of checking the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Product name for the user, e.g. "vJoy".
    pub name: &'static str,
    pub max_devices: u32,
    pub max_buttons: u32,
    pub max_hats: u32,
    /// Force feedback effects a device can declare, in [`DeviceConfig::effects`] order.
    pub effects: &'static [&'static str],
    /// The whole backend can be switched on and off ([`Change::Enable`]).
    pub switchable: bool,
}

/// No backend: nothing can be configured.
pub const NO_CAPABILITIES: Capabilities = Capabilities {
    name: "",
    max_devices: 0,
    max_buttons: 0,
    max_hats: 0,
    effects: &[],
    switchable: false,
};

/// One device's configuration, as the configuration page edits it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceConfig {
    pub id: u32,
    pub axes: [bool; AXIS_SLOTS],
    pub buttons: u32,
    pub hats: u32,
    /// 4-direction hats instead of continuous ones.
    pub discrete_hats: bool,
    /// One flag per [`Capabilities::effects`] entry.
    pub effects: Vec<bool>,
}

impl DeviceConfig {
    /// Defaults for a new device: every axis (Gremlin profiles expect them), 32 buttons, one
    /// continuous hat, no force feedback.
    pub fn new(id: u32, capabilities: &Capabilities) -> Self {
        Self {
            id,
            axes: [true; AXIS_SLOTS],
            buttons: 32.min(capabilities.max_buttons),
            hats: 1.min(capabilities.max_hats),
            discrete_hats: false,
            effects: vec![false; capabilities.effects.len()],
        }
    }
}

/// One configuration change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Create or replace a device.
    Set(DeviceConfig),
    Delete(u32),
    /// Switch the whole backend on or off (only when [`Capabilities::switchable`]).
    Enable(bool),
}

/// What applying a change left to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// The change is stored but the system must restart for it.
    RebootNeeded,
}

/// The virtual devices of this system. Shared (`Arc`) between the GUI and the engine.
pub trait Backend: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    /// Switched on and with at least one device.
    fn enabled(&self) -> bool;
    /// Version for the user, e.g. the driver version; may be empty.
    fn version(&self) -> String;
    fn status(&self, id: u32) -> Status;
    /// Every configured device.
    fn devices(&self) -> Vec<DeviceInfo>;
    /// Takes device `id` until the returned handle is dropped (reset and released then).
    fn acquire(&self, id: u32) -> Result<Box<dyn Device>, Error>;
    /// The stored configuration of every device. Blocking: call it off the GUI thread.
    fn read_config(&self) -> Result<Vec<DeviceConfig>, String>;
    /// Applies a change; may ask the user for administrator rights and waits for that.
    /// Blocking: call it off the GUI thread.
    fn apply(&self, change: &Change) -> Result<Outcome, String>;
}

/// An acquired virtual device. Reset and released when dropped.
pub trait Device: Send {
    /// Sets axis `slot` (1..=8) from a value in -1.0..=1.0.
    fn set_axis(&self, slot: u8, value: f32) -> bool;
    /// Presses or releases button `n` (1-based).
    fn set_button(&self, n: u8, pressed: bool) -> bool;
    /// Sets hat `n` (1-based) to a direction in hundredths of a degree, or centered.
    fn set_hat(&self, n: u8, direction: Option<u16>) -> bool;
    /// Centers axes and hats, releases buttons.
    fn reset(&self) -> bool;
}

/// This platform's backend.
pub fn load() -> Result<Arc<dyn Backend>, Error> {
    #[cfg(windows)]
    return windows::load();
    #[cfg(target_os = "linux")]
    return linux::load();
    #[cfg(not(any(windows, target_os = "linux")))]
    return unsupported::load();
}

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The program's data folder (next to its settings), where a backend that keeps its own files
/// (e.g. the device layouts on Linux) stores them. Set once at start; later calls are ignored.
/// Without it a backend uses the platform's usual place.
pub fn set_data_dir(dir: &Path) {
    let _ = DATA_DIR.set(dir.to_path_buf());
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn data_dir() -> Option<PathBuf> {
    DATA_DIR.get().cloned()
}

/// The elevated side of [`Backend::apply`]: when the program was started for it (`args` without
/// the program name), applies the change and returns the exit code to quit with.
pub fn admin_entry(args: &[String]) -> Option<i32> {
    #[cfg(windows)]
    return windows::admin_entry(args);
    #[cfg(not(windows))]
    {
        let _ = args;
        None
    }
}
