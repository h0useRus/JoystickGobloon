//! Physical game controller and keyboard input: DirectInput 8 on Windows, evdev on Linux.
//!
//! Devices are identified by GUID-formatted ids: on Windows the DirectInput instance GUID, the
//! same id Joystick Gremlin stores in profiles. A background thread reads every attached
//! controller, keeps a shared snapshot for the GUI and emits change events for the engine.

pub mod exclusive;
mod platform;
mod service;

pub use service::{
    Calibrations, DEFAULT_POLL_INTERVAL, InputHandle, InputService, MAX_POLL_INTERVAL,
    MIN_POLL_INTERVAL,
};

/// Number of DirectInput axis slots: X, Y, Z, Rx, Ry, Rz, Slider 0, Slider 1.
pub const AXIS_SLOTS: usize = 8;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeviceInfo {
    /// Instance GUID formatted like Gremlin: lowercase, no braces.
    pub guid: String,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// Present axes as 1-based slot ids (1 = X ... 8 = Slider 1), as used in profiles.
    pub axes: Vec<u8>,
    pub buttons: usize,
    pub hats: usize,
    /// HID interface path (`DIPROP_GUIDANDPATH`), empty when unknown. Links the device to
    /// gob-lighting's LED control (same path there, compare ignoring case).
    pub hid_path: String,
    /// Set by the backend for Joystick Gobloon's own devices that cannot be told by their ids:
    /// on Linux the uinput devices named "Joystick Gobloon ..." (a gamepad has a real Xbox
    /// controller's ids).
    pub own: bool,
}

impl DeviceInfo {
    /// Joystick Gobloon's own virtual output (vJoy devices, our uinput devices), as opposed to
    /// physical controllers.
    pub fn is_own(&self) -> bool {
        self.own || (self.vendor_id == 0x1234 && self.product_id == 0xBEAD)
    }
}

/// Per-axis calibration in raw DirectInput units (Gremlin's calibration data).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calibration {
    pub low: i32,
    pub center_low: i32,
    pub center_high: i32,
    pub high: i32,
    /// Centered axis (stick); `false` for sliders and throttles, which use low..high only.
    pub with_center: bool,
}

impl Default for Calibration {
    /// DirectInput's full range, centered.
    fn default() -> Self {
        Self {
            low: -32768,
            center_low: 0,
            center_high: 0,
            high: 32767,
            with_center: true,
        }
    }
}

impl Calibration {
    /// Raw value -> -1..1 (Gremlin's with_center_calibration / no_center_calibration).
    pub fn apply(&self, raw: i32) -> f32 {
        let v = f64::from(raw.clamp(self.low.min(self.high), self.high.max(self.low)));
        let (low, cl, ch, high) = (
            f64::from(self.low),
            f64::from(self.center_low),
            f64::from(self.center_high),
            f64::from(self.high),
        );
        let out = if self.with_center {
            if v < cl {
                if cl > low { (v - cl) / (cl - low) } else { 0.0 }
            } else if v <= ch {
                0.0
            } else if high > ch {
                (v - ch) / (high - ch)
            } else {
                0.0
            }
        } else if high > low {
            (v - low) / (high - low) * 2.0 - 1.0
        } else {
            0.0
        };
        out.clamp(-1.0, 1.0) as f32
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceState {
    pub info: DeviceInfo,
    /// Indexed by slot id - 1; values in -1.0..=1.0 (0.0 for missing axes), calibrated.
    pub axes: [f32; AXIS_SLOTS],
    /// Raw DirectInput axis values (-32768..=32767), for calibration.
    pub raw: [i32; AXIS_SLOTS],
    pub buttons: Vec<bool>,
    /// Hat direction in hundredths of a degree (0 = up, clockwise); `None` when centered.
    pub hats: Vec<Option<u16>>,
}

impl DeviceState {
    pub fn new(info: DeviceInfo) -> Self {
        Self {
            axes: [0.0; AXIS_SLOTS],
            raw: [0; AXIS_SLOTS],
            buttons: vec![false; info.buttons],
            hats: vec![None; info.hats],
            info,
        }
    }

    /// Copies the live values (not the info) from `other`, reusing this state's buffers.
    pub(crate) fn copy_readings(&mut self, other: &DeviceState) {
        self.axes = other.axes;
        self.raw = other.raw;
        self.buttons.clone_from(&other.buttons);
        self.hats.clone_from(&other.hats);
    }

    pub fn axis(&self, id: u8) -> Option<f32> {
        self.info
            .axes
            .contains(&id)
            .then(|| self.axes[usize::from(id) - 1])
    }
}

/// A single input change; ids are 1-based, as in Gremlin profiles.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    Axis {
        device: String,
        id: u8,
        value: f32,
    },
    Button {
        device: String,
        id: u16,
        pressed: bool,
    },
    Hat {
        device: String,
        id: u8,
        direction: Option<u16>,
    },
    /// A keyboard key (first press or release; auto-repeat is not reported).
    Key {
        /// Set 1 scan code.
        scan_code: u16,
        /// E0 prefix (right Ctrl/Alt, arrows, ...).
        extended: bool,
        pressed: bool,
    },
    /// Devices were attached or removed.
    DevicesChanged,
}

/// An event with the time its device was read, for measuring latency.
#[derive(Debug, Clone, PartialEq)]
pub struct Timed {
    pub event: InputEvent,
    pub read: std::time::Instant,
}

/// The system's own controller numbers of the connected controllers with this vendor and product
/// id (Windows: WinMM ids, as joy.cpl and many games use them); empty where there are none.
pub fn joystick_ids(vendor_id: u16, product_id: u16) -> Vec<u32> {
    platform::joystick_ids(vendor_id, product_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_like_gremlin() {
        let c = Calibration {
            low: -30000,
            center_low: -1000,
            center_high: 1000,
            high: 30000,
            with_center: true,
        };
        assert_eq!(c.apply(0), 0.0);
        assert_eq!(c.apply(500), 0.0, "inside the center band");
        assert_eq!(c.apply(30000), 1.0);
        assert_eq!(c.apply(-32768), -1.0, "clamped below low");
        assert!((c.apply(15500) - 0.5).abs() < 1e-3);
        let slider = Calibration {
            low: 0,
            high: 20000,
            with_center: false,
            ..c
        };
        assert_eq!(slider.apply(0), -1.0);
        assert_eq!(slider.apply(10000), 0.0);
        assert_eq!(slider.apply(25000), 1.0);
    }
}
