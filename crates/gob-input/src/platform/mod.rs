//! Controller and keyboard access per platform behind one interface, so the input service
//! (`service.rs`) and everything above it stay the same everywhere (docs/LINUX_PLAN.md,
//! "Архитектура"). `windows/`: DirectInput, the keyboard hook and WinMM controller numbers;
//! `none.rs`: no controllers (Linux gets evdev, phase 2).

use crate::{Calibrations, DeviceState, InputEvent};

#[cfg(not(windows))]
mod none;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use none::{Platform, joystick_ids, keyboard};
#[cfg(windows)]
pub(crate) use windows::{Platform, joystick_ids, keyboard};

/// A platform's way to find, open and wait for controllers.
pub(crate) trait Backend: Sized {
    /// What [`Backend::attached`] found, to open it.
    type Found;
    type Device: Device;

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
}
