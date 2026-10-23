//! Controller and keyboard access per platform behind one interface, so the input service
//! (`service.rs`) and everything above it stay the same everywhere.
//! `windows/`: DirectInput, the keyboard hook and WinMM controller numbers;
//! `linux/`: evdev controllers and keyboards, joydev numbers; `none.rs`: no controllers.

use crate::{Calibrations, DeviceState, InputEvent};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(windows, target_os = "linux")))]
mod none;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub(crate) use linux::{Platform, joystick_ids, keyboard};
#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) use none::{Platform, joystick_ids, keyboard};
#[cfg(windows)]
pub(crate) use windows::{Platform, joystick_ids, keyboard};

/// A platform's way to find, open and wait for controllers.
pub(crate) trait Backend: Sized {
    /// What [`Backend::attached`] found, to open it.
    type Found;
    type Device: Device;
    /// Devices can be read exclusively ([`Device::set_exclusive`], crate::exclusive).
    const EXCLUSIVE: bool = false;

    /// `None` when the platform's input API is not available.
    fn new() -> Option<Self>;
    /// Attached controllers with their device ids (Gremlin's GUID format).
    fn attached(&self) -> Vec<(String, Self::Found)>;
    /// Opens a controller. `events`: it is read when it signals a change (event mode) instead
    /// of being polled.
    fn open(&self, found: &Self::Found, events: bool) -> Option<Self::Device>;
    /// Waits until a device may have changed (event mode) or the poll interval passed.
    fn wait(&self, devices: &[Self::Device], events: bool);
}

/// An open controller.
pub(crate) trait Device {
    fn state(&self) -> &DeviceState;
    /// The state before the last [`Device::read`].
    fn previous(&self) -> &DeviceState;
    /// Takes this device's axis calibrations; they replace the default scaling.
    fn set_calibrations(&mut self, calibrations: &Calibrations);
    /// Reads the device into `state` (the old one moves to `previous`). Button and hat changes
    /// the device buffered go to `emit` in order (so short presses are not lost); the caller
    /// finds the rest by comparing `previous` and `state`. False when the device is gone.
    fn read(&mut self, emit: &mut dyn FnMut(InputEvent)) -> bool;
    /// Takes the device for this process alone, or lets go of it (only with
    /// [`Backend::EXCLUSIVE`]).
    fn set_exclusive(&mut self, _on: bool) {}
}

/// Formats GUID parts the way Gremlin writes device ids.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
fn format_guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> String {
    let d = data4;
    format!(
        "{data1:08x}-{data2:04x}-{data3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    )
}

/// An axis value in DirectInput's -32768..=32767 -> -1.0..=1.0.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
fn normalize_axis(raw: i32) -> f32 {
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
    fn axis_normalization() {
        assert_eq!(normalize_axis(-32768), -1.0);
        assert_eq!(normalize_axis(32767), 1.0);
        assert!(normalize_axis(0).abs() < 1e-4);
    }
}
