//! Windows: controllers through DirectInput 8 (ids = DirectInput instance GUIDs, like Gremlin),
//! keyboard keys through a low-level hook, controller numbers through WinMM.

mod dinput;
pub(crate) mod keyboard;

use std::cell::RefCell;
use std::time::Duration;

use windows::Win32::Devices::HumanInterfaceDevice::DIDEVICEINSTANCEW;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::WaitForMultipleObjects;

use super::{format_guid, normalize_axis};
use crate::{Calibrations, DeviceState, InputEvent};

/// Polling mode: time between two reads of every device.
const POLL_INTERVAL: Duration = Duration::from_millis(4);
/// Event mode: devices are also read this often without a change signal, as a fallback.
const EVENT_TIMEOUT: Duration = Duration::from_millis(20);
/// WaitForMultipleObjects takes at most this many handles.
const MAX_WAIT: usize = 64;

pub(crate) struct Platform {
    di: dinput::DirectInput,
    /// Reused by `wait`, so waiting does not allocate.
    handles: RefCell<Vec<HANDLE>>,
}

impl super::Backend for Platform {
    type Found = DIDEVICEINSTANCEW;
    type Device = dinput::Device;

    fn new() -> Option<Self> {
        let di = dinput::DirectInput::new().ok()?;
        Some(Self {
            di,
            handles: RefCell::default(),
        })
    }

    fn attached(&self) -> Vec<(String, DIDEVICEINSTANCEW)> {
        self.di
            .enumerate()
            .into_iter()
            .map(|inst| {
                let g = inst.guidInstance;
                (format_guid(g.data1, g.data2, g.data3, g.data4), inst)
            })
            .collect()
    }

    fn open(&self, found: &DIDEVICEINSTANCEW, events: bool) -> Option<dinput::Device> {
        self.di.open(found, events).ok()
    }

    fn wait(&self, devices: &[dinput::Device], events: bool) {
        let mut handles = self.handles.borrow_mut();
        handles.clear();
        handles.extend(
            devices
                .iter()
                .filter_map(dinput::Device::event)
                .take(MAX_WAIT),
        );
        // A device without an event (polling mode, or it refused one) needs the short interval.
        let all_signal = !devices.is_empty() && handles.len() == devices.len();
        if events && all_signal {
            // SAFETY: the handles belong to `devices`, which stay open during the wait.
            unsafe {
                WaitForMultipleObjects(&handles, false, EVENT_TIMEOUT.as_millis() as u32);
            }
        } else if events && devices.is_empty() {
            std::thread::sleep(EVENT_TIMEOUT);
        } else {
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

impl super::Device for dinput::Device {
    fn state(&self) -> &DeviceState {
        &self.state
    }

    fn previous(&self) -> &DeviceState {
        &self.previous
    }

    fn set_calibrations(&mut self, calibrations: &Calibrations) {
        self.use_calibrations(calibrations);
    }

    fn read(&mut self, emit: &mut dyn FnMut(InputEvent)) -> bool {
        if self.event().is_some() {
            self.read_buffered(emit)
        } else {
            self.poll()
        }
    }
}

/// Windows game controller numbers (WinMM ids, as joy.cpl and many games use them) of the
/// connected controllers with this vendor and product id.
pub(crate) fn joystick_ids(vendor_id: u16, product_id: u16) -> Vec<u32> {
    use windows::Win32::Media::Multimedia::{
        JOY_RETURNBUTTONS, JOYCAPSW, JOYINFOEX, joyGetDevCapsW, joyGetNumDevs, joyGetPosEx,
    };
    let mut ids = Vec::new();
    // SAFETY: plain WinMM queries into locally owned structs of the right size.
    unsafe {
        for id in 0..joyGetNumDevs() {
            let mut caps = JOYCAPSW::default();
            if joyGetDevCapsW(
                id as usize,
                &mut caps,
                std::mem::size_of::<JOYCAPSW>() as u32,
            ) != 0
            {
                continue;
            }
            let mut info = JOYINFOEX {
                dwSize: std::mem::size_of::<JOYINFOEX>() as u32,
                dwFlags: JOY_RETURNBUTTONS as u32,
                ..Default::default()
            };
            let connected = joyGetPosEx(id, &mut info) == 0;
            if connected && caps.wMid == vendor_id && caps.wPid == product_id {
                ids.push(id);
            }
        }
    }
    ids
}

/// DirectInput POV value -> direction; centered is reported as 0xFFFF in the low word.
fn pov_direction(raw: u32) -> Option<u16> {
    (raw & 0xFFFF != 0xFFFF).then_some((raw % 36000) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pov_conversion() {
        assert_eq!(pov_direction(0xFFFF_FFFF), None);
        assert_eq!(pov_direction(9000), Some(9000));
    }
}
