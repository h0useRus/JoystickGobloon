//! vJoy straight through its driver, without vJoyInterface.dll (docs/VJOY_PLAN.md).
//!
//! - Control channel `<namespace>device_000` (interface `GUID_DEVINTERFACE_VJOY`): driver info and
//!   per-device status (exists, opened by which process).
//! - Capabilities (axes, buttons, hats) and the driver version come from the HID collections the
//!   driver exposes (VID 0x1234, PID 0xBEAD), one per vJoy device, report id = device id.
//!
//! Read-only for now; output still goes through the DLL (`crate::VJoy`).

use crate::{DeviceInfo, Status, AXIS_USAGES, MAX_DEVICES};

/// `CTL_CODE(FILE_DEVICE_UNKNOWN, f, METHOD_BUFFERED, access)`.
const fn ioctl(f: u32, access: u32) -> u32 {
    (0x22 << 16) | (access << 14) | (f << 2)
}

const FILE_ANY_ACCESS: u32 = 0;
pub const GET_DEV_STAT: u32 = ioctl(0x91D, FILE_ANY_ACCESS);
pub const GET_DRV_INFO: u32 = ioctl(0x91E, FILE_ANY_ACCESS);

/// vJoy's HID identity.
pub const VENDOR_ID: u16 = 0x1234;
pub const PRODUCT_ID: u16 = 0xBEAD;
/// HID usage of a hat switch.
const USAGE_HAT: u16 = 0x39;

/// What `GET_DRV_INFO` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverInfo {
    pub ffb: bool,
    pub max_devices: u8,
    pub existing: u8,
}

/// What `GET_DEV_STAT` reports for one device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceStat {
    pub exists: bool,
    /// Some process has the device open (acquired it).
    pub opened: bool,
    pub owner_pid: u16,
}

impl DeviceStat {
    pub fn parse(buf: &[u8]) -> Option<Self> {
        let flags = *buf.first()?;
        Some(Self {
            exists: flags & 0x01 != 0,
            opened: flags & 0x04 != 0,
            owner_pid: u16::from_le_bytes([*buf.get(1)?, *buf.get(2)?]),
        })
    }

    /// The status as vJoyInterface.dll's `GetVJDStatus` reports it.
    pub fn status(&self, own_pid: u32) -> Status {
        match (self.exists, self.opened) {
            (false, _) => Status::Missing,
            (true, false) => Status::Free,
            // The driver keeps a 16-bit PID, like the DLL compares it.
            (true, true) if u32::from(self.owner_pid) == own_pid & 0xFFFF => Status::Own,
            (true, true) => Status::Busy,
        }
    }
}

/// `2.1.9` from the HID version number `0x0219`, as the DLL formats it.
pub fn format_version(v: u16) -> String {
    format!("{}.{}.{}", v >> 8, (v >> 4) & 0xF, v & 0xF)
}

/// A vJoy HID collection as Windows sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub report_id: u8,
    pub version: u16,
    /// (slot 1..=8, logical min, logical max) of present axes.
    pub axes: Vec<(u8, i32, i32)>,
    pub buttons: u32,
    pub cont_hats: u32,
    pub disc_hats: u32,
}

impl Collection {
    pub fn info(&self, status: Status) -> DeviceInfo {
        DeviceInfo {
            id: u32::from(self.report_id),
            status,
            axes: self.axes.iter().map(|a| a.0).collect(),
            buttons: self.buttons,
            cont_hats: self.cont_hats,
            disc_hats: self.disc_hats,
        }
    }
}

/// vJoy read through the driver.
pub struct Direct {
    namespace: String,
}

impl Direct {
    /// Finds the vJoy driver; `None` when it is not installed or not running.
    pub fn find() -> Option<Self> {
        imp::namespace().map(|namespace| Self { namespace })
    }

    pub fn driver_info(&self) -> Option<DriverInfo> {
        let buf = imp::control(&self.namespace, GET_DRV_INFO, &[], 10)?;
        Some(DriverInfo {
            ffb: buf.first()? & 0x01 != 0,
            max_devices: *buf.get(1)?,
            existing: *buf.get(2)?,
        })
    }

    pub fn device_stat(&self, id: u8) -> Option<DeviceStat> {
        DeviceStat::parse(&imp::control(&self.namespace, GET_DEV_STAT, &[id], 10)?)
    }

    pub fn status(&self, id: u32) -> Status {
        let Ok(id) = u8::try_from(id) else {
            return Status::Unknown;
        };
        self.device_stat(id)
            .map_or(Status::Unknown, |s| s.status(std::process::id()))
    }

    /// HID collections of the configured devices, by report id.
    pub fn collections(&self) -> Vec<Collection> {
        let mut found = imp::collections();
        found.sort_by_key(|c| c.report_id);
        found
    }

    /// Driver version from the HID collections, e.g. "2.1.9".
    pub fn version(&self) -> Option<String> {
        self.collections()
            .first()
            .map(|c| format_version(c.version))
    }

