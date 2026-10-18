//! HID access: present HID interfaces of a vendor, their feature reports, Get/SetFeature.

/// One HID top-level collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    /// Interface path (`\\?\hid#vid_231d&pid_...`), the same one DirectInput reports in
    /// `DIPROP_GUIDANDPATH` for its device.
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: String,
    pub usage_page: u16,
    pub usage: u16,
    /// Feature report length with the report id byte, 0 = none.
    pub feature_len: u16,
    /// Report ids of the feature reports.
    pub feature_ids: Vec<u8>,
}

#[cfg(windows)]
pub use imp::{collections, get_feature, set_feature};

#[cfg(not(windows))]
pub fn collections(_vendor: u16) -> Vec<Collection> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn get_feature(_path: &str, _id: u8, _len: usize) -> std::io::Result<Vec<u8>> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(not(windows))]
pub fn set_feature(_path: &str, _report: &[u8]) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(windows)]
mod imp {
    use super::Collection;
    use std::io;
    use windows::core::{GUID, PCWSTR};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    };
    use windows::Win32::Devices::HumanInterfaceDevice::{
        HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetFeature, HidD_GetHidGuid,
        HidD_GetPreparsedData, HidD_GetProductString, HidD_SetFeature, HidP_Feature,
        HidP_GetButtonCaps, HidP_GetCaps, HidP_GetValueCaps, HIDD_ATTRIBUTES, HIDP_BUTTON_CAPS,
        HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS, PHIDP_PREPARSED_DATA,
    };
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    /// Closes the handle on drop.
    struct Handle(HANDLE);

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateFileW and is closed once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    /// Opens a HID interface shared with everyone else (DirectInput keeps reading it);
    /// `access` 0 = attributes and strings only.
    fn open(path: &str, access: u32) -> io::Result<Handle> {
        let path = wide(path);
        // SAFETY: `path` is NUL-terminated and lives across the call.
        unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .map(Handle)
        .map_err(|e| io::Error::from_raw_os_error(e.code().0))
    }

    /// Interface paths of all present HID collections.
    fn interface_paths() -> Vec<String> {
        let mut paths = Vec::new();
        // SAFETY: standard SetupAPI enumeration; the list is destroyed before returning.
        unsafe {
            let hid: GUID = HidD_GetHidGuid();
            let Ok(set) = SetupDiGetClassDevsW(
                Some(&hid),
                None,
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            ) else {
                return paths;
            };
            for index in 0.. {
                let mut data = SP_DEVICE_INTERFACE_DATA {
                    cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInterfaces(set, None, &hid, index, &mut data).is_err() {
                    break;
                }
                let mut needed = 0u32;
                let _ =
                    SetupDiGetDeviceInterfaceDetailW(set, &data, None, 0, Some(&mut needed), None);
                let mut buffer = vec![0u8; needed as usize + 8];
                let detail = buffer
                    .as_mut_ptr()
                    .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
                (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                if SetupDiGetDeviceInterfaceDetailW(set, &data, Some(detail), needed, None, None)
                    .is_ok()
                {
                    let path = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
                    let len = (0..).take_while(|&i| *path.add(i) != 0).count();
                    paths.push(String::from_utf16_lossy(std::slice::from_raw_parts(
                        path, len,
                    )));
                }
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
        }
        paths
    }

    /// Caps and feature report ids from the preparsed data.
    fn read_caps(handle: &Handle, c: &mut Collection) {
        // SAFETY: the preparsed data is freed before returning; cap arrays are sized by
        // the counts HidP_GetCaps reports.
        unsafe {
            let mut data = PHIDP_PREPARSED_DATA::default();
            if !HidD_GetPreparsedData(handle.0, &mut data) {
                return;
            }
            let mut caps = HIDP_CAPS::default();
            if HidP_GetCaps(data, &mut caps) == HIDP_STATUS_SUCCESS {
                c.usage_page = caps.UsagePage;
                c.usage = caps.Usage;
                c.feature_len = caps.FeatureReportByteLength;
                let mut n = caps.NumberFeatureValueCaps;
                let mut values = vec![HIDP_VALUE_CAPS::default(); n as usize];
                if n > 0
                    && HidP_GetValueCaps(HidP_Feature, values.as_mut_ptr(), &mut n, data)
                        == HIDP_STATUS_SUCCESS
                {
                    c.feature_ids
                        .extend(values[..n as usize].iter().map(|v| v.ReportID));
                }
                let mut n = caps.NumberFeatureButtonCaps;
                let mut buttons = vec![HIDP_BUTTON_CAPS::default(); n as usize];
                if n > 0
                    && HidP_GetButtonCaps(HidP_Feature, buttons.as_mut_ptr(), &mut n, data)
                        == HIDP_STATUS_SUCCESS
                {
                    c.feature_ids
                        .extend(buttons[..n as usize].iter().map(|b| b.ReportID));
                }
                c.feature_ids.sort_unstable();
                c.feature_ids.dedup();
            }
            let _ = HidD_FreePreparsedData(data);
        }
    }

    /// Present HID collections of `vendor`.
    pub fn collections(vendor: u16) -> Vec<Collection> {
        let mut found = Vec::new();
        for path in interface_paths() {
            let Ok(handle) = open(&path, 0) else {
                continue;
            };
            let mut attributes = HIDD_ATTRIBUTES {
                Size: size_of::<HIDD_ATTRIBUTES>() as u32,
                ..Default::default()
            };
            // SAFETY: `attributes` is a valid out struct with its size set.
            if !unsafe { HidD_GetAttributes(handle.0, &mut attributes) }
                || attributes.VendorID != vendor
            {
                continue;
            }
            let mut c = Collection {
                path,
                vendor_id: attributes.VendorID,
                product_id: attributes.ProductID,
                product: String::new(),
                usage_page: 0,
                usage: 0,
                feature_len: 0,
                feature_ids: Vec::new(),
            };
            let mut buffer = [0u16; 128];
            // SAFETY: the buffer size in bytes is passed.
            if unsafe {
                HidD_GetProductString(
                    handle.0,
                    buffer.as_mut_ptr().cast(),
                    (buffer.len() * 2) as u32,
                )
            } {
                let end = buffer.iter().position(|&w| w == 0).unwrap_or(buffer.len());
                c.product = String::from_utf16_lossy(&buffer[..end]).trim().to_string();
            }
            read_caps(&handle, &mut c);
            found.push(c);
        }
        found
    }

    fn last_error() -> io::Error {
        io::Error::last_os_error()
    }

    /// Feature report `id` (`len` bytes with the id byte).
    pub fn get_feature(path: &str, id: u8, len: usize) -> io::Result<Vec<u8>> {
        let handle = open(path, (GENERIC_READ | GENERIC_WRITE).0)?;
        let mut buffer = vec![0u8; len.max(1)];
        buffer[0] = id;
        // SAFETY: the buffer and its length are passed together.
        if unsafe { HidD_GetFeature(handle.0, buffer.as_mut_ptr().cast(), buffer.len() as u32) } {
            Ok(buffer)
        } else {
            Err(last_error())
        }
    }

    /// Sends a feature report; `report[0]` is its id.
    pub fn set_feature(path: &str, report: &[u8]) -> io::Result<()> {
        let handle = open(path, (GENERIC_READ | GENERIC_WRITE).0)?;
        // SAFETY: the buffer and its length are passed together.
        if unsafe { HidD_SetFeature(handle.0, report.as_ptr().cast(), report.len() as u32) } {
            Ok(())
        } else {
            Err(last_error())
        }
    }
}
