//! Windows: controllers through DirectInput 8 (ids = DirectInput instance GUIDs, like Gremlin),
//! keyboard keys through a low-level hook, controller numbers through WinMM.

mod dinput;
pub(crate) mod keyboard;

use std::cell::RefCell;
use std::time::Duration;

use windows::Win32::Devices::HumanInterfaceDevice::DIDEVICEINSTANCEW;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, SetWaitableTimer,
    TIMER_ALL_ACCESS, WaitForMultipleObjects,
};
use windows::core::PCWSTR;

use super::{format_guid, normalize_axis};
use crate::{Calibrations, DeviceState, InputEvent};

/// Event mode: devices are also read this often without a change signal, as a fallback.
const EVENT_TIMEOUT: Duration = Duration::from_millis(20);
/// WaitForMultipleObjects takes at most this many handles.
const MAX_WAIT: usize = 64;

pub(crate) struct Platform {
    di: dinput::DirectInput,
    /// Reused by `wait`, so waiting does not allocate.
    handles: RefCell<Vec<HANDLE>>,
    /// High resolution timer that paces the polled devices while waiting for the others'
    /// change signals; `None` before Windows 10 1803.
    timer: Option<HANDLE>,
}

impl Drop for Platform {
    fn drop(&mut self) {
        if let Some(timer) = self.timer {
            // SAFETY: created in `new` and only closed here.
            let _ = unsafe { CloseHandle(timer) };
        }
    }
}

impl super::Backend for Platform {
    type Found = DIDEVICEINSTANCEW;
    type Device = dinput::Device;

    fn new() -> Option<Self> {
        let di = dinput::DirectInput::new().ok()?;
        // SAFETY: an unnamed timer without security attributes, closed in Drop.
        let timer = unsafe {
            CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
        }
        .ok();
        Some(Self {
            di,
            handles: RefCell::default(),
            timer,
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

    fn wait(&self, devices: &[dinput::Device], events: bool, poll: Duration) {
        if !events {
            std::thread::sleep(poll);
            return;
        }
        if devices.is_empty() {
            std::thread::sleep(EVENT_TIMEOUT);
            return;
        }
        let mut handles = self.handles.borrow_mut();
        handles.clear();
        handles.extend(
            devices
                .iter()
                .filter_map(dinput::Device::event)
                .take(MAX_WAIT - 1),
        );
        if handles.is_empty() {
            std::thread::sleep(poll);
            return;
        }
        let mut timeout = EVENT_TIMEOUT.as_millis() as u32;
        if handles.len() < devices.len() {
            // Some devices refused a change signal: wake for the signals and, every `poll`,
            // for those devices. The timer is precise; a wait timeout is not (~15 ms steps).
            let due = -((poll.as_nanos() / 100) as i64);
            // SAFETY: our own timer, relative due time in 100 ns units, no completion routine.
            let armed = self
                .timer
                .filter(|&t| unsafe { SetWaitableTimer(t, &due, 0, None, None, false) }.is_ok());
            match armed {
                Some(timer) => handles.push(timer),
                None => timeout = (poll.as_millis() as u32).max(1),
            }
        }
        // SAFETY: the handles belong to `devices` (open during the wait) and to `self`.
        unsafe {
            WaitForMultipleObjects(&handles, false, timeout);
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

    fn signals(&self) -> bool {
        self.event().is_some()
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
