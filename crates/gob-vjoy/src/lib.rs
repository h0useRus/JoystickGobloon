//! vJoy virtual joystick output through `vJoyInterface.dll`.
//!
//! The DLL is loaded at runtime from the vJoy installation, so JoyGob still
//! starts when vJoy is missing.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libloading::Library;

/// vJoy supports device ids 1..=16.
pub const MAX_DEVICES: u32 = 16;
/// HID usages of X, Y, Z, Rx, Ry, Rz, Slider 0, Slider 1 (axis slots 1..=8).
const AXIS_USAGES: [u32; 8] = [0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37];

#[derive(Debug)]
pub enum Error {
    /// vJoy is not installed or the DLL could not be loaded.
    NotInstalled(String),
    /// The driver is installed but disabled.
    Disabled,
    /// The device is missing, or owned by another program (e.g. Joystick Gremlin).
    Unavailable { id: u32, status: Status },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled(e) => write!(f, "vJoy is not available: {e}"),
            Self::Disabled => write!(f, "vJoy driver is disabled"),
            Self::Unavailable { id, status } => write!(f, "vJoy device {id} is {status:?}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Acquired by this process.
    Own,
    Free,
    /// Acquired by another process.
    Busy,
    /// Not configured.
    Missing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: u32,
    pub status: Status,
    /// Present axes as slot ids (1 = X ... 8 = Slider 1), as in Gremlin profiles.
    pub axes: Vec<u8>,
    pub buttons: u32,
    pub cont_hats: u32,
    pub disc_hats: u32,
}

impl DeviceInfo {
    pub fn hats(&self) -> u32 {
        self.cont_hats.max(self.disc_hats)
    }
}

type Bool = i32;

/// Function table resolved from vJoyInterface.dll.
struct Api {
    _lib: Library,
    enabled: unsafe extern "C" fn() -> Bool,
    version: unsafe extern "C" fn() -> i16,
    status: unsafe extern "C" fn(u32) -> i32,
    acquire: unsafe extern "C" fn(u32) -> Bool,
    relinquish: unsafe extern "C" fn(u32),
    reset: unsafe extern "C" fn(u32) -> Bool,
    buttons: unsafe extern "C" fn(u32) -> i32,
    disc_povs: unsafe extern "C" fn(u32) -> i32,
    cont_povs: unsafe extern "C" fn(u32) -> i32,
    axis_exists: unsafe extern "C" fn(u32, u32) -> Bool,
    axis_min: unsafe extern "C" fn(u32, u32, *mut i32) -> Bool,
    axis_max: unsafe extern "C" fn(u32, u32, *mut i32) -> Bool,
    set_axis: unsafe extern "C" fn(i32, u32, u32) -> Bool,
    set_button: unsafe extern "C" fn(Bool, u32, u8) -> Bool,
    set_disc_pov: unsafe extern "C" fn(i32, u32, u8) -> Bool,
    set_cont_pov: unsafe extern "C" fn(u32, u32, u8) -> Bool,
}

impl Api {
    fn load(path: &Path) -> Result<Self, libloading::Error> {
        unsafe {
            let lib = Library::new(path)?;
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get($name)?
                };
            }
            Ok(Self {
                enabled: sym!(b"vJoyEnabled\0"),
                version: sym!(b"GetvJoyVersion\0"),
                status: sym!(b"GetVJDStatus\0"),
                acquire: sym!(b"AcquireVJD\0"),
                relinquish: sym!(b"RelinquishVJD\0"),
                reset: sym!(b"ResetVJD\0"),
                buttons: sym!(b"GetVJDButtonNumber\0"),
                disc_povs: sym!(b"GetVJDDiscPovNumber\0"),
                cont_povs: sym!(b"GetVJDContPovNumber\0"),
                axis_exists: sym!(b"GetVJDAxisExist\0"),
                axis_min: sym!(b"GetVJDAxisMin\0"),
                axis_max: sym!(b"GetVJDAxisMax\0"),
                set_axis: sym!(b"SetAxis\0"),
                set_button: sym!(b"SetBtn\0"),
                set_disc_pov: sym!(b"SetDiscPov\0"),
                set_cont_pov: sym!(b"SetContPov\0"),
                _lib: lib,
            })
        }
    }
}

/// Loaded vJoy driver interface; shared via [`Arc`].
pub struct VJoy {
    api: Api,
    pub dll_path: PathBuf,
}

