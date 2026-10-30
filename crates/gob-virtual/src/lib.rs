//! Virtual joysticks and gamepads: the common interface the engine and the GUI use, whatever
//! provides the devices. Each platform has its own backend: vJoy on
//! Windows (`windows.rs`, over gob-vjoy), uinput on Linux (`linux.rs`, over gob-uinput),
//! `unsupported.rs` elsewhere.
//! A new backend (e.g. our own Windows driver) only implements [`Backend`] and [`Device`].
//!
//! Device ids are 1-based like Gremlin's `vjoy-device`; axes use slots 1..=8 (X, Y, Z, Rx, Ry, Rz,
//! Slider 0, Slider 1) like gob-input. Gamepads (an Xbox-style layout, where the backend has
//! them: [`Capabilities::max_gamepads`]) have ids of their own, 1-based as well.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "linux")]
mod linux;

/// Linux: the uinput devices as this crate makes them, for SystemGobloon, which keeps them (same
/// names, ids, layouts and `virtual.gob` format) and lends them to the program.
#[cfg(target_os = "linux")]
pub mod uinput {
    pub use crate::linux::service::{joystick_config, joystick_record};
    pub use crate::linux::{
        Layout, changed, create_gamepad, create_joystick, gamepad_reset, joystick_reset, parse,
        to_text,
    };
}
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
    /// Applying a configuration change asks the user for administrator rights.
    pub needs_admin: bool,
    /// Virtual gamepads it can create; 0 = none.
    pub max_gamepads: u32,
    /// Each joystick can have its own name and product id ([`DeviceConfig::name`],
    /// [`DeviceConfig::product`]); vJoy names them all alike.
    pub custom_names: bool,
}

/// No backend: nothing can be configured.
pub const NO_CAPABILITIES: Capabilities = Capabilities {
    name: "",
    max_devices: 0,
    max_gamepads: 0,
    max_buttons: 0,
    max_hats: 0,
    effects: &[],
    switchable: false,
    needs_admin: false,
    custom_names: false,
};

/// Vendor id of our virtual joysticks (vJoy's too).
pub const VENDOR: u16 = 0x1234;
/// Product ids our virtual joysticks can have: each its own, so games (which keep bindings by
/// the DirectInput product GUID or the SDL GUID, both made from vendor and product) tell them
/// apart. vJoy's single id [`VJOY_PRODUCT`] lies inside but is never given out.
pub const PRODUCTS: std::ops::RangeInclusive<u16> = 0xBE01..=0xBEFF;
pub const VJOY_PRODUCT: u16 = 0xBEAD;
/// Longest name in bytes (uinput keeps 79).
pub const MAX_NAME: usize = 79;

/// Whether `product` may be set by hand.
pub fn product_allowed(product: u16) -> bool {
    PRODUCTS.contains(&product) && product != VJOY_PRODUCT
}

/// Whether `name` may be a device name: not empty, at most [`MAX_NAME`] bytes, no control
/// characters, no space around it.
pub fn name_allowed(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name.trim() == name
        && !name.chars().any(char::is_control)
}

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
    /// The name games see; `None` = the backend's ("JoyGob N"). Only where
    /// [`Capabilities::custom_names`].
    pub name: Option<String>,
    /// Product id; `None` = [`DeviceConfig::default_product`]. Only where
    /// [`Capabilities::custom_names`].
    pub product: Option<u16>,
}

impl DeviceConfig {
    /// Defaults for a new device: every axis (Gremlin profiles expect them), as many buttons
    /// and continuous hats as the backend has (vJoy: 128 and 4), no force feedback.
    pub fn new(id: u32, capabilities: &Capabilities) -> Self {
        Self {
            id,
            axes: [true; AXIS_SLOTS],
            buttons: capabilities.max_buttons,
            hats: capabilities.max_hats,
            discrete_hats: false,
            effects: vec![false; capabilities.effects.len()],
            name: None,
            product: None,
        }
    }

    /// The product id device `id` gets unless one is set: fixed by its number, so it stays
    /// when the layout changes.
    pub fn default_product(id: u32) -> u16 {
        let p = 0xBE00 + id.min(0xFF) as u16;
        if p == VJOY_PRODUCT { 0xBEFF } else { p }
    }

    pub fn product_id(&self) -> u16 {
        self.product.unwrap_or(Self::default_product(self.id))
    }

    /// The name device `id` has unless one is set.
    pub fn default_name(id: u32) -> String {
        format!("JoyGob {id}")
    }
}

