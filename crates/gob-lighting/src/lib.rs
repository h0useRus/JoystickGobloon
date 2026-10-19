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

/// How an LED's two colors combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Blend {
    /// `color` only.
    #[default]
    First,
    /// `color2` only.
    Second,
    /// `color`, then `color2` (with blinking).
    Alternate,
    /// `color2`, then `color`.
    AlternateReverse,
    /// Both at once.
    Mix,
}

/// What one LED should show. Colors are 8-bit RGB; the vendor scales them to what the LED
/// can show (VKB: 8 levels per channel; see [`LedKind`] for LEDs that are not RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Led {
    pub id: u8,
    /// 0: the whole LED. 1, 2: one element of a LED number that drives two single-color
    /// LEDs (VKB base: 1 blue, 2 red); its brightness is the red channel of `color`, and
    /// the other element keeps its state.
    pub element: u8,
    pub color: [u8; 3],
    pub color2: [u8; 3],
    pub blend: Blend,
    pub mode: Mode,
}

/// What a known LED is, for editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedKind {
    Rgb,
    /// One color; its brightness is the red channel of `color`.
    Single,
}

/// A LED a device is known to have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownLed {
    pub id: u8,
    /// See [`Led::element`].
    pub element: u8,
    pub kind: LedKind,
}

/// LED state saved by [`Device::save`]: what this program can read back and write again
/// (VKB: the list set with report `0x59`, not the colors of the device's own profile).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Saved {
    Vkb(Vec<vkb::LedConfig>),
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

    /// LEDs this device is known to have; empty when unknown (any number may work).
    pub fn known_leds(&self) -> &'static [KnownLed] {
        match self.vendor {
            Vendor::Vkb => vkb::KNOWN_LEDS,
        }
    }

    /// Whether this is the device DirectInput reports with `hid_path` (gob-input's
    /// `DeviceInfo::hid_path`); paths differ only in case, if at all.
    pub fn is_at(&self, hid_path: &str) -> bool {
        !hid_path.is_empty() && self.hid.path.eq_ignore_ascii_case(hid_path)
    }

    /// Sets these LEDs (later ones win for the same id), leaving the others as they are.
    pub fn set(&self, leds: &[Led]) -> io::Result<()> {
        match self.vendor {
            Vendor::Vkb => vkb::set(&self.hid, leds),
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

    /// Reads the LED state to put back later with [`Device::restore`].
    pub fn save(&self) -> io::Result<Saved> {
        match self.vendor {
            Vendor::Vkb => vkb::read(&self.hid).map(Saved::Vkb),
        }
    }

    /// Puts back a state from [`Device::save`].
    pub fn restore(&self, saved: &Saved) -> io::Result<()> {
        match (self.vendor, saved) {
            (Vendor::Vkb, Saved::Vkb(list)) => vkb::write(&self.hid, list),
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
