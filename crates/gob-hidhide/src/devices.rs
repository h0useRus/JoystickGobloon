//! HID devices known to Windows, present or not, grouped by physical device like HidHide's
//! `--dev-all` / `--dev-gaming` (`ref/HidHide/HidHideCLI/src/HID.cpp`).

use crate::{DeviceGroup, HidDevice};

/// Friendly name of a group: the words of its vendor and product strings, each once.
fn merge_words(model: &mut Vec<String>, text: &str) {
    for word in text.split_whitespace() {
        if !model.iter().any(|w| w.eq_ignore_ascii_case(word)) {
            model.push(word.to_string());
        }
    }
}

/// A HID collection with the extra facts used for grouping.
pub struct Found {
    pub device: HidDevice,
    pub description: String,
    /// Name of the base container from Windows, for groups without vendor/product strings.
    pub container_name: String,
}

/// Groups collections by base container (each collection without one alone), sorted by name.
pub fn group(found: Vec<Found>, gaming_only: bool) -> Vec<DeviceGroup> {
    let mut groups: Vec<(DeviceGroup, Vec<String>, String)> = Vec::new();
    for f in found {
        let base = f.device.base_container_device_instance_path.clone();
        let existing = (!base.is_empty())
            .then(|| {
                groups.iter().position(|(g, ..)| {
                    g.devices[0]
                        .base_container_device_instance_path
                        .eq_ignore_ascii_case(&base)
                })
            })
            .flatten();
        let i = existing.unwrap_or_else(|| {
            groups.push((
                DeviceGroup {
                    friendly_name: String::new(),
                    devices: Vec::new(),
                },
                Vec::new(),
                String::new(),
            ));
            groups.len() - 1
        });
        let (g, model, fallback) = &mut groups[i];
        merge_words(model, &f.device.vendor);
        merge_words(model, &f.device.product);
        if fallback.is_empty() {
            *fallback = if f.container_name.is_empty() {
                f.description.clone()
            } else {
                f.container_name.clone()
            };
        }
        g.devices.push(f.device);
    }
    let mut result: Vec<DeviceGroup> = groups
        .into_iter()
        .map(|(mut g, model, fallback)| {
            g.friendly_name = if model.is_empty() {
                fallback.trim().to_string()
            } else {
                model.join(" ")
            };
            g
        })
        .filter(|g| !gaming_only || g.gaming())
        .collect();
    result.sort_by_key(|g| g.friendly_name.to_lowercase());
    result
}

/// `(usage page, usage)` from a HID id like `HID_DEVICE_UP:0001_U:0004` or
/// `HID\VID_231D&UP:0001_U:0004`.
fn parse_usage(id: &str) -> Option<(u16, u16)> {
    let start = id.find("UP:")? + 3;
    let (page, usage) = id[start..].split_once("_U:")?;
    Some((
        u16::from_str_radix(page, 16).ok()?,
        u16::from_str_radix(usage.get(..4)?, 16).ok()?,
    ))
}

#[cfg(windows)]
pub fn find() -> Vec<Found> {
    imp::find()
}

#[cfg(not(windows))]
pub fn find() -> Vec<Found> {
    Vec::new()
}