/// What a device name is suggested from: the controller that maps the most onto it, or else
/// its layout.
pub fn suggested_name(config: &DeviceConfig, source: Option<&str>, taken: &[&str]) -> String {
    let base = match source.map(str::trim).filter(|s| !s.is_empty()) {
        Some(device) => format!("Gobloon {device}"),
        None => {
            let axes = |slots: &[usize]| slots.iter().filter(|&&i| config.axes[i]).count();
            let kind = if axes(&[0, 1]) == 2 {
                "Stick"
            } else if axes(&[2, 6, 7]) > 0 {
                "Throttle"
            } else if config.buttons > 0 || config.hats > 0 {
                "Panel"
            } else {
                "Stick"
            };
            format!("Gobloon {kind}")
        }
    };
    let base = cut(&base, MAX_NAME - 3);
    let free = |name: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(name));
    if source.is_some() && free(base) {
        return base.to_string();
    }
    (1..)
        .map(|n| format!("{base} {n}"))
        .find(|name| free(name))
        .unwrap_or_default()
}

/// `s` cut to at most `max` bytes at a character boundary, without trailing space.
fn cut(s: &str, max: usize) -> &str {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].trim_end()
}

/// One configuration change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Create or replace a device.
    Set(DeviceConfig),
    Delete(u32),
    /// Switch the whole backend on or off (only when [`Capabilities::switchable`]).
    Enable(bool),
    /// Add gamepad `id` (only up to [`Capabilities::max_gamepads`]).
    AddGamepad(u32),
    DeleteGamepad(u32),
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
    /// Every configured gamepad.
    fn gamepads(&self) -> Vec<GamepadInfo> {
        Vec::new()
    }
    /// Takes gamepad `id` until the returned handle is dropped (reset and released then).
    fn acquire_gamepad(&self, id: u32) -> Result<Box<dyn Gamepad>, Error> {
        let _ = id;
        Err(Error::Unsupported)
    }
}

/// Which inputs a virtual gamepad's controls are when the gamepad is read back as an input
/// device (e.g. in the input viewer).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GamepadInputs {
    /// (axis slot 1..=8, stick or trigger).
    pub axes: Vec<(u8, GamepadControl)>,
    /// (button number from 1, button).
    pub buttons: Vec<(u32, GamepadControl)>,
    /// Hat number of the d-pad.
    pub dpad: Option<u32>,
}

/// A configured virtual gamepad; its layout is fixed (Xbox-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GamepadInfo {
    pub id: u32,
    pub status: Status,
}

/// What a gamepad control takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Button,
    /// -1.0..=1.0; Y is negative up, like the input axes.
    Stick,
    /// 0.0..=1.0.
    Trigger,
    /// A direction, like a hat.
    Dpad,
}

/// A control of the Xbox-style gamepad. [`GamepadControl::name`] is how profiles name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadControl {
    A,
    B,
    X,
    Y,
    Lb,
    Rb,
    Back,
    Start,
    Guide,
    /// Pressing the left stick.
    LeftStick,
    RightStick,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftX,
    LeftY,
    RightX,
    RightY,
    Lt,
    Rt,
    /// The whole d-pad, set from a hat.
    Dpad,
}

