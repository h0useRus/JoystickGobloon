//! Windows: controllers through DirectInput 8 (ids = DirectInput instance GUIDs, like Gremlin),
//! Xbox controllers through XInput under the same ids (`xinput.rs`), keyboard keys through a
//! low-level hook, controller numbers through WinMM.

mod dinput;
pub(crate) mod keyboard;
mod xinput;

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
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
    /// `None` without xinput1_4.dll: Xbox controllers are read through DirectInput then.
    xinput: Option<Arc<xinput::XInput>>,
    /// XInput slots read by open controllers.
    slots: Arc<Mutex<HashSet<u32>>>,
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
    type Device = Controller;

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
            xinput: xinput::XInput::load().map(Arc::new),
            slots: Arc::default(),
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

    fn open(&self, found: &DIDEVICEINSTANCEW, events: bool) -> Option<Controller> {
        let di = self.di.open(found, events).ok()?;
        let pad = self
            .xinput
            .as_ref()
            .filter(|_| xinput::is_xinput_path(&di.state.info.hid_path))
            .map(|x| xinput::Pad::new(x.clone(), self.slots.clone(), &di.state.info));
        Some(Controller { di, pad })
    }

    fn wait(&self, devices: &[Controller], events: bool, poll: Duration) {
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
                .filter(|d| super::Device::signals(*d))
                .filter_map(|d| d.di.event())
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

/// A controller: its DirectInput device, and for an Xbox controller its XInput side, which
/// then gives the state.
pub(crate) struct Controller {
    di: dinput::Device,
    pad: Option<xinput::Pad>,
}

impl super::Device for Controller {
    fn state(&self) -> &DeviceState {
        self.pad.as_ref().map_or(&self.di.state, |p| &p.state)
    }

    fn previous(&self) -> &DeviceState {
        self.pad.as_ref().map_or(&self.di.previous, |p| &p.previous)
    }

    fn set_calibrations(&mut self, calibrations: &Calibrations) {
        self.di.use_calibrations(calibrations);
        if let Some(pad) = &mut self.pad {
            pad.set_calibrations(calibrations);
        }
    }

    /// An Xbox controller read through XInput is polled.
    fn signals(&self) -> bool {
        self.di.event().is_some() && self.pad.as_ref().is_none_or(|p| p.slot().is_none())
    }

    fn read(&mut self, emit: &mut dyn FnMut(InputEvent)) -> bool {
        let di = &mut self.di;
        match &mut self.pad {
            // DirectInput's state only while the XInput slot is not found yet.
            Some(pad) => pad.read(|| di.poll().then(|| di.state.clone())),
            None if di.event().is_some() => di.read_buffered(emit),
            None => di.poll(),
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