#[cfg(windows)]
mod imp {
    use super::Found;
    use crate::HidDevice;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_GETIDLIST_FILTER_CLASS, CM_Get_DevNode_PropertyW, CM_Get_Device_ID_List_SizeW,
        CM_Get_Device_ID_ListW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_LOCATE_DEVNODE_PHANTOM,
        CM_Locate_DevNodeW, CR_SUCCESS, DIGCF_DEVICEINTERFACE, GUID_DEVCLASS_HIDCLASS,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList,
        SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
    };
    use windows::Win32::Devices::HumanInterfaceDevice::{
        HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS, HidD_FreePreparsedData,
        HidD_GetAttributes, HidD_GetHidGuid, HidD_GetManufacturerString, HidD_GetPreparsedData,
        HidD_GetProductString, HidP_GetCaps, PHIDP_PREPARSED_DATA,
    };
    use windows::Win32::Devices::Properties::{
        DEVPKEY_Device_BusReportedDeviceDesc, DEVPKEY_Device_CompatibleIds,
        DEVPKEY_Device_ContainerId, DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_FriendlyName,
        DEVPKEY_Device_HardwareIds, DEVPKEY_Device_InstanceId, DEVPROP_TYPE_GUID,
        DEVPROP_TYPE_STRING, DEVPROP_TYPE_STRING_LIST, DEVPROPTYPE,
    };
    use windows::Win32::Foundation::{CloseHandle, DEVPROPKEY};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::core::{GUID, PCWSTR};

    /// Container of devices that are part of the PC itself: not a separate device.
    const CONTAINER_ID_SYSTEM: GUID = GUID::from_u128(0x00000000_0000_0000_ffff_ffffffffffff);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    fn from_wide(s: &[u16]) -> String {
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        String::from_utf16_lossy(&s[..end])
    }

    fn devnode(instance: &str, phantom: bool) -> Option<u32> {
        let id = wide(instance);
        let flags = if phantom {
            CM_LOCATE_DEVNODE_PHANTOM
        } else {
            CM_LOCATE_DEVNODE_NORMAL
        };
        let mut node = 0u32;
        // SAFETY: `id` is a NUL-terminated string living across the call.
        let r = unsafe { CM_Locate_DevNodeW(&mut node, PCWSTR(id.as_ptr()), flags) };
        (r == CR_SUCCESS).then_some(node)
    }

    fn property(node: u32, key: &DEVPROPKEY, want: DEVPROPTYPE) -> Option<Vec<u8>> {
        let mut ty = DEVPROPTYPE(0);
        let mut size = 0u32;
        // SAFETY: first call only reports the size; the second fills a buffer of that size.
        unsafe {
            let _ = CM_Get_DevNode_PropertyW(node, key, &mut ty, None, &mut size, 0);
            if size == 0 {
                return None;
            }
            let mut buffer = vec![0u8; size as usize];
            let r = CM_Get_DevNode_PropertyW(
                node,
                key,
                &mut ty,
                Some(buffer.as_mut_ptr()),
                &mut size,
                0,
            );
            (r == CR_SUCCESS && ty == want).then_some(buffer)
        }
    }

    fn to_wide(bytes: &[u8]) -> Vec<u16> {
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    fn string_property(node: u32, key: &DEVPROPKEY) -> Option<String> {
        let bytes = property(node, key, DEVPROP_TYPE_STRING)?;
        Some(from_wide(&to_wide(&bytes)).trim().to_string()).filter(|s| !s.is_empty())
    }

    /// Top-level usage from the hardware / compatible ids (`HID\VID_231D&UP:0001_U:0004`):
    /// known even for devices that are disconnected or hidden from this program.
    fn id_usage(node: u32) -> Option<(u16, u16)> {
        [&DEVPKEY_Device_HardwareIds, &DEVPKEY_Device_CompatibleIds]
            .into_iter()
            .filter_map(|key| property(node, key, DEVPROP_TYPE_STRING_LIST))
            .flat_map(|bytes| crate::driver::from_multi_sz(&to_wide(&bytes)))
            .find_map(|id| super::parse_usage(&id))
    }

    fn container_id(instance: &str) -> Option<GUID> {
        let node = devnode(instance, true)?;
        let bytes = property(node, &DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID)?;
        let bytes: [u8; 16] = bytes.get(..16)?.try_into().ok()?;
        // SAFETY: GUID is 16 bytes of plain data, as written by the property call.
        let guid: GUID = unsafe { std::mem::transmute(bytes) };
        (guid != GUID::zeroed()).then_some(guid)
    }

    fn parent(instance: &str) -> Option<String> {
        let node = devnode(instance, true)?;
        let mut parent = 0u32;
        // SAFETY: plain out-parameter call.
        if unsafe { CM_Get_Parent(&mut parent, node, 0) } != CR_SUCCESS {
            return None;
        }
        string_property(parent, &DEVPKEY_Device_InstanceId)
    }

    /// The topmost ancestor in the same container (the USB device of a HID collection).
    fn base_container(instance: &str) -> String {
        let Some(id) = container_id(instance).filter(|id| *id != CONTAINER_ID_SYSTEM) else {
            return String::new();
        };
        let mut current = instance.to_string();
        while let Some(up) = parent(&current) {
            if container_id(&up) != Some(id) {
                break;
            }
            current = up;
        }
        current
    }

    /// Instance ids of the HID class, present or not.
    fn instance_ids() -> Vec<String> {
        let class = wide(&format!("{{{:?}}}", GUID_DEVCLASS_HIDCLASS));
        let mut len = 0u32;
        // SAFETY: size query, then a buffer of that size.
        unsafe {
            if CM_Get_Device_ID_List_SizeW(
                &mut len,
                PCWSTR(class.as_ptr()),
                CM_GETIDLIST_FILTER_CLASS,
            ) != CR_SUCCESS
            {
                return Vec::new();
            }
            let mut buffer = vec![0u16; len as usize];
            if CM_Get_Device_ID_ListW(
                PCWSTR(class.as_ptr()),
                &mut buffer,
                CM_GETIDLIST_FILTER_CLASS,
            ) != CR_SUCCESS
            {
                return Vec::new();
            }
            crate::driver::from_multi_sz(&buffer)
        }
    }

    /// The HID interface path of an instance, `None` for non-HID-interface nodes.
    fn symbolic_link(hid: &GUID, instance: &str) -> Option<String> {
        let id = wide(instance);
        // SAFETY: standard SetupAPI enumeration of one device's interfaces; the list is
        // destroyed before returning.
        unsafe {
            let set =
                SetupDiGetClassDevsW(Some(hid), PCWSTR(id.as_ptr()), None, DIGCF_DEVICEINTERFACE)
                    .ok()?;
            let mut data = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            let mut result = None;
            if SetupDiEnumDeviceInterfaces(set, None, hid, 0, &mut data).is_ok() {
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
                    result = Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                        path, len,
                    )));
                }
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
            result
        }
    }

    fn usage_name(page: u16, usage: u16) -> String {
        match (page, usage) {
            (0x01, 0x01) => "Pointer".into(),
            (0x01, 0x02) => "Mouse".into(),
            (0x01, 0x04) => "Joystick".into(),
            (0x01, 0x05) => "Game Pad".into(),
            (0x01, 0x06) => "Keyboard".into(),
            (0x01, 0x07) => "Keypad".into(),
            (0x01, 0x08) => "Multi-axis Controller".into(),
            (0x0C, 0x01) => "Consumer Control".into(),
            _ => format!("{page:04X} {usage:04X}"),
        }
    }

    fn set_usage(device: &mut HidDevice, page: u16, usage: u16, steam: bool) {
        device.usage = usage_name(page, usage);
        // Like HidHide: game controllers, joysticks, gamepads, Steam Controller/Deck.
        device.gaming_device =
            steam || page == 0x05 || (page == 0x01 && matches!(usage, 0x04 | 0x05));
    }

    /// Strings and capabilities read from the device itself (only when present); false when
    /// it cannot be opened (e.g. hidden from this program).
    fn read_hid(link: &str, device: &mut HidDevice) -> bool {
        let path = wide(link);
        // SAFETY: zero-access open (attributes and strings only); every handle and the
        // preparsed data are released before returning.
        unsafe {
            let Ok(handle) = CreateFileW(
                PCWSTR(path.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) else {
                return false;
            };
            let mut buffer = [0u16; 128];
            let bytes = (buffer.len() * 2) as u32;
            if HidD_GetProductString(handle, buffer.as_mut_ptr().cast(), bytes) {
                device.product = from_wide(&buffer);
            }
            buffer.fill(0);
            if HidD_GetManufacturerString(handle, buffer.as_mut_ptr().cast(), bytes) {
                device.vendor = from_wide(&buffer);
            }
            let mut attributes = HIDD_ATTRIBUTES {
                Size: size_of::<HIDD_ATTRIBUTES>() as u32,
                ..Default::default()
            };
            let have_attributes = HidD_GetAttributes(handle, &mut attributes);
            let mut data = PHIDP_PREPARSED_DATA::default();
            if HidD_GetPreparsedData(handle, &mut data) {
                let mut caps = HIDP_CAPS::default();
                if HidP_GetCaps(data, &mut caps) == HIDP_STATUS_SUCCESS {
                    let steam = have_attributes
                        && attributes.VendorID == 0x28DE
                        && matches!(attributes.ProductID, 0x1205 | 0x1142);
                    set_usage(device, caps.UsagePage, caps.Usage, steam);
                }
                let _ = HidD_FreePreparsedData(data);
            }
            let _ = CloseHandle(handle);
            device.product = device.product.trim().to_string();
            device.vendor = device.vendor.trim().to_string();
            true
        }
    }

    pub fn find() -> Vec<Found> {
        // SAFETY: returns a constant GUID.
        let hid = unsafe { HidD_GetHidGuid() };
        let mut found = Vec::new();
        for instance in instance_ids() {
            let Some(link) = symbolic_link(&hid, &instance) else {
                continue;
            };
            let present = devnode(&instance, false).is_some();
            let node = devnode(&instance, true);
            let description = node
                .and_then(|n| string_property(n, &DEVPKEY_Device_DeviceDesc))
                .unwrap_or_default();
            let base = base_container(&instance);
            let container_name = devnode(&base, true)
                .and_then(|n| {
                    string_property(n, &DEVPKEY_Device_BusReportedDeviceDesc)
                        .or_else(|| string_property(n, &DEVPKEY_Device_FriendlyName))
                })
                .unwrap_or_default();
            let mut device = HidDevice {
                present,
                gaming_device: false,
                vendor: String::new(),
                product: String::new(),
                usage: String::new(),
                device_instance_path: instance,
                base_container_device_instance_path: base,
            };
            if !(present && read_hid(&link, &mut device)) {
                if let Some((page, usage)) = node.and_then(id_usage) {
                    set_usage(&mut device, page, usage, false);
                }
                if !present {
                    device.usage = "absent".into();
                }
            }
            found.push(Found {
                device,
                description,
                container_name,
            });
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(path: &str, base: &str, vendor: &str, product: &str, gaming: bool) -> Found {
        Found {
            device: HidDevice {
                present: true,
                gaming_device: gaming,
                vendor: vendor.into(),
                product: product.into(),
                usage: String::new(),
                device_instance_path: path.into(),
                base_container_device_instance_path: base.into(),
            },
            description: "HID-compliant device".into(),
            container_name: String::new(),
        }
    }

    #[test]
    fn parses_compatible_usage() {
        assert_eq!(parse_usage("HID_DEVICE_UP:0001_U:0004"), Some((1, 4)));
        assert_eq!(parse_usage("HID_DEVICE_UP:FF00_U:0001"), Some((0xFF00, 1)));
        assert_eq!(parse_usage(r"HID\VID_231D&UP:0001_U:0004"), Some((1, 4)));
        assert_eq!(parse_usage("HID_DEVICE_SYSTEM_GAME"), None);
    }

    #[test]
    fn groups_by_container() {
        let groups = group(
            vec![
                found(r"HID\B&Col01\1", r"USB\B\1", "VKB-Sim", "Gladiator R", true),
                found(r"HID\A\1", "", "", "", false),
                found(
                    r"HID\B&Col02\1",
                    r"usb\b\1",
                    "VKB-Sim",
                    "Gladiator  R",
                    false,
                ),
            ],
            false,
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].friendly_name, "HID-compliant device");
        assert_eq!(groups[1].friendly_name, "VKB-Sim Gladiator R");
        assert_eq!(groups[1].devices.len(), 2);
        assert!(groups[1].gaming());
        let gaming = group(vec![found(r"HID\A\1", "", "x", "y", false)], true);
        assert!(gaming.is_empty());
    }
}
