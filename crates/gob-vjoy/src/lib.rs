//! vJoy virtual joystick output straight through the vJoy driver (`direct`), without
//! vJoyInterface.dll. JoyGob still starts when vJoy is missing.
//!
//! Device configuration still runs vJoy's `vJoyConfig.exe` (`config`), found in the vJoy
//! installation folder (docs/VJOY_PLAN.md, session 3 replaces it).

pub mod config;
pub mod configure;
pub mod descriptor;
pub mod direct;
mod ffb_desc;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use direct::{Acquired, Direct};

/// vJoy supports device ids 1..=16.
pub const MAX_DEVICES: u32 = 16;
/// HID usages of X, Y, Z, Rx, Ry, Rz, Slider 0, Slider 1 (axis slots 1..=8).
pub(crate) const AXIS_USAGES: [u32; 8] = [0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37];

#[derive(Debug)]
pub enum Error {
    /// vJoy is not installed.
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

/// The installed vJoy; shared via [`Arc`]. The driver is looked up on each call, so a
/// driver that is switched on or off later is noticed.
pub struct VJoy {
    /// `vJoyInterface.dll` in vJoy's `x64` folder; `vJoyConfig.exe` lives next to it
    /// (`config::tool`). The DLL itself is not loaded.
    pub dll_path: PathBuf,
}

impl VJoy {
    /// Succeeds when vJoy is installed (its driver may be disabled).
    pub fn load() -> Result<Arc<Self>, Error> {
        let dll_path = install_dir()
            .map(|d| d.join(arch()).join("vJoyInterface.dll"))
            .unwrap_or_else(|| {
                PathBuf::from(format!(
                    r"C:\Program Files\vJoy\{}\vJoyInterface.dll",
                    arch()
                ))
            });
        if Direct::find().is_none() && !dll_path.exists() {
            return Err(Error::NotInstalled("vJoy driver not found".into()));
        }
        Ok(Arc::new(Self { dll_path }))
    }

    /// The driver runs and has at least one device.
    pub fn enabled(&self) -> bool {
        Direct::find().is_some_and(|d| !d.collections().is_empty())
    }

    /// Driver version, e.g. `2.1.9`; empty when the driver is not running.
    pub fn version(&self) -> String {
        Direct::find().and_then(|d| d.version()).unwrap_or_default()
    }

    pub fn status(&self, id: u32) -> Status {
        Direct::find().map_or(Status::Unknown, |d| d.status(id))
    }

    /// Every configured device.
    pub fn devices(&self) -> Vec<DeviceInfo> {
        Direct::find().map(|d| d.devices()).unwrap_or_default()
    }

    pub fn device(&self, id: u32) -> Option<DeviceInfo> {
        self.devices().into_iter().find(|d| d.id == id)
    }

    /// Takes ownership of device `id` until the returned handle is dropped.
    pub fn acquire(self: &Arc<Self>, id: u32) -> Result<Device, Error> {
        let direct = Direct::find().ok_or(Error::Disabled)?;
        let collections = direct.collections();
        if collections.is_empty() {
            return Err(Error::Disabled);
        }
        let status = direct.status(id);
        let collection = collections.iter().find(|c| u32::from(c.report_id) == id);
        let acquired = match (collection, u8::try_from(id)) {
            (Some(_), Ok(n)) if matches!(status, Status::Free | Status::Own) => direct.acquire(n),
            _ => None,
        };
        let (Some(acquired), Some(collection)) = (acquired, collection) else {
            return Err(Error::Unavailable { id, status });
        };
        let mut ranges = [(0, 0x8000); 8];
        for &(slot, min, max) in &collection.axes {
            if max > min {
                ranges[usize::from(slot) - 1] = (min, max);
            }
        }
        Ok(Device {
            id,
            ranges,
            continuous: collection.cont_hats > 0,
            acquired: Mutex::new(acquired),
        })
    }
}

/// An acquired vJoy device. Reset and released on drop.
pub struct Device {
    id: u32,
    ranges: [(i32, i32); 8],
    continuous: bool,
    acquired: Mutex<Acquired>,
}

impl Device {
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Changes the state and sends all of it to the driver.
    fn update(&self, change: impl FnOnce(&mut direct::Position) -> bool) -> bool {
        let Ok(mut device) = self.acquired.lock() else {
            return false;
        };
        change(&mut device.position) && device.send()
    }

    /// Sets axis `slot` (1..=8) from a value in -1.0..=1.0.
    pub fn set_axis(&self, slot: u8, value: f32) -> bool {
        let Some(&(min, max)) = self.ranges.get(usize::from(slot).wrapping_sub(1)) else {
            return false;
        };
        let raw = scale_axis(value, min, max);
        self.update(|p| {
            p.axes[usize::from(slot) - 1] = raw;
            true
        })
    }

    /// Presses or releases button `n` (1-based).
    pub fn set_button(&self, n: u8, pressed: bool) -> bool {
        self.update(|p| p.set_button(n, pressed))
    }

    /// Sets hat `n` (1-based) to a direction in hundredths of a degree, or centered.
    pub fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
        if self.continuous {
            let raw = direction.map_or(u32::MAX, |d| u32::from(d % 36000));
            self.update(|p| p.set_cont_hat(n, raw))
        } else {
            self.update(|p| p.set_disc_hat(n, discrete_pov(direction)))
        }
    }

    /// Centers axes and hats, releases buttons.
    pub fn reset(&self) -> bool {
        self.acquired.lock().is_ok_and(|mut d| d.reset())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // Closing the handle (dropping `Acquired` after this) releases the device.
        self.reset();
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

fn arch() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "x64"
    } else {
        "x86"
    }
}

/// vJoy's install location from its uninstall entry.
#[cfg(windows)]
fn install_dir() -> Option<PathBuf> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    let mut buf = [0u16; 512];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: the buffer and its length live across the call.
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
fn install_dir() -> Option<PathBuf> {
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
