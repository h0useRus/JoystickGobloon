//! Virtual input devices on Linux through uinput (`/dev/uinput`): the kernel creates an evdev
//! device that games and the desktop see like a real one. Backend for gob-virtual (joysticks)
//! and gob-engine (keyboard and mouse output); `keys` maps key codes to Gremlin's scan codes.
//! No driver to install; writing `/dev/uinput` needs root or a udev rule (`access`).
//!
//! Example `uinput_check`: creates a joystick (or with `--gamepad` an Xbox-style gamepad) for a
//! few seconds (shows whether uinput works).
#![cfg(target_os = "linux")]

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

pub mod access;
pub mod codes;
pub mod gamepad;
pub mod keys;

const UINPUT: &str = "/dev/uinput";
/// `UINPUT_MAX_NAME_SIZE`.
const MAX_NAME: usize = 80;

/// `ioctl` request numbers: `_IOC(dir, 'U', nr, size)` in the generic layout (x86, ARM).
mod request {
    const fn ioc(dir: u32, nr: u32, size: usize) -> u32 {
        (dir << 30) | ((size as u32) << 16) | ((b'U' as u32) << 8) | nr
    }
    const WRITE: u32 = 1;
    const READ: u32 = 2;
    const INT: usize = size_of::<libc::c_int>();

    pub const DEV_CREATE: u32 = ioc(0, 1, 0);
    pub const DEV_DESTROY: u32 = ioc(0, 2, 0);
    pub const DEV_SETUP: u32 = ioc(WRITE, 3, size_of::<super::Setup>());
    pub const ABS_SETUP: u32 = ioc(WRITE, 4, size_of::<super::AbsSetup>());
    pub const SET_EVBIT: u32 = ioc(WRITE, 100, INT);
    pub const SET_KEYBIT: u32 = ioc(WRITE, 101, INT);
    pub const SET_RELBIT: u32 = ioc(WRITE, 102, INT);
    pub const SET_ABSBIT: u32 = ioc(WRITE, 103, INT);
    pub const fn get_sysname(len: usize) -> u32 {
        ioc(READ, 44, len)
    }
}

/// `struct uinput_setup`.
#[repr(C)]
struct Setup {
    id: [u16; 4],
    name: [u8; MAX_NAME],
    ff_effects_max: u32,
}

/// `struct uinput_abs_setup`.
#[repr(C)]
struct AbsSetup {
    code: u16,
    info: libc::input_absinfo,
}

/// An absolute axis and its range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Axis {
    pub code: u16,
    pub min: i32,
    pub max: i32,
}

/// What a new device reports.
#[derive(Debug, Clone, Default)]
pub struct Spec<'a> {
    /// Device name; longer than 79 bytes is cut.
    pub name: &'a str,
    /// `BUS_*` (e.g. [`codes::BUS_VIRTUAL`]).
    pub bus: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
    /// `EV_KEY` codes: keys and buttons.
    pub keys: &'a [u16],
    /// `EV_ABS` axes.
    pub axes: &'a [Axis],
    /// `EV_REL` codes (mouse motion and wheels).
    pub rel: &'a [u16],
}

/// A virtual device; the kernel removes it when this is dropped.
#[derive(Debug)]
pub struct VirtualDevice {
    file: File,
}

fn ioctl_int(file: &File, request: u32, value: u16) -> io::Result<()> {
    // SAFETY: the UI_SET_*BIT requests take an int argument.
    check(unsafe { libc::ioctl(file.as_raw_fd(), request as _, libc::c_int::from(value)) })
}

fn ioctl_ptr<T>(file: &File, request: u32, value: &T) -> io::Result<()> {
    // SAFETY: `value` is the struct `request` reads.
    check(unsafe { libc::ioctl(file.as_raw_fd(), request as _, std::ptr::from_ref(value)) })
}

fn check(result: libc::c_int) -> io::Result<()> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// std opens with `O_CLOEXEC`, so programs we start (e.g. an installer) do not inherit devices.
fn open() -> io::Result<File> {
    OpenOptions::new().write(true).open(UINPUT)
}

/// Whether devices can be created: `/dev/uinput` exists and is writable.
pub fn available() -> io::Result<()> {
    open().map(drop)
}