impl VJoy {
    /// Loads vJoyInterface.dll from the vJoy installation.
    pub fn load() -> Result<Arc<Self>, Error> {
        let path =
            find_dll().ok_or_else(|| Error::NotInstalled("vJoyInterface.dll not found".into()))?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Arc<Self>, Error> {
        let api = Api::load(path).map_err(|e| Error::NotInstalled(e.to_string()))?;
        Ok(Arc::new(Self {
            api,
            dll_path: path.to_path_buf(),
        }))
    }

    pub fn enabled(&self) -> bool {
        unsafe { (self.api.enabled)() != 0 }
    }

    /// Driver version as reported by vJoy, e.g. `2.1.9`.
    pub fn version(&self) -> String {
        let v = unsafe { (self.api.version)() } as u16;
        format!("{}.{}.{}", v >> 8, (v >> 4) & 0xF, v & 0xF)
    }

    pub fn status(&self, id: u32) -> Status {
        match unsafe { (self.api.status)(id) } {
            0 => Status::Own,
            1 => Status::Free,
            2 => Status::Busy,
            3 => Status::Missing,
            _ => Status::Unknown,
        }
    }

    /// Every configured device.
    pub fn devices(&self) -> Vec<DeviceInfo> {
        (1..=MAX_DEVICES).filter_map(|id| self.device(id)).collect()
    }

    pub fn device(&self, id: u32) -> Option<DeviceInfo> {
        let status = self.status(id);
        if matches!(status, Status::Missing | Status::Unknown) {
            return None;
        }
        let api = &self.api;
        unsafe {
            Some(DeviceInfo {
                id,
                status,
                axes: (1..=8u8)
                    .filter(|&s| (api.axis_exists)(id, AXIS_USAGES[usize::from(s) - 1]) != 0)
                    .collect(),
                buttons: (api.buttons)(id).max(0) as u32,
                cont_hats: (api.cont_povs)(id).max(0) as u32,
                disc_hats: (api.disc_povs)(id).max(0) as u32,
            })
        }
    }

    /// Takes ownership of device `id` until the returned handle is dropped.
    pub fn acquire(self: &Arc<Self>, id: u32) -> Result<Device, Error> {
        if !self.enabled() {
            return Err(Error::Disabled);
        }
        let status = self.status(id);
        if !matches!(status, Status::Free | Status::Own) || unsafe { (self.api.acquire)(id) } == 0 {
            return Err(Error::Unavailable { id, status });
        }
        let mut ranges = [(0, 0x8000); 8];
        for (i, range) in ranges.iter_mut().enumerate() {
            let (mut min, mut max) = (0, 0);
            unsafe {
                if (self.api.axis_min)(id, AXIS_USAGES[i], &mut min) != 0
                    && (self.api.axis_max)(id, AXIS_USAGES[i], &mut max) != 0
                    && max > min
                {
                    *range = (min, max);
                }
            }
        }
        let continuous = unsafe { (self.api.cont_povs)(id) } > 0;
        let device = Device {
            vjoy: self.clone(),
            id,
            ranges,
            continuous,
        };
        device.reset();
        Ok(device)
    }
}

/// An acquired vJoy device. Reset and released on drop.
pub struct Device {
    vjoy: Arc<VJoy>,
    id: u32,
    ranges: [(i32, i32); 8],
    continuous: bool,
}

impl Device {
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Sets axis `slot` (1..=8) from a value in -1.0..=1.0.
    pub fn set_axis(&self, slot: u8, value: f32) -> bool {
        let Some(&(min, max)) = self.ranges.get(usize::from(slot).wrapping_sub(1)) else {
            return false;
        };
        let raw = scale_axis(value, min, max);
        unsafe { (self.vjoy.api.set_axis)(raw, self.id, AXIS_USAGES[usize::from(slot) - 1]) != 0 }
    }

    /// Presses or releases button `n` (1-based).
    pub fn set_button(&self, n: u8, pressed: bool) -> bool {
        unsafe { (self.vjoy.api.set_button)(Bool::from(pressed), self.id, n) != 0 }
    }

    /// Sets hat `n` (1-based) to a direction in hundredths of a degree, or centered.
    pub fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
        unsafe {
            if self.continuous {
                let raw = direction.map_or(u32::MAX, |d| u32::from(d % 36000));
                (self.vjoy.api.set_cont_pov)(raw, self.id, n) != 0
            } else {
                (self.vjoy.api.set_disc_pov)(discrete_pov(direction), self.id, n) != 0
            }
        }
    }

    /// Centers axes and hats, releases buttons.
    pub fn reset(&self) -> bool {
        unsafe { (self.vjoy.api.reset)(self.id) != 0 }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.reset();
        unsafe { (self.vjoy.api.relinquish)(self.id) }
    }
}

/// -1.0..=1.0 -> min..=max.
pub fn scale_axis(value: f32, min: i32, max: i32) -> i32 {
    let t = (value.clamp(-1.0, 1.0) + 1.0) / 2.0;
    min + (t * (max - min) as f32).round() as i32
}

/// Direction in hundredths of a degree -> 4-way POV (0 up, 1 right, 2 down, 3 left, -1 centered).
pub fn discrete_pov(direction: Option<u16>) -> i32 {
    direction.map_or(-1, |d| ((i32::from(d % 36000) + 4500) / 9000) % 4)
}

/// vJoy's install location from its uninstall entry, then the default path.
fn find_dll() -> Option<PathBuf> {
    let arch = if cfg!(target_pointer_width = "64") {
        "x64"
    } else {
        "x86"
    };
    let mut candidates = Vec::new();
    if let Some(dir) = install_location() {
        candidates.push(dir.join(arch).join("vJoyInterface.dll"));
    }
    candidates.push(PathBuf::from(format!(
        r"C:\Program Files\vJoy\{arch}\vJoyInterface.dll"
    )));
    candidates.into_iter().find(|p| p.exists())
}

#[cfg(windows)]
fn install_location() -> Option<PathBuf> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    let mut buf = [0u16; 512];
    let mut len = (buf.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{8E31F76F-74C3-47F1-9550-E041EEDC5FBB}_is1"
            ),
            w!("InstallLocation"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if result.is_err() {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..end])))
}

#[cfg(not(windows))]
fn install_location() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_scaling() {
        assert_eq!(scale_axis(-1.0, 1, 0x8000), 1);
        assert_eq!(scale_axis(1.0, 1, 0x8000), 0x8000);
        assert_eq!(scale_axis(0.0, 0, 100), 50);
        assert_eq!(scale_axis(5.0, 0, 100), 100);
    }

    #[test]
    fn discrete_hat() {
        assert_eq!(discrete_pov(None), -1);
        assert_eq!(discrete_pov(Some(0)), 0);
        assert_eq!(discrete_pov(Some(9000)), 1);
        assert_eq!(discrete_pov(Some(18000)), 2);
        assert_eq!(discrete_pov(Some(27000)), 3);
        assert_eq!(discrete_pov(Some(35000)), 0);
        assert_eq!(discrete_pov(Some(4400)), 0);
    }
}
