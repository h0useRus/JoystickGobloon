//! HidHide control straight through its driver (`\\.\HidHide` IOCTLs): no HidHideCLI or
//! HidHide client needed. Works with the signed 1.5.x driver (IOCTL 2048-2055, see
//! docs/HIDHIDE_PLAN.md).

mod devices;
mod driver;
mod volume;

use std::fmt;
use std::path::{Path, PathBuf};

use driver::Driver;

#[derive(Debug)]
pub enum Error {
    /// The HidHide driver is not installed.
    NotInstalled,
    /// The driver reported an error.
    Driver(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "HidHide is not installed"),
            Self::Driver(e) => write!(f, "HidHide: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// A HID collection known to Windows.
#[derive(Debug, Clone, PartialEq)]
pub struct HidDevice {
    pub present: bool,
    pub gaming_device: bool,
    pub vendor: String,
    pub product: String,
    pub usage: String,
    pub device_instance_path: String,
    /// Parent USB device; HidHide's own client hides it together with the HID collection.
    pub base_container_device_instance_path: String,
}

impl HidDevice {
    pub fn name(&self) -> &str {
        let product = self.product.trim();
        if product.is_empty() {
            &self.device_instance_path
        } else {
            product
        }
    }

    /// `(vendor id, product id)` parsed from the instance path, e.g. `HID\VID_231D&PID_0200\...`.
    pub fn vid_pid(&self) -> Option<(u16, u16)> {
        vid_pid(&self.device_instance_path)
    }

    /// Instance paths to hide or unhide for this device.
    pub fn instance_paths(&self) -> Vec<&str> {
        let mut paths = vec![self.device_instance_path.as_str()];
        if !self.base_container_device_instance_path.is_empty() {
            paths.push(&self.base_container_device_instance_path);
        }
        paths
    }
}

/// HID collections of one physical device (USB container), one row in HidHide's own client.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceGroup {
    pub friendly_name: String,
    pub devices: Vec<HidDevice>,
}

impl DeviceGroup {
    pub fn name(&self) -> &str {
        match self.friendly_name.trim() {
            "" => self.devices.first().map_or("", HidDevice::name),
            name => name,
        }
    }

    /// Whether any of its collections is connected.
    pub fn present(&self) -> bool {
        self.devices.iter().any(|d| d.present)
    }

    /// Whether any of its collections is a game controller.
    pub fn gaming(&self) -> bool {
        self.devices.iter().any(|d| d.gaming_device)
    }

    /// Instance paths to hide or unhide: every collection and the USB container, once each.
    pub fn instance_paths(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = Vec::new();
        for p in self.devices.iter().flat_map(HidDevice::instance_paths) {
            if !paths.iter().any(|q| q.eq_ignore_ascii_case(p)) {
                paths.push(p);
            }
        }
        paths
    }

    /// Hidden when all its collections are in the `hidden` list.
    pub fn is_hidden(&self, hidden: &[String]) -> bool {
        !self.devices.is_empty()
            && self.devices.iter().all(|d| {
                hidden
                    .iter()
                    .any(|h| h.eq_ignore_ascii_case(&d.device_instance_path))
            })
    }
}

/// The installed HidHide driver. Each call opens the driver briefly (see `driver`).
pub struct HidHide {
    /// Version of the running driver file (`HidHide.sys`), e.g. "1.5.230".
    pub version: String,
}

impl HidHide {
    /// Finds the driver; `Error::NotInstalled` without it.
    pub fn find() -> Result<Self, Error> {
        let _ = Driver::open()?;
        Ok(Self {
            version: driver_version().unwrap_or_default(),
        })
    }

    /// Whether hiding is active.
    pub fn cloaked(&self) -> Result<bool, Error> {
        Driver::open()?.get_bool(driver::GET_ACTIVE)
    }

    pub fn set_cloaked(&self, on: bool) -> Result<(), Error> {
        Driver::open()?.set_bool(driver::SET_ACTIVE, on)
    }

