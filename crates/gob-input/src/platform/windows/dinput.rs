//! Thin safe wrapper over the DirectInput 8 joystick API.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Devices::HumanInterfaceDevice::{
    DI8DEVCLASS_GAMECTRL, DIDATAFORMAT, DIDEVCAPS, DIDEVICEINSTANCEW, DIDEVICEOBJECTDATA,
    DIEDFL_ATTACHEDONLY, DIJOYSTATE2, DIPH_BYOFFSET, DIPH_DEVICE, DIPROPDWORD, DIPROPGUIDANDPATH,
    DIPROPHEADER, DIPROPRANGE, DirectInput8Create, IDirectInput8W, IDirectInputDevice8W,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateEventW;
use windows::core::{BOOL, GUID, Interface};

use super::{format_guid, normalize_axis, pov_direction};
use crate::{AXIS_SLOTS, Calibration, DeviceInfo, DeviceState, InputEvent};

const DIRECTINPUT_VERSION: u32 = 0x0800;
/// `DIPROP_RANGE` is `MAKEDIPROP(4)`: a small integer passed as a GUID pointer.
const DIPROP_RANGE: *const GUID = std::ptr::without_provenance(4);
/// `DIPROP_BUFFERSIZE` is `MAKEDIPROP(1)`.
const DIPROP_BUFFERSIZE: *const GUID = std::ptr::without_provenance(1);
/// `DIPROP_GUIDANDPATH` is `MAKEDIPROP(12)`.
const DIPROP_GUIDANDPATH: *const GUID = std::ptr::without_provenance(12);
/// Byte offsets of lX, lY, lZ, lRx, lRy, lRz, rglSlider[0], rglSlider[1] in DIJOYSTATE2.
const AXIS_OFFSETS: [u32; AXIS_SLOTS] = [0, 4, 8, 12, 16, 20, 24, 28];
/// Byte offsets of rgdwPOV[0] and rgbButtons[0] in DIJOYSTATE2.
const POV_OFFSET: u32 = 32;
const BUTTON_OFFSET: u32 = 48;
/// Byte offset just past rgbButtons (velocity and force fields follow; not used).
const BUTTON_END: u32 = BUTTON_OFFSET + 128;
/// Changes DirectInput keeps per device between two reads in event mode.
const BUFFER_SIZE: u32 = 1024;

#[link(name = "dinput8", kind = "static", modifiers = "-bundle")]
unsafe extern "C" {
    /// Predefined data format from dinput8.lib describing DIJOYSTATE2.
    static c_dfDIJoystick2: DIDATAFORMAT;
}

// Axis/POV GUIDs referenced by `c_dfDIJoystick2`.
#[link(name = "dxguid", kind = "static", modifiers = "-bundle")]
unsafe extern "C" {}

pub struct DirectInput {
    di: IDirectInput8W,
}

pub struct Device {
    dev: IDirectInputDevice8W,
    pub state: DeviceState,
    /// The state before the last poll, to find what changed.
    pub previous: DeviceState,
    /// Calibration per axis slot, taken from [`crate::Calibrations`] by `set_calibrations`.
    calibration: [Option<Calibration>; AXIS_SLOTS],
    /// Event mode: signaled by DirectInput when the device changes; its changes are buffered.
    event: Option<HANDLE>,
}

