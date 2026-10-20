//! Platforms without controller access yet (Linux gets evdev, docs/LINUX_PLAN.md phase 2): no
//! controllers, no keyboard keys.

use crate::{Calibrations, DeviceState, InputEvent};

pub(crate) struct Platform;

/// Never exists: there is nothing to open.
pub(crate) enum NoDevice {}

impl super::Backend for Platform {
    type Found = ();
    type Device = NoDevice;

    fn new() -> Option<Self> {
        None
    }

    fn attached(&self) -> Vec<(String, ())> {
        Vec::new()
    }

    fn open(&self, _: &(), _: bool) -> Option<NoDevice> {
        None
    }

    fn wait(&self, _: &[NoDevice], _: bool) {}
}

impl super::Device for NoDevice {
    fn state(&self) -> &DeviceState {
        match *self {}
    }

    fn previous(&self) -> &DeviceState {
        match *self {}
    }

    fn set_calibrations(&mut self, _: &Calibrations) {
        match *self {}
    }

    fn read(&mut self, _: &mut dyn FnMut(InputEvent)) -> bool {
        match *self {}
    }
}

pub(crate) fn joystick_ids(_vendor_id: u16, _product_id: u16) -> Vec<u32> {
    Vec::new()
}

pub(crate) mod keyboard {
    use std::sync::Arc;

    /// Reports (scan code, extended, pressed).
    pub type KeySink = Arc<dyn Fn(u16, bool, bool) + Send + Sync>;

    pub struct Hook;

    pub fn start(_sink: KeySink) -> Option<Hook> {
        None
    }
}
