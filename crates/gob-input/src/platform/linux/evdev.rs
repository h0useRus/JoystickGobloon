//! Thin wrapper over the evdev character devices (`/dev/input/event*`): the ioctls that
//! describe a device and non-blocking reads of its events.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_REP: u16 = 0x14;
pub const SYN_REPORT: u16 = 0;
pub const SYN_DROPPED: u16 = 3;
pub const KEY_CNT: usize = 0x300;
pub const ABS_CNT: usize = 0x40;
const PROP_CNT: usize = 0x20;

/// `struct input_event`.
pub type Event = libc::input_event;

/// What a device reports, from its capability bits.
#[derive(Debug, Clone, Default)]
pub struct Caps {
    pub events: Bits,
    pub keys: Bits,
    pub abs: Bits,
    pub props: Bits,
}

/// A kernel bit mask (`EVIOCGBIT` and friends).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bits(pub Vec<u8>);

impl Bits {
    pub fn has(&self, bit: usize) -> bool {
        self.0
            .get(bit / 8)
            .is_some_and(|b| b & (1 << (bit % 8)) != 0)
    }

    pub fn any(&self, bits: std::ops::Range<usize>) -> bool {
        bits.into_iter().any(|b| self.has(b))
    }

    #[cfg(test)]
    pub fn of(bits: &[usize]) -> Self {
        let mut v = vec![0u8; KEY_CNT / 8];
        for &b in bits {
            v[b / 8] |= 1 << (b % 8);
        }
        Self(v)
    }
}

/// An open event device.
pub struct Node {
    file: File,
    pub path: PathBuf,
}

impl Node {
    /// Opens the device for non-blocking reads; a hidden device's node through the opener.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = super::open_node(path)?;
        Ok(Self {
            file,
            path: path.to_owned(),
        })
    }

    /// Like [`Node::open`], but never asks the opener: for nodes that are never hidden.
    pub fn open_direct(path: &Path) -> io::Result<Self> {
        let file = super::open_direct(path)?;
        Ok(Self {
            file,
            path: path.to_owned(),
        })
    }

    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// `ioctl` with a `_IOC(_IOC_READ, 'E', nr, len)` request into `buf`.
    fn read_ioctl(&self, nr: u32, buf: &mut [u8]) -> io::Result<usize> {
        // _IOC_READ << 30 | size << 16 | 'E' << 8 | nr (the generic layout: x86, ARM).
        let request = (2u32 << 30) | ((buf.len() as u32) << 16) | (u32::from(b'E') << 8) | nr;
        // SAFETY: the request reads at most `buf.len()` bytes into `buf`.
        let n = unsafe { libc::ioctl(self.fd(), request as _, buf.as_mut_ptr()) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    fn string(&self, nr: u32) -> String {
        let mut buf = [0u8; 256];
        match self.read_ioctl(nr, &mut buf) {
            Ok(_) => {
                let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                String::from_utf8_lossy(&buf[..end]).trim().to_string()
            }
            Err(_) => String::new(),
        }
    }

    fn bits(&self, nr: u32, count: usize) -> Bits {
        let mut buf = vec![0u8; count.div_ceil(8)];
        if self.read_ioctl(nr, &mut buf).is_err() {
            buf.fill(0);
        }
        Bits(buf)
    }

    /// `EVIOCGNAME`.
    pub fn name(&self) -> String {
        self.string(0x06)
    }

    /// `EVIOCGPHYS`: where the device is attached (e.g. `usb-0000:00:14.0-2/input0`).
    pub fn phys(&self) -> String {
        self.string(0x07)
    }

    /// `EVIOCGUNIQ`: the serial number, often empty.
    pub fn uniq(&self) -> String {
        self.string(0x08)
    }

    /// `EVIOCGID`: (bus type, vendor, product, version).
    pub fn id(&self) -> [u16; 4] {
        let mut buf = [0u8; 8];
        let _ = self.read_ioctl(0x02, &mut buf);
        std::array::from_fn(|i| u16::from_ne_bytes([buf[i * 2], buf[i * 2 + 1]]))
    }

    pub fn caps(&self) -> Caps {
        Caps {
            events: self.bits(0x20, 32),
            keys: self.bits(0x20 + u32::from(EV_KEY), KEY_CNT),
            abs: self.bits(0x20 + u32::from(EV_ABS), ABS_CNT),
            props: self.bits(0x09, PROP_CNT),
        }
    }

    /// `EVIOCGRAB`: takes the device for this file alone (other readers, joydev included, get
    /// no events), or lets go of it. Closing the file lets go as well.
    pub fn grab(&self, on: bool) -> io::Result<()> {
        // _IOW('E', 0x90, int)
        let request = (1u32 << 30)
            | ((size_of::<libc::c_int>() as u32) << 16)
            | (u32::from(b'E') << 8)
            | 0x90;
        // SAFETY: EVIOCGRAB takes an int by value.
        let n = unsafe { libc::ioctl(self.fd(), request as _, libc::c_int::from(on)) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// `EVIOCGKEY`: the keys and buttons held down right now.
    pub fn pressed(&self) -> Bits {
        self.bits(0x18, KEY_CNT)
    }

    /// `EVIOCGABS`: (value, minimum, maximum) of an axis.
    pub fn abs_info(&self, code: usize) -> Option<(i32, i32, i32)> {
        let mut buf = [0u8; size_of::<libc::input_absinfo>()];
        self.read_ioctl(0x40 + code as u32, &mut buf).ok()?;
        // SAFETY: the kernel filled `buf` with a `struct input_absinfo`.
        let info: libc::input_absinfo = unsafe { std::ptr::read_unaligned(buf.as_ptr().cast()) };
        Some((info.value, info.minimum, info.maximum))
    }

    /// Reads the waiting events into `buf`: `Ok(0)` when there are none, `Err` when the
    /// device is gone.
    pub fn read(&self, buf: &mut [Event]) -> io::Result<usize> {
        let size = size_of_val(buf);
        // SAFETY: `buf` is valid for `size` bytes of `input_event`s.
        let n = unsafe { libc::read(self.fd(), buf.as_mut_ptr().cast(), size) };
        if n < 0 {
            let err = io::Error::last_os_error();
            return match err.kind() {
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted => Ok(0),
                _ => Err(err),
            };
        }
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(n as usize / size_of::<Event>())
    }
}

/// Every `/dev/input/event*` node, in number order.
pub fn event_nodes() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir("/dev/input") else {
        return Vec::new();
    };
    let mut nodes: Vec<(u32, PathBuf)> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let n = name.to_str()?.strip_prefix("event")?.parse().ok()?;
            Some((n, e.path()))
        })
        .collect();
    nodes.sort();
    nodes.into_iter().map(|(_, p)| p).collect()
}

/// Waits until one of `fds` can be read or `timeout_ms` passed.
pub fn wait_readable(fds: &mut [libc::pollfd], timeout_ms: i32) {
    // SAFETY: `fds` is a valid array of `fds.len()` pollfd entries.
    unsafe {
        libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms);
    }
}