    /// Inverse mode: listed applications are the ones that do NOT see hidden devices.
    pub fn inverse(&self) -> Result<bool, Error> {
        Driver::open()?.get_bool(driver::GET_WLINVERSE)
    }

    pub fn set_inverse(&self, on: bool) -> Result<(), Error> {
        Driver::open()?.set_bool(driver::SET_WLINVERSE, on)
    }

    /// Instance paths of hidden devices.
    pub fn hidden(&self) -> Result<Vec<String>, Error> {
        Driver::open()?.get_list(driver::GET_BLACKLIST)
    }

    /// Hides or shows all collections of a device together with its USB container.
    pub fn set_group_hidden(&self, group: &DeviceGroup, hidden: bool) -> Result<(), Error> {
        self.change_hidden(&group.instance_paths(), hidden)
    }

    /// One read-modify-write of the device list.
    fn change_hidden(&self, paths: &[&str], hide: bool) -> Result<(), Error> {
        let driver = Driver::open()?;
        let list = driver.get_list(driver::GET_BLACKLIST)?;
        let next = edit_list(&list, paths, hide);
        if next != list {
            driver.set_list(driver::SET_BLACKLIST, &next)?;
        }
        Ok(())
    }

    /// Applications in HidHide's list (whitelist, or blacklist in inverse mode). Entries on a
    /// volume that is not mounted are returned as the driver's raw `\Device\...` path.
    pub fn apps(&self) -> Result<Vec<PathBuf>, Error> {
        let list = Driver::open()?.get_list(driver::GET_WHITELIST)?;
        let volumes = volume::volumes();
        Ok(list
            .iter()
            .map(|image| {
                volume::from_image_name_in(&volumes, image).unwrap_or_else(|| image.into())
            })
            .collect())
    }

    /// Adds or removes a program. Removing works for files that no longer exist too.
    pub fn set_app(&self, exe: &Path, listed: bool) -> Result<(), Error> {
        let driver = Driver::open()?;
        let list = driver.get_list(driver::GET_WHITELIST)?;
        let volumes = volume::volumes();
        let image = volume::to_image_name_in(&volumes, exe);
        let matches = |entry: &String| {
            image
                .as_ref()
                .is_some_and(|i| i.eq_ignore_ascii_case(entry))
                || same_path(Path::new(entry), exe)
                || volume::from_image_name_in(&volumes, entry).is_some_and(|p| same_path(&p, exe))
        };
        let mut next: Vec<String> = list.iter().filter(|e| !matches(e)).cloned().collect();
        if listed {
            let image = image.ok_or_else(|| {
                Error::Driver(format!("{} is not on a mounted volume", exe.display()))
            })?;
            next.push(image);
        }
        if next != list {
            driver.set_list(driver::SET_WHITELIST, &next)?;
        }
        Ok(())
    }

    /// Adds `exe` to the whitelist unless it is there already; returns whether it was added.
    pub fn ensure_app(&self, exe: &Path) -> Result<bool, Error> {
        if self.apps()?.iter().any(|a| same_path(a, exe)) {
            return Ok(false);
        }
        self.set_app(exe, true).map(|()| true)
    }

