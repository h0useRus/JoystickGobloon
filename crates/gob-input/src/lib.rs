//! Physical game controller input via DirectInput 8.
//!
//! Devices are identified by their DirectInput instance GUID, the same id
//! Joystick Gremlin stores in profiles. A background thread polls every
//! attached controller, keeps a shared snapshot for the GUI and emits change
//! events for the engine.

#[cfg(windows)]
mod dinput;
mod keyboard;
mod service;

pub use service::InputService;

/// Number of DirectInput axis slots: X, Y, Z, Rx, Ry, Rz, Slider 0, Slider 1.
pub const AXIS_SLOTS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
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
}

impl DeviceInfo {
    /// vJoy virtual devices (our own output), as opposed to physical controllers.
    pub fn is_vjoy(&self) -> bool {
        self.vendor_id == 0x1234 && self.product_id == 0xBEAD
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceState {
    pub info: DeviceInfo,
    /// Indexed by slot id - 1; values in -1.0..=1.0 (0.0 for missing axes).
    pub axes: [f32; AXIS_SLOTS],
    pub buttons: Vec<bool>,
    /// Hat direction in hundredths of a degree (0 = up, clockwise); `None` when centered.
    pub hats: Vec<Option<u16>>,
}

impl DeviceState {
    pub fn new(info: DeviceInfo) -> Self {
        Self {
            axes: [0.0; AXIS_SLOTS],
            buttons: vec![false; info.buttons],
            hats: vec![None; info.hats],
            info,
        }
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

/// Formats GUID parts the way Gremlin writes device ids.
pub fn format_guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> String {
    let d = data4;
    format!(
        "{data1:08x}-{data2:04x}-{data3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    )
}

/// DirectInput POV value -> direction; centered is reported as 0xFFFF in the low word.
pub fn pov_direction(raw: u32) -> Option<u16> {
    (raw & 0xFFFF != 0xFFFF).then_some((raw % 36000) as u16)
}

/// DirectInput axis value in -32768..=32767 -> -1.0..=1.0.
pub fn normalize_axis(raw: i32) -> f32 {
    ((raw as f32 + 0.5) / 32767.5).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_matches_gremlin_format() {
        let g = format_guid(
            0x7d12d5c0,
            0x43ea,
            0x11f0,
            [0x80, 0x0a, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00],
        );
        assert_eq!(g, "7d12d5c0-43ea-11f0-800a-444553540000");
    }

    #[test]
    fn pov_and_axis_conversion() {
        assert_eq!(pov_direction(0xFFFF_FFFF), None);
        assert_eq!(pov_direction(9000), Some(9000));
        assert_eq!(normalize_axis(-32768), -1.0);
        assert_eq!(normalize_axis(32767), 1.0);
        assert!(normalize_axis(0).abs() < 1e-4);
    }
}