impl DirectInput {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let module = GetModuleHandleW(None)?;
            let mut raw: *mut c_void = std::ptr::null_mut();
            DirectInput8Create(
                module.into(),
                DIRECTINPUT_VERSION,
                &IDirectInput8W::IID,
                &mut raw,
                None,
            )?;
            Ok(Self {
                di: IDirectInput8W::from_raw(raw),
            })
        }
    }

    /// Attached game controllers.
    pub fn enumerate(&self) -> Vec<DIDEVICEINSTANCEW> {
        let mut found: Vec<DIDEVICEINSTANCEW> = Vec::new();
        unsafe {
            let _ = self.di.EnumDevices(
                DI8DEVCLASS_GAMECTRL,
                Some(collect_device),
                &mut found as *mut _ as *mut c_void,
                DIEDFL_ATTACHEDONLY,
            );
        }
        found
    }

    /// Opens a controller. With `events`, DirectInput buffers its changes and signals
    /// [`Device::event`] when there are new ones; otherwise it is only polled.
    pub fn open(&self, inst: &DIDEVICEINSTANCEW, events: bool) -> windows::core::Result<Device> {
        unsafe {
            let mut dev = None;
            self.di.CreateDevice(&inst.guidInstance, &mut dev, None)?;
            let dev: IDirectInputDevice8W = dev.ok_or_else(windows::core::Error::empty)?;
            dev.SetDataFormat((&raw const c_dfDIJoystick2).cast_mut())?;

            // Setting the range doubles as presence detection: it fails for missing axes.
            let axes = (1..=AXIS_SLOTS as u8)
                .filter(|&id| {
                    let range = DIPROPRANGE {
                        diph: DIPROPHEADER {
                            dwSize: size_of::<DIPROPRANGE>() as u32,
                            dwHeaderSize: size_of::<DIPROPHEADER>() as u32,
                            dwObj: AXIS_OFFSETS[usize::from(id) - 1],
                            dwHow: DIPH_BYOFFSET,
                        },
                        lMin: -32768,
                        lMax: 32767,
                    };
                    dev.SetProperty(DIPROP_RANGE, (&raw const range.diph).cast_mut())
                        .is_ok()
                })
                .collect();

            let mut caps = DIDEVCAPS {
                dwSize: size_of::<DIDEVCAPS>() as u32,
                ..Default::default()
            };
            dev.GetCapabilities(&mut caps)?;
            // Buffer and event must be set before Acquire. Without them the device is polled.
            let event = if events { buffered_event(&dev) } else { None };
            if events && event.is_none() {
                log::warn!(
                    "{}: no DirectInput change events, the device is polled",
                    wide_to_string(&inst.tszProductName)
                );
            }
            let _ = dev.Acquire();

            let g = inst.guidInstance;
            let product = inst.guidProduct.data1;
            let info = DeviceInfo {
                guid: format_guid(g.data1, g.data2, g.data3, g.data4),
                name: wide_to_string(&inst.tszProductName),
                vendor_id: (product & 0xFFFF) as u16,
                product_id: (product >> 16) as u16,
                axes,
                buttons: (caps.dwButtons as usize).min(128),
                hats: (caps.dwPOVs as usize).min(4),
                hid_path: hid_path(&dev),
                own: false,
                unread_buttons: 0,
            };
            let state = DeviceState::new(info);
            Ok(Device {
                dev,
                previous: state.clone(),
                state,
                calibration: [None; AXIS_SLOTS],
                event,
            })
        }
    }
}

/// Gives the (not yet acquired) device a change buffer and an auto-reset event it signals.
/// `None` when either fails: the device is then polled.
fn buffered_event(dev: &IDirectInputDevice8W) -> Option<HANDLE> {
    let size = DIPROPDWORD {
        diph: DIPROPHEADER {
            dwSize: size_of::<DIPROPDWORD>() as u32,
            dwHeaderSize: size_of::<DIPROPHEADER>() as u32,
            dwObj: 0,
            dwHow: DIPH_DEVICE,
        },
        dwData: BUFFER_SIZE,
    };
    // SAFETY: `size` is a DIPROPDWORD with its sizes set; the device is not acquired yet.
    unsafe { dev.SetProperty(DIPROP_BUFFERSIZE, (&raw const size.diph).cast_mut()) }.ok()?;
    // SAFETY: a plain auto-reset event, closed in `Device`'s Drop (or here if refused).
    let event = unsafe { CreateEventW(None, false, false, None) }.ok()?;
    if unsafe { dev.SetEventNotification(event) }.is_err() {
        let _ = unsafe { CloseHandle(event) };
        return None;
    }
    Some(event)
}

/// The device's HID interface path, empty when DirectInput does not know it.
fn hid_path(dev: &IDirectInputDevice8W) -> String {
    let mut prop = DIPROPGUIDANDPATH {
        diph: DIPROPHEADER {
            dwSize: size_of::<DIPROPGUIDANDPATH>() as u32,
            dwHeaderSize: size_of::<DIPROPHEADER>() as u32,
            dwObj: 0,
            dwHow: DIPH_DEVICE,
        },
        ..Default::default()
    };
    // SAFETY: `prop` is a DIPROPGUIDANDPATH with its sizes set, as the property expects.
    match unsafe { dev.GetProperty(DIPROP_GUIDANDPATH, &mut prop.diph) } {
        Ok(()) => wide_to_string(&prop.wszPath),
        Err(_) => String::new(),
    }
}

impl Device {
    /// Takes this device's axis calibrations; they replace the default scaling of their axes.
    pub fn use_calibrations(&mut self, calibrations: &crate::Calibrations) {
        for (i, c) in self.calibration.iter_mut().enumerate() {
            *c = calibrations
                .get(&(self.state.info.guid.clone(), i as u8 + 1))
                .copied();
        }
    }

    /// Reads the current state into `state` (the old one moves to `previous`); returns false
    /// when the device is gone.
    pub fn poll(&mut self) -> bool {
        self.previous.copy_readings(&self.state);
        self.read_state()
    }

    /// The event DirectInput signals on changes (event mode only).
    pub fn event(&self) -> Option<HANDLE> {
        self.event
    }

