//! Which keys the keyboards hold right now (`EVIOCGKEY` on every evdev keyboard), for keyboard
//! conditions. Needs read access to /dev/input (the `input` group or a udev rule); without it
//! no key counts as held.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Look for new keyboards this often.
const RESCAN: Duration = Duration::from_secs(2);
const KEY_CNT: usize = 0x300;
const EV_KEY: u32 = 0x01;
/// Keys every keyboard has: Esc, A, Z, Space.
const KEYBOARD_KEYS: [usize; 4] = [1, 30, 44, 57];

struct Keyboards {
    files: Vec<File>,
    scanned: Option<Instant>,
}

static KEYBOARDS: Mutex<Keyboards> = Mutex::new(Keyboards {
    files: Vec::new(),
    scanned: None,
});

/// `ioctl(_IOC(_IOC_READ, 'E', nr, len))` into `buf`.
fn read_bits(file: &File, nr: u32, buf: &mut [u8]) -> bool {
    let request = (2u32 << 30) | ((buf.len() as u32) << 16) | (u32::from(b'E') << 8) | nr;
    // SAFETY: the request writes at most `buf.len()` bytes into `buf`.
    unsafe { libc::ioctl(file.as_raw_fd(), request as _, buf.as_mut_ptr()) >= 0 }
}

fn has(bits: &[u8], bit: usize) -> bool {
    bits.get(bit / 8).is_some_and(|b| b & (1 << (bit % 8)) != 0)
}

fn is_keyboard(file: &File) -> bool {
    let mut keys = [0u8; KEY_CNT / 8];
    read_bits(file, 0x20 + EV_KEY, &mut keys) && KEYBOARD_KEYS.iter().all(|&k| has(&keys, k))
}

fn keyboards() -> Vec<File> {
    let Ok(dir) = std::fs::read_dir("/dev/input") else {
        return Vec::new();
    };
    dir.flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("event"))
        })
        .filter_map(|e| File::open(e.path()).ok())
        .filter(is_keyboard)
        .collect()
}

/// Whether a keyboard holds the evdev key `code`.
pub(super) fn pressed(code: u16) -> bool {
    let mut k = KEYBOARDS.lock().unwrap_or_else(|e| e.into_inner());
    if k.scanned.is_none_or(|t| t.elapsed() >= RESCAN) {
        k.files = keyboards();
        k.scanned = Some(Instant::now());
    }
    let mut held = [0u8; KEY_CNT / 8];
    k.files
        .iter()
        .any(|f| read_bits(f, 0x18, &mut held) && has(&held, usize::from(code)))
}
