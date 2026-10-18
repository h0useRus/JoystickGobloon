//! Joystick LEDs straight through HID feature reports. First (and only so far) vendor:
//! VKB (`vkb`), protocol after pyvkb; see docs/LIGHTING_PLAN.md.
//!
//! Writes are temporary: they last until the device is reconnected and change neither the
//! firmware nor its VKBDevCfg profile. LEDs not written follow the device profile.

pub mod hid;
pub mod vkb;

use std::io;

pub use vkb::{Color, ColorMode, LedConfig, LedMode};

/// A HID collection whose LEDs can be set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub hid: hid::Collection,
}

/// Present devices with LED control (VKB collections that have feature report `0x59`).
pub fn devices() -> Vec<Device> {
    hid::collections(vkb::VENDOR_ID)
        .into_iter()
        .filter(|c| c.feature_ids.contains(&vkb::REPORT_ID))
        .map(|hid| Device { hid })
        .collect()
}

impl Device {
    pub fn name(&self) -> &str {
        &self.hid.product
    }

    /// Interface path, as DirectInput reports it in `DIPROP_GUIDANDPATH`.
    pub fn path(&self) -> &str {
        &self.hid.path
    }

    fn report_len(&self) -> io::Result<usize> {
        let len = self.hid.feature_len as usize;
        if len < vkb::REPORT_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "feature report is {len} bytes, LEDs need {}",
                    vkb::REPORT_LEN
                ),
            ));
        }
        Ok(len)
    }

    /// LEDs set by a write (ours or VKBDevCfg's LED test) since the device was connected.
    pub fn read(&self) -> io::Result<Vec<LedConfig>> {
        let report = hid::get_feature(&self.hid.path, vkb::REPORT_ID, self.report_len()?)?;
        Ok(vkb::decode(&report))
    }

    /// Replaces the whole list of set LEDs (empty = all LEDs back to the device profile).
    pub fn write(&self, configs: &[LedConfig]) -> io::Result<()> {
        let len = self.report_len()?;
        let report = vkb::encode(configs, vkb::random_bytes()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("at most {} LEDs at once", vkb::MAX_CONFIGS - 1),
            )
        })?;
        let mut buffer = vec![0u8; len];
        buffer[..report.len()].copy_from_slice(&report);
        hid::set_feature(&self.hid.path, &buffer)
    }

    /// Sets one LED, keeping the others that were set.
    pub fn set(&self, config: LedConfig) -> io::Result<()> {
        self.write(&vkb::merge(&self.read()?, config))
    }

    /// Returns one LED to the device profile.
    pub fn release(&self, led: u8) -> io::Result<()> {
        let current = self.read()?;
        if current.iter().any(|c| c.led == led) {
            self.write(&vkb::remove(&current, led))
        } else {
            Ok(())
        }
    }

    /// Returns every LED to the device profile.
    pub fn release_all(&self) -> io::Result<()> {
        self.write(&[])
    }
}
