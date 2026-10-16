//! Thin safe wrapper over the DirectInput 8 joystick API.

use std::ffi::c_void;
use std::mem::size_of;

use windows::core::{Interface, BOOL, GUID};
use windows::Win32::Devices::HumanInterfaceDevice::{
    DirectInput8Create, IDirectInput8W, IDirectInputDevice8W, DI8DEVCLASS_GAMECTRL, DIDATAFORMAT,
    DIDEVCAPS, DIDEVICEINSTANCEW, DIEDFL_ATTACHEDONLY, DIJOYSTATE2, DIPH_BYOFFSET, DIPROPHEADER,
    DIPROPRANGE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use crate::{format_guid, normalize_axis, pov_direction, DeviceInfo, DeviceState, AXIS_SLOTS};

const DIRECTINPUT_VERSION: u32 = 0x0800;
/// `DIPROP_RANGE` is `MAKEDIPROP(4)`: a small integer passed as a GUID pointer.
const DIPROP_RANGE: *const GUID = std::ptr::without_provenance(4);
/// Byte offsets of lX, lY, lZ, lRx, lRy, lRz, rglSlider[0], rglSlider[1] in DIJOYSTATE2.
const AXIS_OFFSETS: [u32; AXIS_SLOTS] = [0, 4, 8, 12, 16, 20, 24, 28];

#[link(name = "dinput8", kind = "static", modifiers = "-bundle")]
extern "C" {
    /// Predefined data format from dinput8.lib describing DIJOYSTATE2.
    static c_dfDIJoystick2: DIDATAFORMAT;
}

// Axis/POV GUIDs referenced by `c_dfDIJoystick2`.
#[link(name = "dxguid", kind = "static", modifiers = "-bundle")]
extern "C" {}

pub struct DirectInput {
    di: IDirectInput8W,
}

pub struct Device {
    dev: IDirectInputDevice8W,
    pub state: DeviceState,
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
            };
            Ok(Device {
                dev,
                state: DeviceState::new(info),
            })
        }
    }
}

impl Device {
    /// Reads the current state; returns false when the device is gone.
    /// `calibrations` replace the default scaling for the axes they cover.
    pub fn poll(&mut self, calibrations: &crate::Calibrations) -> bool {
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
            s.axes[i] = match calibrations.get(&(s.info.guid.clone(), id)) {
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
    let found = &mut *(ctx as *mut Vec<DIDEVICEINSTANCEW>);
    found.push(*inst);
    BOOL(1) // DIENUM_CONTINUE
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}