impl GamepadControl {
    pub const ALL: [Self; 22] = [
        Self::A,
        Self::B,
        Self::X,
        Self::Y,
        Self::Lb,
        Self::Rb,
        Self::Back,
        Self::Start,
        Self::Guide,
        Self::LeftStick,
        Self::RightStick,
        Self::DpadUp,
        Self::DpadDown,
        Self::DpadLeft,
        Self::DpadRight,
        Self::LeftX,
        Self::LeftY,
        Self::RightX,
        Self::RightY,
        Self::Lt,
        Self::Rt,
        Self::Dpad,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::X => "x",
            Self::Y => "y",
            Self::Lb => "lb",
            Self::Rb => "rb",
            Self::Back => "back",
            Self::Start => "start",
            Self::Guide => "guide",
            Self::LeftStick => "ls",
            Self::RightStick => "rs",
            Self::DpadUp => "dpad-up",
            Self::DpadDown => "dpad-down",
            Self::DpadLeft => "dpad-left",
            Self::DpadRight => "dpad-right",
            Self::LeftX => "left-x",
            Self::LeftY => "left-y",
            Self::RightX => "right-x",
            Self::RightY => "right-y",
            Self::Lt => "lt",
            Self::Rt => "rt",
            Self::Dpad => "dpad",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    pub fn kind(self) -> ControlKind {
        match self {
            Self::LeftX | Self::LeftY | Self::RightX | Self::RightY => ControlKind::Stick,
            Self::Lt | Self::Rt => ControlKind::Trigger,
            Self::Dpad => ControlKind::Dpad,
            _ => ControlKind::Button,
        }
    }
}

/// An acquired virtual gamepad. Reset and released when dropped.
pub trait Gamepad: Send {
    /// Presses or releases a button or one d-pad direction (other controls: `false`).
    fn set_button(&self, control: GamepadControl, pressed: bool) -> bool;
    /// Moves a stick axis (-1.0..=1.0) or a trigger (0.0..=1.0).
    fn set_axis(&self, control: GamepadControl, value: f32) -> bool;
    /// Points the d-pad: hundredths of a degree (0 = up, clockwise), `None` = released.
    fn set_dpad(&self, direction: Option<u16>) -> bool;
    /// Centers sticks and d-pad, releases buttons and triggers.
    fn reset(&self) -> bool;
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

/// What this platform's backend can do, without loading it (e.g. its name for the About page).
pub fn capabilities() -> Capabilities {
    #[cfg(windows)]
    return windows::CAPABILITIES;
    #[cfg(target_os = "linux")]
    return linux::CAPABILITIES;
    #[cfg(not(any(windows, target_os = "linux")))]
    return NO_CAPABILITIES;
}

/// How the input device named `device_name` maps to gamepad controls, when it is one of this
/// backend's virtual gamepads.
pub fn gamepad_inputs(device_name: &str) -> Option<GamepadInputs> {
    #[cfg(target_os = "linux")]
    return linux::gamepad_inputs(device_name);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = device_name;
        None
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamepad_control_names() {
        for c in GamepadControl::ALL {
            assert_eq!(GamepadControl::parse(c.name()), Some(c));
        }
        assert_eq!(
            GamepadControl::parse(" left-y "),
            Some(GamepadControl::LeftY)
        );
        assert_eq!(GamepadControl::parse("z"), None);
        assert_eq!(GamepadControl::Lt.kind(), ControlKind::Trigger);
        assert_eq!(GamepadControl::DpadUp.kind(), ControlKind::Button);
    }

    #[test]
    fn products_and_names() {
        assert_eq!(DeviceConfig::default_product(1), 0xBE01);
        assert_eq!(DeviceConfig::default_product(16), 0xBE10);
        assert_ne!(DeviceConfig::default_product(0xAD), VJOY_PRODUCT);
        let mut c = DeviceConfig::new(2, &NO_CAPABILITIES);
        assert_eq!(c.product_id(), 0xBE02);
        c.product = Some(0xBE42);
        assert_eq!(c.product_id(), 0xBE42);
        assert!(product_allowed(0xBE01) && product_allowed(0xBEFF));
        assert!(!product_allowed(0xBEAD) && !product_allowed(0xBE00));
        assert!(name_allowed("Gobloon Throttle"));
        assert!(!name_allowed("") && !name_allowed(" x") && !name_allowed("a	b"));
        assert!(!name_allowed(&"x".repeat(MAX_NAME + 1)));
    }

    #[test]
    fn suggested_names() {
        let caps = Capabilities {
            max_buttons: 32,
            max_hats: 1,
            ..NO_CAPABILITIES
        };
        let stick = DeviceConfig::new(1, &caps);
        assert_eq!(
            suggested_name(&stick, Some("VKB Gladiator EVO L "), &[]),
            "Gobloon VKB Gladiator EVO L"
        );
        assert_eq!(
            suggested_name(
                &stick,
                Some("VKB Gladiator EVO L"),
                &["gobloon vkb gladiator evo l"]
            ),
            "Gobloon VKB Gladiator EVO L 1",
            "taken by another device"
        );
        assert_eq!(suggested_name(&stick, None, &[]), "Gobloon Stick 1");
        assert_eq!(
            suggested_name(&stick, None, &["Gobloon Stick 1"]),
            "Gobloon Stick 2"
        );
        let mut throttle = stick.clone();
        throttle.axes = [false, false, true, false, false, false, true, false];
        assert_eq!(suggested_name(&throttle, None, &[]), "Gobloon Throttle 1");
        let mut panel = stick.clone();
        panel.axes = [false; AXIS_SLOTS];
        assert_eq!(suggested_name(&panel, None, &[]), "Gobloon Panel 1");
        let long = "ж".repeat(60);
        let name = suggested_name(&stick, Some(&long), &[]);
        assert!(name.len() <= MAX_NAME && name_allowed(&name), "{name}");
    }
}
