//! The HidHide control device `\\.\HidHide` and its IOCTLs (`ref/HidHide/Shared/HidHideIoctlContract.h`).
//!
//! The device is opened for each operation and closed right away: the driver lets few clients
//! hold it at once (HidHide's own client keeps it open while running).

use crate::Error;

/// `CTL_CODE(32769, n, METHOD_BUFFERED, FILE_READ_DATA)`.
const fn ioctl(n: u32) -> u32 {
    (32769 << 16) | (1 << 14) | (n << 2)
}

pub const GET_WHITELIST: u32 = ioctl(2048);
pub const SET_WHITELIST: u32 = ioctl(2049);
pub const GET_BLACKLIST: u32 = ioctl(2050);
pub const SET_BLACKLIST: u32 = ioctl(2051);
pub const GET_ACTIVE: u32 = ioctl(2052);
pub const SET_ACTIVE: u32 = ioctl(2053);
pub const GET_WLINVERSE: u32 = ioctl(2054);
pub const SET_WLINVERSE: u32 = ioctl(2055);

/// UTF-16 `REG_MULTI_SZ`: each string with its terminator, then one more 0.
pub fn to_multi_sz(items: &[String]) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for item in items.iter().filter(|s| !s.is_empty()) {
        out.extend(item.encode_utf16());
        out.push(0);
    }
    out.push(0);
    out
}

/// Strings of a `REG_MULTI_SZ` buffer; empty strings (the terminators) are skipped.
pub fn from_multi_sz(buffer: &[u16]) -> Vec<String> {
    buffer
        .split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_SHARING_VIOLATION,
        GENERIC_READ, HANDLE,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;
    use windows::core::w;

    /// An open control device; closed on drop.
    pub struct Driver(HANDLE);

    impl Drop for Driver {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateFileW and is closed once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    impl Driver {
        /// Opens `\\.\HidHide`. A busy device (another client, our own other thread) is
        /// retried briefly before giving up.
        pub fn open() -> Result<Self, Error> {
            let mut tries = 0;
            loop {
                // SAFETY: plain CreateFileW on a constant device name.
                let handle = unsafe {
                    CreateFileW(
                        w!(r"\\.\HidHide"),
                        GENERIC_READ.0,
                        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                        None,
                        OPEN_EXISTING,
                        FILE_ATTRIBUTE_NORMAL,
                        None,
                    )
                };
                match handle {
                    Ok(h) => return Ok(Self(h)),
                    Err(e) if e.code() == ERROR_FILE_NOT_FOUND.to_hresult() => {
                        return Err(Error::NotInstalled);
                    }
                    Err(e)
                        if tries < 10
                            && (e.code() == ERROR_ACCESS_DENIED.to_hresult()
                                || e.code() == ERROR_SHARING_VIOLATION.to_hresult()) =>
                    {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_millis(30));
                    }
                    Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => {
                        return Err(Error::Driver(
                            "the HidHide device is in use by another program".into(),
                        ));
                    }
                    Err(e) => return Err(Error::Driver(format!("opening HidHide: {e}"))),
                }
            }
        }

        fn control(
            &self,
            code: u32,
            input: Option<&[u8]>,
            output: Option<&mut [u8]>,
        ) -> Result<u32, Error> {
            let mut returned = 0u32;
            let (in_ptr, in_len) =
                input.map_or((None, 0), |b| (Some(b.as_ptr().cast()), b.len() as u32));
            let (out_ptr, out_len) = match output {
                Some(b) => (Some(b.as_mut_ptr().cast()), b.len() as u32),
                None => (None, 0),
            };
            // SAFETY: the buffers outlive the synchronous call and their lengths match.
            unsafe {
                DeviceIoControl(
                    self.0,
                    code,
                    in_ptr,
                    in_len,
                    out_ptr,
                    out_len,
                    Some(&mut returned),
                    None,
                )
            }
            .map_err(|e| Error::Driver(format!("IOCTL {:#x}: {e}", code)))?;
            Ok(returned)
        }

        pub fn get_bool(&self, code: u32) -> Result<bool, Error> {
            let mut b = [0u8; 1];
            let n = self.control(code, None, Some(&mut b))?;
            if n != 1 {
                return Err(Error::Driver(format!("IOCTL {code:#x}: {n} bytes")));
            }
            Ok(b[0] != 0)
        }

        pub fn set_bool(&self, code: u32, on: bool) -> Result<(), Error> {
            self.control(code, Some(&[u8::from(on)]), None).map(drop)
        }

        /// A list: the first call asks for the size, the second reads it.
        pub fn get_list(&self, code: u32) -> Result<Vec<String>, Error> {
            let needed = self.control(code, None, None)? as usize;
            let mut buffer = vec![0u8; needed];
            let n = self.control(code, None, Some(&mut buffer))? as usize;
            let wide: Vec<u16> = buffer[..n.min(needed)]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(from_multi_sz(&wide))
        }

        /// Replaces the whole list.
        pub fn set_list(&self, code: u32, items: &[String]) -> Result<(), Error> {
            let bytes: Vec<u8> = to_multi_sz(items)
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect();
            self.control(code, Some(&bytes), None).map(drop)
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub struct Driver;

    impl Driver {
        pub fn open() -> Result<Self, Error> {
            Err(Error::NotInstalled)
        }
        pub fn get_bool(&self, _code: u32) -> Result<bool, Error> {
            Err(Error::NotInstalled)
        }
        pub fn set_bool(&self, _code: u32, _on: bool) -> Result<(), Error> {
            Err(Error::NotInstalled)
        }
        pub fn get_list(&self, _code: u32) -> Result<Vec<String>, Error> {
            Err(Error::NotInstalled)
        }
        pub fn set_list(&self, _code: u32, _items: &[String]) -> Result<(), Error> {
            Err(Error::NotInstalled)
        }
    }
}

pub use imp::Driver;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_codes() {
        // CTL_CODE(32769, 2048, 0, 1) as computed by the C macro.
        assert_eq!(GET_WHITELIST, 0x8001_6000);
        assert_eq!(SET_WLINVERSE, 0x8001_601C);
    }

    #[test]
    fn multi_sz_round_trip() {
        assert_eq!(to_multi_sz(&[]), [0]);
        assert!(from_multi_sz(&[0]).is_empty());
        assert!(from_multi_sz(&[0, 0]).is_empty());
        let one = vec![r"\Device\HarddiskVolume3\a.exe".to_string()];
        let sz = to_multi_sz(&one);
        assert_eq!(sz.len(), one[0].encode_utf16().count() + 2);
        assert_eq!(from_multi_sz(&sz), one);
        let two = vec![
            r"\Device\HarddiskVolume3\Игры\игра.exe".to_string(),
            r"HID\VID_231D&PID_0200\a&2f28a505&1&0000".to_string(),
        ];
        assert_eq!(from_multi_sz(&to_multi_sz(&two)), two);
    }
}