    /// HID devices known to Windows (present or not); only game controllers with `gaming_only`.
    pub fn device_groups(&self, gaming_only: bool) -> Result<Vec<DeviceGroup>, Error> {
        Ok(devices::group(devices::find(), gaming_only))
    }
}

/// `list` with `paths` added (`hide`) or removed, compared without case; order kept.
fn edit_list(list: &[String], paths: &[&str], hide: bool) -> Vec<String> {
    let mut next: Vec<String> = list
        .iter()
        .filter(|e| hide || !paths.iter().any(|p| p.eq_ignore_ascii_case(e)))
        .cloned()
        .collect();
    if hide {
        for p in paths {
            if !next.iter().any(|e| e.eq_ignore_ascii_case(p)) {
                next.push((*p).to_string());
            }
        }
    }
    next
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

fn vid_pid(instance_path: &str) -> Option<(u16, u16)> {
    let upper = instance_path.to_ascii_uppercase();
    let hex = |tag: &str| {
        let start = upper.find(tag)? + tag.len();
        u16::from_str_radix(upper.get(start..start + 4)?, 16).ok()
    };
    Some((hex("VID_")?, hex("PID_")?))
}

/// File version of `%SystemRoot%\System32\drivers\HidHide.sys` as "major.minor.build".
#[cfg(windows)]
fn driver_version() -> Option<String> {
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    };
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
    use windows::core::{PCWSTR, w};

    let mut dir = [0u16; 260];
    // SAFETY: the buffer outlives the call.
    let n = unsafe { GetSystemDirectoryW(Some(&mut dir)) } as usize;
    if n == 0 || n >= dir.len() {
        return None;
    }
    let path = format!(
        r"{}\drivers\HidHide.sys",
        String::from_utf16_lossy(&dir[..n])
    );
    let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
    // SAFETY: standard version resource query; `info` points into `data`, alive here.
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(path.as_ptr()), None, size, data.as_mut_ptr().cast()).ok()?;
        let mut info: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(data.as_ptr().cast(), w!("\\"), &mut info, &mut len).as_bool()
            || info.is_null()
        {
            return None;
        }
        let info = &*(info as *const VS_FIXEDFILEINFO);
        Some(format!(
            "{}.{}.{}",
            info.dwFileVersionMS >> 16,
            info.dwFileVersionMS & 0xffff,
            info.dwFileVersionLS >> 16
        ))
    }
}

#[cfg(not(windows))]
fn driver_version() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vkb() -> DeviceGroup {
        DeviceGroup {
            friendly_name: "VKB".into(),
            devices: vec![HidDevice {
                present: true,
                gaming_device: true,
                vendor: "VKB-Sim".into(),
                product: " VKBsim Gladiator EVO R  ".into(),
                usage: "Joystick".into(),
                device_instance_path: r"HID\VID_231D&PID_0200\a&2f28a505&1&0000".into(),
                base_container_device_instance_path: r"USB\VID_231D&PID_0200\9&7B3A559&0&2".into(),
            }],
        }
    }

    #[test]
    fn group_state() {
        let g = vkb();
        assert_eq!(g.name(), "VKB");
        assert!(g.present() && g.gaming());
        assert_eq!(g.devices[0].name(), "VKBsim Gladiator EVO R");
        assert_eq!(g.devices[0].vid_pid(), Some((0x231D, 0x0200)));
        assert_eq!(g.instance_paths().len(), 2);
        let hidden = vec![r"hid\vid_231d&pid_0200\A&2F28A505&1&0000".to_string()];
        assert!(g.is_hidden(&hidden));
        assert!(!g.is_hidden(&[]));
    }

    #[test]
    fn group_paths_once() {
        let device = |col: &str| HidDevice {
            present: true,
            gaming_device: false,
            vendor: String::new(),
            product: String::new(),
            usage: String::new(),
            device_instance_path: format!(r"HID\VID_3329&PID_4B1E&{col}\0"),
            base_container_device_instance_path: r"USB\VID_3329&PID_4B1E\0".into(),
        };
        let group = DeviceGroup {
            friendly_name: " ".into(),
            devices: vec![device("Col01"), device("Col02")],
        };
        assert_eq!(group.instance_paths().len(), 3);
        assert_eq!(group.name(), group.devices[0].device_instance_path);
    }

    #[test]
    fn edits_lists_without_case() {
        let list = vec![r"HID\A\1".to_string(), r"USB\A\1".to_string()];
        assert_eq!(edit_list(&list, &[r"hid\a\1", r"HID\B\1"], true).len(), 3);
        assert_eq!(edit_list(&list, &[r"hid\a\1"], false), [r"USB\A\1"]);
        assert_eq!(edit_list(&list, &[r"HID\C\1"], false), list);
    }
}