impl VirtualDevice {
    pub fn create(spec: &Spec) -> io::Result<Self> {
        let file = open()?;
        let events = [
            (!spec.keys.is_empty(), codes::EV_KEY),
            (!spec.axes.is_empty(), codes::EV_ABS),
            (!spec.rel.is_empty(), codes::EV_REL),
        ];
        for (_, ev) in events.iter().filter(|(on, _)| *on) {
            ioctl_int(&file, request::SET_EVBIT, *ev)?;
        }
        for &key in spec.keys {
            ioctl_int(&file, request::SET_KEYBIT, key)?;
        }
        for &rel in spec.rel {
            ioctl_int(&file, request::SET_RELBIT, rel)?;
        }
        for axis in spec.axes {
            ioctl_int(&file, request::SET_ABSBIT, axis.code)?;
            let info = libc::input_absinfo {
                value: ((i64::from(axis.min) + i64::from(axis.max) + 1) / 2) as i32,
                minimum: axis.min,
                maximum: axis.max,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            };
            let setup = AbsSetup {
                code: axis.code,
                info,
            };
            ioctl_ptr(&file, request::ABS_SETUP, &setup)?;
        }
        let mut setup = Setup {
            id: [spec.bus, spec.vendor, spec.product, spec.version],
            name: [0; MAX_NAME],
            ff_effects_max: 0,
        };
        let name = truncate(spec.name, MAX_NAME - 1);
        setup.name[..name.len()].copy_from_slice(name.as_bytes());
        ioctl_ptr(&file, request::DEV_SETUP, &setup)?;
        // SAFETY: UI_DEV_CREATE has no argument.
        check(unsafe { libc::ioctl(file.as_raw_fd(), request::DEV_CREATE as _) })?;
        Ok(Self { file })
    }

    /// Sends events as one report (`SYN_REPORT` is added): (type, code, value).
    pub fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity((events.len() + 1) * size_of::<libc::input_event>());
        for &(type_, code, value) in events
            .iter()
            .chain(&[(codes::EV_SYN, codes::SYN_REPORT, 0)])
        {
            // SAFETY: `input_event` is plain data; all zeroes is a valid value (the kernel
            // sets the time).
            let mut e: libc::input_event = unsafe { std::mem::zeroed() };
            (e.type_, e.code, e.value) = (type_, code, value);
            // SAFETY: viewing a plain struct as its bytes.
            bytes.extend_from_slice(unsafe {
                std::slice::from_raw_parts(
                    std::ptr::from_ref(&e).cast::<u8>(),
                    size_of::<libc::input_event>(),
                )
            });
        }
        (&self.file).write_all(&bytes)
    }

    /// The device's sysfs directory, e.g. `/sys/devices/virtual/input/input42`.
    pub fn sys_path(&self) -> io::Result<PathBuf> {
        let mut buf = [0u8; 64];
        // SAFETY: the request writes at most `buf.len()` bytes into `buf`.
        let n = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request::get_sysname(buf.len()) as _,
                buf.as_mut_ptr(),
            )
        };
        check(n)?;
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let name = String::from_utf8_lossy(&buf[..end]);
        Ok(Path::new("/sys/devices/virtual/input").join(name.as_ref()))
    }

    /// The device's evdev node (`/dev/input/eventN`), once the kernel has made it.
    pub fn event_node(&self) -> io::Result<PathBuf> {
        let dir = std::fs::read_dir(self.sys_path()?)?;
        dir.flatten()
            .find_map(|e| {
                let name = e.file_name();
                let name = name.to_str()?;
                name.starts_with("event")
                    .then(|| Path::new("/dev/input").join(name))
            })
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }
}

impl Drop for VirtualDevice {
    fn drop(&mut self) {
        // SAFETY: UI_DEV_DESTROY has no argument. Closing the file would remove it as well.
        unsafe { libc::ioctl(self.file.as_raw_fd(), request::DEV_DESTROY as _) };
    }
}

/// Names of every input device on the system (`/sys/class/input/input*/name`), e.g. to find
/// devices another program created.
pub fn device_names() -> Vec<String> {
    let Ok(dir) = std::fs::read_dir("/sys/class/input") else {
        return Vec::new();
    };
    dir.flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("input"))
        })
        .filter_map(|e| std::fs::read_to_string(e.path().join("name")).ok())
        .map(|n| n.trim_end().to_string())
        .collect()
}

/// `s` cut to at most `max` bytes at a character boundary.
fn truncate(s: &str, max: usize) -> &str {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_numbers_match_the_kernel() {
        // From linux/uinput.h on x86_64.
        assert_eq!(request::DEV_CREATE, 0x5501);
        assert_eq!(request::DEV_DESTROY, 0x5502);
        assert_eq!(request::DEV_SETUP, 0x405c_5503);
        assert_eq!(request::ABS_SETUP, 0x401c_5504);
        assert_eq!(request::SET_EVBIT, 0x4004_5564);
        assert_eq!(request::SET_ABSBIT, 0x4004_5567);
        assert_eq!(request::get_sysname(64), 0x8040_552c);
    }

    #[test]
    fn names_are_cut_at_characters() {
        assert_eq!(truncate("abc", 2), "ab");
        assert_eq!(truncate("аб", 3), "а");
        assert_eq!(truncate("ok", 79), "ok");
    }
}
