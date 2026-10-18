//! Joystick LEDs. Vendor-neutral API here; each vendor's protocol/SDK lives in its own file
//! (`vkb.rs`, later others) and `Device` dispatches on `Vendor`. See docs/LIGHTING_PLAN.md.
//!
//! Writes are temporary: they last until the device is reconnected and change neither the
//! firmware nor the vendor's own profile. A written LED keeps its last state until reconnect,
//! so the profile sets the colors it needs when it starts.

pub mod hid;
pub mod vkb;

use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Vkb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Off,
    #[default]
    On,
    SlowBlink,
    FastBlink,
    FastestBlink,
}

/// What one LED should show. `color` is 8-bit RGB; the vendor scales it to what the LED
/// can show (VKB: 8 levels per channel, single-color LEDs use red).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Led {
    pub id: u8,
    pub color: [u8; 3],
    pub mode: Mode,
}

/// A device whose LEDs can be set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub vendor: Vendor,
    pub hid: hid::Collection,
}

/// Present devices with LED control, every vendor.
pub fn devices() -> Vec<Device> {
    vkb::devices()
}

impl Device {
    pub fn name(&self) -> &str {
        &self.hid.product
    }

    /// HID interface path, as DirectInput reports it in `DIPROP_GUIDANDPATH`.
    pub fn path(&self) -> &str {
        &self.hid.path
    }

    /// Whether this is the device DirectInput reports with `hid_path` (gob-input's
    /// `DeviceInfo::hid_path`); paths differ only in case, if at all.
    pub fn is_at(&self, hid_path: &str) -> bool {
        !hid_path.is_empty() && self.hid.path.eq_ignore_ascii_case(hid_path)
    }

    /// Sets one LED, leaving the others as they are.
    pub fn set(&self, led: Led) -> io::Result<()> {
        match self.vendor {
            Vendor::Vkb => vkb::set(&self.hid, led),
        }
    }

    /// Stops controlling one LED (VKB: it keeps its last state until reconnect).
    pub fn release(&self, id: u8) -> io::Result<()> {
        match self.vendor {
            Vendor::Vkb => vkb::release(&self.hid, id),
        }
    }

    /// Stops controlling every LED.
    pub fn release_all(&self) -> io::Result<()> {
        match self.vendor {
            Vendor::Vkb => vkb::write(&self.hid, &[]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_directinput_path() {
        let device = Device {
            vendor: Vendor::Vkb,
            hid: hid::Collection {
                path: r"\\?\hid#vid_231d&pid_0200#a&2f28a505&1&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}".into(),
                vendor_id: vkb::VENDOR_ID,
                product_id: 0x0200,
                product: "VKBsim Gladiator EVO R".into(),
                usage_page: 1,
                usage: 4,
                feature_len: 129,
                feature_ids: vec![vkb::REPORT_ID],
            },
        };
        assert!(device.is_at(
            r"\\?\HID#VID_231D&PID_0200#A&2F28A505&1&0000#{4D1E55B2-F16F-11CF-88CB-001111000030}"
        ));
        assert!(!device.is_at(
            r"\\?\hid#vid_231d&pid_3201#a&3985aa8&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"
        ));
        assert!(!device.is_at(""));
    }
}
