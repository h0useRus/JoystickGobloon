//! Thin safe wrapper over the DirectInput 8 joystick API.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Devices::HumanInterfaceDevice::{
    DI8DEVCLASS_GAMECTRL, DIDATAFORMAT, DIDEVCAPS, DIDEVICEINSTANCEW, DIEDFL_ATTACHEDONLY,
    DIJOYSTATE2, DIPH_BYOFFSET, DIPH_DEVICE, DIPROPGUIDANDPATH, DIPROPHEADER, DIPROPRANGE,
    DirectInput8Create, IDirectInput8W, IDirectInputDevice8W,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::{BOOL, GUID, Interface};

use crate::{
    AXIS_SLOTS, Calibration, DeviceInfo, DeviceState, format_guid, normalize_axis, pov_direction,
};

const DIRECTINPUT_VERSION: u32 = 0x0800;
/// `DIPROP_RANGE` is `MAKEDIPROP(4)`: a small integer passed as a GUID pointer.
const DIPROP_RANGE: *const GUID = std::ptr::without_provenance(4);
/// `DIPROP_GUIDANDPATH` is `MAKEDIPROP(12)`.
const DIPROP_GUIDANDPATH: *const GUID = std::ptr::without_provenance(12);
/// Byte offsets of lX, lY, lZ, lRx, lRy, lRz, rglSlider[0], rglSlider[1] in DIJOYSTATE2.
const AXIS_OFFSETS: [u32; AXIS_SLOTS] = [0, 4, 8, 12, 16, 20, 24, 28];

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

    pub fn open(&self, inst: &DIDEVICEINSTANCEW) -> windows::core::Result<Device> {
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
            };
            let state = DeviceState::new(info);
            Ok(Device {
                dev,
                previous: state.clone(),
                state,
                calibration: [None; AXIS_SLOTS],
            })
        }
    }
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
    pub fn set_calibrations(&mut self, calibrations: &crate::Calibrations) {
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
