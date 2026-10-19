//! vJoy straight through its driver, without vJoyInterface.dll (docs/VJOY_PLAN.md).
//!
//! - Control channel `<namespace>device_000` (interface `GUID_DEVINTERFACE_VJOY`): driver info and
//!   per-device status (exists, opened by which process).
//! - Capabilities (axes, buttons, hats) and the driver version come from the HID collections the
//!   driver exposes (VID 0x1234, PID 0xBEAD), one per vJoy device, report id = device id.
//!
//! - Output: opening `<namespace>device_NNN` acquires device N; `LOAD_POSITIONS` sends the whole
//!   state (`Position`, `JOYSTICK_POSITION_V2`), `RESET_DEV` resets it, closing releases it.

use crate::{AXIS_USAGES, DeviceInfo, MAX_DEVICES, Status};

/// `CTL_CODE(FILE_DEVICE_UNKNOWN, f, METHOD_BUFFERED, access)`.
const fn ioctl(f: u32, access: u32) -> u32 {
    (0x22 << 16) | (access << 14) | (f << 2)
}

const FILE_ANY_ACCESS: u32 = 0;
const FILE_READ_ACCESS: u32 = 1;
const FILE_WRITE_ACCESS: u32 = 2;
pub const LOAD_POSITIONS: u32 = ioctl(0x910, FILE_WRITE_ACCESS);
pub const RESET_DEV: u32 = ioctl(0x91F, FILE_WRITE_ACCESS);
pub const GET_POSITIONS: u32 = ioctl(0x920, FILE_READ_ACCESS);
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

/// `JOYSTICK_POSITION_V2` from vJoy's `public.h` (natural C alignment, 108 bytes): the whole
/// state of one device, sent at once.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    pub device: u8,
    pub throttle: i32,
    pub rudder: i32,
    pub aileron: i32,
    /// X, Y, Z, Rx, Ry, Rz, Slider, Dial: axis slots 1..=8.
    pub axes: [i32; 8],
    pub wheel: i32,
    pub vx: i32,
    pub vy: i32,
    pub vz: i32,
    pub vbrx: i32,
    pub vbry: i32,
    pub vbrz: i32,
    /// Buttons 1..=32 (bit 0 = button 1).
    pub buttons: u32,
    /// Hat 1 (continuous), or the 4 discrete hats as nibbles.
    pub hats: u32,
    /// Continuous hats 2..=4.
    pub hats_ex: [u32; 3],
    /// Buttons 33..=128.
    pub buttons_ex: [u32; 3],
}

impl Position {
    pub fn set_button(&mut self, n: u8, pressed: bool) -> bool {
        let i = usize::from(n.wrapping_sub(1));
        let word = match i / 32 {
            0 => &mut self.buttons,
            1..=3 => &mut self.buttons_ex[i / 32 - 1],
            _ => return false,
        };
        let bit = 1u32 << (i % 32);
        if pressed {
            *word |= bit;
        } else {
            *word &= !bit;
        }
        true
    }

    /// Continuous hat `n` (1..=4): hundredths of a degree, `u32::MAX` centered.
    pub fn set_cont_hat(&mut self, n: u8, raw: u32) -> bool {
        match n {
            1 => self.hats = raw,
            2..=4 => self.hats_ex[usize::from(n) - 2] = raw,
            _ => return false,
        }
        true
    }

    /// Discrete hat `n` (1..=4): 0 up, 1 right, 2 down, 3 left, -1 centered (nibble 0xF).
    pub fn set_disc_hat(&mut self, n: u8, value: i32) -> bool {
        if !(1..=4).contains(&n) {
            return false;
        }
        let shift = 4 * u32::from(n - 1);
        self.hats = (self.hats & !(0xF << shift)) | (((value as u32) & 0xF) << shift);
        true
    }

    fn to_bytes(self) -> [u8; size_of::<Position>()] {
        // SAFETY: plain `repr(C)` integers; the padding after `device` is never read by the
        // driver.
        unsafe { std::mem::transmute(self) }
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let bytes: [u8; size_of::<Position>()] =
            bytes.get(..size_of::<Position>())?.try_into().ok()?;
        // SAFETY: every bit pattern is a valid `Position`.
        Some(unsafe { std::mem::transmute::<[u8; size_of::<Position>()], Position>(bytes) })
    }
}

/// An acquired device: its open handle and the state last sent. Released when dropped.
pub struct Acquired {
    handle: imp::Handle,
    pub id: u8,
    pub position: Position,
}

impl Acquired {
    /// Sends `position` to the driver.
    pub fn send(&mut self) -> bool {
        self.position.device = self.id;
        imp::ioctl(&self.handle, LOAD_POSITIONS, &self.position.to_bytes(), 0).is_some()
    }