    /// Every configured device, like `VJoy::devices`.
    pub fn devices(&self) -> Vec<DeviceInfo> {
        self.collections()
            .iter()
            .filter(|c| (1..=MAX_DEVICES).contains(&u32::from(c.report_id)))
            .map(|c| c.info(self.status(u32::from(c.report_id))))
            .collect()
    }
}

/// Axis slot (1..=8) of a HID usage, as in `AXIS_USAGES`.
fn axis_slot(usage: u16) -> Option<u8> {
    AXIS_USAGES
        .iter()
        .position(|&u| u == u32::from(usage))
        .map(|i| i as u8 + 1)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::{GUID, PCWSTR};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    };
    use windows::Win32::Devices::HumanInterfaceDevice::{
        HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid, HidD_GetPreparsedData,
        HidP_GetButtonCaps, HidP_GetCaps, HidP_GetValueCaps, HidP_Input, HIDD_ATTRIBUTES,
        HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS, PHIDP_PREPARSED_DATA,
    };
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    /// `{781EF630-72B2-11D2-B852-00C04FAD5101}`
    const GUID_DEVINTERFACE_VJOY: GUID = GUID::from_u128(0x781ef630_72b2_11d2_b852_00c04fad5101);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    /// Paths of the present interfaces of `class`.
    fn interface_paths(class: &GUID) -> Vec<String> {
        let mut paths = Vec::new();
        // SAFETY: standard SetupAPI enumeration; the list is destroyed before returning.
        unsafe {
            let Ok(set) = SetupDiGetClassDevsW(
                Some(class),
                PCWSTR::null(),
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            ) else {
                return paths;
            };
            for i in 0.. {
                let mut data = SP_DEVICE_INTERFACE_DATA {
                    cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInterfaces(set, None, class, i, &mut data).is_err() {
                    break;
                }
                if let Some(p) = detail(set, &data) {
                    paths.push(p);
                }
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
        }
        paths
    }

    unsafe fn detail(set: HDEVINFO, data: &SP_DEVICE_INTERFACE_DATA) -> Option<String> {
        let mut needed = 0u32;
        let _ = SetupDiGetDeviceInterfaceDetailW(set, data, None, 0, Some(&mut needed), None);
        let mut buffer = vec![0u8; needed as usize + 8];
        let detail = buffer
            .as_mut_ptr()
            .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
        SetupDiGetDeviceInterfaceDetailW(set, data, Some(detail), needed, None, None).ok()?;
        let path = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
        let len = (0..).take_while(|&i| *path.add(i) != 0).count();
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(
            path, len,
        )))
    }

    /// The driver's interface path up to its last `\`: device N is `<namespace>device_NNN`.
    pub fn namespace() -> Option<String> {
        let path = interface_paths(&GUID_DEVINTERFACE_VJOY)
            .into_iter()
            .next()?;
        let cut = path.rfind('\\')? + 1;
        Some(path[..cut].to_string())
    }

    fn open(path: &str, access: u32) -> Option<HANDLE> {
        let p = wide(path);
        // SAFETY: plain CreateFileW on a NUL-terminated path.
        unsafe {
            CreateFileW(
                PCWSTR(p.as_ptr()),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
            .ok()
        }
    }

    /// One IOCTL on the control channel `device_000`; the bytes it returned.
    pub fn control(namespace: &str, code: u32, input: &[u8], out_len: usize) -> Option<Vec<u8>> {
        let handle = open(
            &format!("{namespace}device_000"),
            GENERIC_READ.0 | GENERIC_WRITE.0,
        )?;
        let mut out = vec![0u8; out_len];
        let mut returned = 0u32;
        // SAFETY: buffers outlive the synchronous call; the handle is closed after it.
        let ok = unsafe {
            let ok = DeviceIoControl(
                handle,
                code,
                (!input.is_empty()).then(|| input.as_ptr().cast()),
                input.len() as u32,
                Some(out.as_mut_ptr().cast()),
                out.len() as u32,
                Some(&mut returned),
                None,
            )
            .is_ok();
            let _ = CloseHandle(handle);
            ok
        };
        if !ok || returned == 0 {
            return None;
        }
        out.truncate(returned as usize);
        Some(out)
    }

    fn read_collection(handle: HANDLE) -> Option<Collection> {
        // SAFETY: attribute and capability queries on an open HID handle; the preparsed
        // data is freed before returning.
        unsafe {
            let mut attributes = HIDD_ATTRIBUTES {
                Size: size_of::<HIDD_ATTRIBUTES>() as u32,
                ..Default::default()
            };
            if !HidD_GetAttributes(handle, &mut attributes)
                || attributes.VendorID != VENDOR_ID
                || attributes.ProductID != PRODUCT_ID
            {
                return None;
            }
            let mut data = PHIDP_PREPARSED_DATA::default();
            if !HidD_GetPreparsedData(handle, &mut data) {
                return None;
            }
            let result = (|| {
                let mut caps = HIDP_CAPS::default();
                if HidP_GetCaps(data, &mut caps) != HIDP_STATUS_SUCCESS {
                    return None;
                }
                let mut values =
                    vec![HIDP_VALUE_CAPS::default(); caps.NumberInputValueCaps as usize];
                let mut n = values.len() as u16;
                if n > 0
                    && HidP_GetValueCaps(HidP_Input, values.as_mut_ptr(), &mut n, data)
                        != HIDP_STATUS_SUCCESS
                {
                    return None;
                }
                values.truncate(n as usize);
                let mut buttons =
                    vec![HIDP_BUTTON_CAPS::default(); caps.NumberInputButtonCaps as usize];
                let mut nb = buttons.len() as u16;
                if nb > 0
                    && HidP_GetButtonCaps(HidP_Input, buttons.as_mut_ptr(), &mut nb, data)
                        != HIDP_STATUS_SUCCESS
                {
                    return None;
                }
                buttons.truncate(nb as usize);

                let mut c = Collection {
                    report_id: values
                        .first()
                        .map(|v| v.ReportID)
                        .or_else(|| buttons.first().map(|b| b.ReportID))?,
                    version: attributes.VersionNumber,
                    axes: Vec::new(),
                    buttons: 0,
                    cont_hats: 0,
                    disc_hats: 0,
                };
                for v in &values {
                    let (min, max) = if v.IsRange {
                        (v.Anonymous.Range.UsageMin, v.Anonymous.Range.UsageMax)
                    } else {
                        (v.Anonymous.NotRange.Usage, v.Anonymous.NotRange.Usage)
                    };
                    for usage in min..=max {
                        if usage == USAGE_HAT {
                            // Discrete hats report 0..3, continuous ones 0..35999.
                            let count = u32::from(v.ReportCount.max(1));
                            if v.LogicalMax > 3 {
                                c.cont_hats += count;
                            } else {
                                c.disc_hats += count;
                            }
                        } else if let Some(slot) = axis_slot(usage) {
                            c.axes.push((slot, v.LogicalMin, v.LogicalMax));
                        }
                    }
                }
                c.axes.sort_by_key(|a| a.0);
                c.axes.dedup_by_key(|a| a.0);
                for b in &buttons {
                    c.buttons += if b.IsRange {
                        u32::from(b.Anonymous.Range.UsageMax)
                            .saturating_sub(u32::from(b.Anonymous.Range.UsageMin))
                            + 1
                    } else {
                        1
                    };
                }
                Some(c)
            })();
            let _ = HidD_FreePreparsedData(data);
            result
        }
    }

    pub fn collections() -> Vec<Collection> {
        // SAFETY: returns a constant GUID.
        let hid = unsafe { HidD_GetHidGuid() };
        interface_paths(&hid)
            .iter()
            .filter(|p| {
                let p = p.to_ascii_lowercase();
                p.contains("vid_1234&pid_bead") || p.contains("hidclass")
            })
            .filter_map(|p| {
                // Zero access: attributes and capabilities only, works while acquired.
                let handle = open(p, 0)?;
                let c = read_collection(handle);
                // SAFETY: the handle came from CreateFileW.
                unsafe {
                    let _ = CloseHandle(handle);
                }
                c
            })
            .collect()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn namespace() -> Option<String> {
        None
    }
    pub fn control(_: &str, _: u32, _: &[u8], _: usize) -> Option<Vec<u8>> {
        None
    }
    pub fn collections() -> Vec<Collection> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_codes() {
        // CTL_CODE(FILE_DEVICE_UNKNOWN, 0x91D, METHOD_BUFFERED, FILE_ANY_ACCESS)
        assert_eq!(GET_DEV_STAT, 0x0022_2474);
        assert_eq!(GET_DRV_INFO, 0x0022_2478);
    }

    #[test]
    fn device_status() {
        let missing = DeviceStat::parse(&[0, 0, 0, 0, 0]).unwrap();
        assert_eq!(missing.status(42), Status::Missing);
        let free = DeviceStat::parse(&[0x01, 0, 0, 0, 0]).unwrap();
        assert_eq!(free.status(42), Status::Free);
        let mine = DeviceStat::parse(&[0x05, 42, 0, 0, 0]).unwrap();
        assert_eq!(mine.owner_pid, 42);
        assert_eq!(mine.status(42), Status::Own);
        assert_eq!(mine.status(0x1_002A), Status::Own);
        assert_eq!(mine.status(7), Status::Busy);
        assert!(DeviceStat::parse(&[1]).is_none());
    }

    #[test]
    fn versions_and_axes() {
        assert_eq!(format_version(0x0219), "2.1.9");
        assert_eq!(axis_slot(0x30), Some(1));
        assert_eq!(axis_slot(0x37), Some(8));
        assert_eq!(axis_slot(0x39), None);
    }
}