    /// Event mode: sends every buffered button and hat change in order (a press shorter than
    /// the time between two reads is not lost), then reads the state like [`Device::poll`].
    /// Axes, and anything the buffer missed, are left to the `previous` -> `state` diff.
    /// Returns false when the device is gone.
    pub fn read_buffered(&mut self, emit: &mut dyn FnMut(InputEvent)) -> bool {
        self.previous.copy_readings(&self.state);
        let mut data = [DIDEVICEOBJECTDATA::default(); 64];
        loop {
            let mut count = data.len() as u32;
            // SAFETY: `data` has room for `count` DIDEVICEOBJECTDATA entries.
            let read = unsafe {
                let _ = self.dev.Poll();
                self.dev.GetDeviceData(
                    size_of::<DIDEVICEOBJECTDATA>() as u32,
                    data.as_mut_ptr(),
                    &mut count,
                    0,
                )
            };
            if read.is_err() {
                // Input lost: re-acquire, the state read below tells whether it worked.
                let _ = unsafe { self.dev.Acquire() };
                break;
            }
            for item in &data[..count as usize] {
                self.apply_buffered(item, emit);
            }
            if (count as usize) < data.len() {
                break;
            }
        }
        // Already sent: the diff must not send these again.
        self.previous.buttons.clone_from(&self.state.buttons);
        self.previous.hats.clone_from(&self.state.hats);
        self.read_state()
    }

    /// One buffered button or hat change: updates `state` and sends it if it differs.
    fn apply_buffered(&mut self, item: &DIDEVICEOBJECTDATA, emit: &mut dyn FnMut(InputEvent)) {
        let s = &mut self.state;
        let ofs = item.dwOfs;
        if (BUTTON_OFFSET..BUTTON_END).contains(&ofs) {
            let i = (ofs - BUTTON_OFFSET) as usize;
            let pressed = item.dwData & 0x80 != 0;
            if let Some(b) = s.buttons.get_mut(i)
                && *b != pressed
            {
                *b = pressed;
                emit(InputEvent::Button {
                    device: s.info.guid.clone(),
                    id: i as u16 + 1,
                    pressed,
                });
            }
        } else if (POV_OFFSET..BUTTON_OFFSET).contains(&ofs) {
            let i = ((ofs - POV_OFFSET) / 4) as usize;
            let direction = pov_direction(item.dwData);
            if let Some(h) = s.hats.get_mut(i)
                && *h != direction
            {
                *h = direction;
                emit(InputEvent::Hat {
                    device: s.info.guid.clone(),
                    id: i as u8 + 1,
                    direction,
                });
            }
        }
    }

    /// Reads the whole state into `state`; returns false when the device is gone.
    fn read_state(&mut self) -> bool {
        let mut raw = DIJOYSTATE2::default();
        let read = |dev: &IDirectInputDevice8W, raw: &mut DIJOYSTATE2| unsafe {
            let _ = dev.Poll();
            dev.GetDeviceState(
                size_of::<DIJOYSTATE2>() as u32,
                raw as *mut _ as *mut c_void,
            )
        };
        if read(&self.dev, &mut raw).is_err() {
            // Input lost (e.g. focus or power change): re-acquire once before giving up.
            if unsafe { self.dev.Acquire() }.is_err() || read(&self.dev, &mut raw).is_err() {
                return false;
            }
        }

        let values = [
            raw.lX,
            raw.lY,
            raw.lZ,
            raw.lRx,
            raw.lRy,
            raw.lRz,
            raw.rglSlider[0],
            raw.rglSlider[1],
        ];
        let s = &mut self.state;
        for &id in &s.info.axes {
            let i = usize::from(id) - 1;
            s.raw[i] = values[i];
            s.axes[i] = match &self.calibration[i] {
                Some(c) => c.apply(values[i]),
                None => normalize_axis(values[i]),
            };
        }
        for (i, b) in s.buttons.iter_mut().enumerate() {
            *b = raw.rgbButtons[i] & 0x80 != 0;
        }
        for (i, h) in s.hats.iter_mut().enumerate() {
            *h = pov_direction(raw.rgdwPOV[i]);
        }
        true
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            let _ = self.dev.Unacquire();
            if let Some(event) = self.event {
                let _ = self.dev.SetEventNotification(HANDLE::default());
                let _ = CloseHandle(event);
            }
        }
    }
}

unsafe extern "system" fn collect_device(inst: *mut DIDEVICEINSTANCEW, ctx: *mut c_void) -> BOOL {
    unsafe {
        let found = &mut *(ctx as *mut Vec<DIDEVICEINSTANCEW>);
        found.push(*inst);
        BOOL(1) // DIENUM_CONTINUE
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}