    /// Driver reset (axes and hats to their start values, buttons up). `RESET_DEV` only
    /// changes the driver's copy: the state is read back and sent, so games see it too.
    pub fn reset(&mut self) -> bool {
        let ok = imp::ioctl(&self.handle, RESET_DEV, &[], 0).is_some();
        if let Some(p) = imp::ioctl(&self.handle, GET_POSITIONS, &[], size_of::<Position>())
            .and_then(|b| Position::from_bytes(&b))
        {
            self.position = p;
        }
        ok && self.send()
    }
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

    /// Acquires device `id` (1..=16): `None` when it is missing or another program holds it.
    pub fn acquire(&self, id: u8) -> Option<Acquired> {
        let handle = imp::open_device(&self.namespace, id)?;
        let mut device = Acquired {
            handle,
            id,
            position: Position::default(),
        };
        device.reset();
        Some(device)
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
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
        SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList,
        SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
    };
    use windows::Win32::Devices::HumanInterfaceDevice::{
        HIDD_ATTRIBUTES, HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS,
        HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid, HidD_GetPreparsedData,
        HidP_GetButtonCaps, HidP_GetCaps, HidP_GetValueCaps, HidP_Input, PHIDP_PREPARSED_DATA,
    };
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;
    use windows::core::{GUID, PCWSTR};

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
        unsafe {
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
    }

    /// The driver's interface path up to its last `\`: device N is `<namespace>device_NNN`.
    pub fn namespace() -> Option<String> {
        let path = interface_paths(&GUID_DEVINTERFACE_VJOY)
            .into_iter()
            .next()?;
        let cut = path.rfind('\\')? + 1;
        Some(path[..cut].to_string())
    }

    /// An open device handle, closed on drop.
    pub struct Handle(HANDLE);

    // SAFETY: a file handle may be used from any thread; calls on it are synchronous.
    unsafe impl Send for Handle {}

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateFileW and is closed once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    /// Opens `device_NNN` for writing: this acquires it. Fails while another program has it.
    pub fn open_device(namespace: &str, id: u8) -> Option<Handle> {
        open(
            &format!("{namespace}device_{id:03}"),
            GENERIC_READ.0 | GENERIC_WRITE.0,
        )
        .map(Handle)
    }

    /// One IOCTL on an open device; the bytes it returned (`None` on failure).
    pub fn ioctl(handle: &Handle, code: u32, input: &[u8], out_len: usize) -> Option<Vec<u8>> {
        let mut out = vec![0u8; out_len];
        let mut returned = 0u32;
        // SAFETY: buffers outlive the synchronous call.
        unsafe {
            DeviceIoControl(
                handle.0,
                code,
                (!input.is_empty()).then(|| input.as_ptr().cast()),
                input.len() as u32,
                (out_len > 0).then(|| out.as_mut_ptr().cast()),
                out_len as u32,
                Some(&mut returned),
                None,
            )
            .ok()?;
        }
        out.truncate(returned as usize);
        Some(out)
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
    pub struct Handle;
    pub fn open_device(_: &str, _: u8) -> Option<Handle> {
        None
    }
    pub fn ioctl(_: &Handle, _: u32, _: &[u8], _: usize) -> Option<Vec<u8>> {
        None
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
        assert_eq!(LOAD_POSITIONS, 0x0022_A440);
        assert_eq!(RESET_DEV, 0x0022_A47C);
        assert_eq!(GET_POSITIONS, 0x0022_6480);
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
    fn position_layout() {
        // JOYSTICK_POSITION_V2 with natural alignment.
        assert_eq!(size_of::<Position>(), 108);
        assert_eq!(std::mem::offset_of!(Position, axes), 16);
        assert_eq!(std::mem::offset_of!(Position, buttons), 76);
        assert_eq!(std::mem::offset_of!(Position, buttons_ex), 96);
        let mut p = Position::default();
        assert!(p.set_button(1, true) && p.set_button(33, true) && p.set_button(128, true));
        assert_eq!((p.buttons, p.buttons_ex), (1, [1, 0, 0x8000_0000]));
        assert!(!p.set_button(0, true) && !p.set_button(129, true));
        p.set_disc_hat(2, -1);
        p.set_disc_hat(1, 3);
        assert_eq!(p.hats, 0xF3);
        assert!(p.set_cont_hat(4, 9000));
        assert_eq!(p.hats_ex[2], 9000);
        let bytes = p.to_bytes();
        assert_eq!(Position::from_bytes(&bytes), Some(p));
    }

    #[test]
    fn versions_and_axes() {
        assert_eq!(format_version(0x0219), "2.1.9");
        assert_eq!(axis_slot(0x30), Some(1));
        assert_eq!(axis_slot(0x37), Some(8));
        assert_eq!(axis_slot(0x39), None);
    }
}
