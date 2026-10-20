//! Windows: controllers through DirectInput 8 (ids = DirectInput instance GUIDs, like Gremlin),
//! keyboard keys through a low-level hook, controller numbers through WinMM.

mod dinput;
pub(crate) mod keyboard;

use std::cell::RefCell;
use std::time::Duration;

use windows::Win32::Devices::HumanInterfaceDevice::DIDEVICEINSTANCEW;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::WaitForMultipleObjects;

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

/// Formats GUID parts the way Gremlin writes device ids.
fn format_guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> String {
    let d = data4;
    format!(
        "{data1:08x}-{data2:04x}-{data3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    )
}

/// DirectInput POV value -> direction; centered is reported as 0xFFFF in the low word.
fn pov_direction(raw: u32) -> Option<u16> {
    (raw & 0xFFFF != 0xFFFF).then_some((raw % 36000) as u16)
}

/// DirectInput axis value in -32768..=32767 -> -1.0..=1.0.
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
    fn pov_and_axis_conversion() {
        assert_eq!(pov_direction(0xFFFF_FFFF), None);
        assert_eq!(pov_direction(9000), Some(9000));
        assert_eq!(normalize_axis(-32768), -1.0);
        assert_eq!(normalize_axis(32767), 1.0);
        assert!(normalize_axis(0).abs() < 1e-4);
    }
}
