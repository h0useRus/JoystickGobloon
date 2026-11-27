//! Buttons from the raw HID device (`/dev/hidrawN`), for controllers with more buttons than the
//! kernel's HID driver passes on to evdev: it gives a joystick 16 `BTN_JOYSTICK` codes and 64
//! `BTN_TRIGGER_HAPPY` codes and drops the rest, so button 81 and up never reach evdev.
//!
//! [`buttons`] reads where each button sits in the input reports from the report descriptor
//! (`/sys/class/hidraw/hidrawN/device/report_descriptor`, readable by everyone); [`Raw`] reads
//! the reports. Buttons are numbered by their HID usage like on Windows, so profiles count them
//! the same way: usage 1 is button 1.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::path::Path;

/// HID usage page of buttons.
const PAGE_BUTTON: u32 = 0x09;
/// `HIDIOCGINPUT(len)`: `_IOC(_IOC_READ | _IOC_WRITE, 'H', 0x0A, len)`, reads an input report
/// from the device on request (Linux 5.11).
const fn hidiocginput(len: usize) -> u32 {
    (3 << 30) | ((len as u32) << 16) | ((b'H' as u32) << 8) | 0x0A
}
/// Largest input report read in one go (with its report id).
const MAX_REPORT: usize = 256;

/// One button in the input reports: bit `bit` of the report with id `report` (0 when the
/// device has no report ids), counted after the id byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Button {
    pub report: u8,
    pub bit: u32,
    /// Button number, 1-based (its HID usage).
    pub usage: u16,
}

/// Global item state, saved and restored by Push and Pop.
#[derive(Debug, Clone, Copy, Default)]
struct Globals {
    usage_page: u32,
    report_size: u32,
    report_count: u32,
    report_id: u8,
}

/// What [`buttons`] finds in a report descriptor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    pub buttons: Vec<Button>,
    /// Input reports start with a report id byte.
    pub has_ids: bool,
    /// The input reports holding buttons: (report id, length in bytes without the id).
    pub reports: Vec<(u8, usize)>,
}

/// The buttons a report descriptor declares in its input reports (variable, non-constant
/// fields on the Button page, one bit each).
pub fn buttons(desc: &[u8]) -> Layout {
    let mut g = Globals::default();
    let mut stack: Vec<Globals> = Vec::new();
    let mut usages: Vec<u32> = Vec::new();
    let (mut usage_min, mut usage_max): (Option<u32>, Option<u32>) = (None, None);
    // Next free bit of each input report, by report id.
    let mut offsets = [0u32; 256];
    let mut has_ids = false;
    let mut found = Vec::new();
    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];
        // Long item: 0xFE, data size, long tag, data.
        if prefix == 0xFE {
            let size = desc.get(i + 1).copied().unwrap_or(0) as usize;
            i += 3 + size;
            continue;
        }
        let size = match prefix & 0x03 {
            3 => 4,
            n => n as usize,
        };
        let Some(bytes) = desc.get(i + 1..i + 1 + size) else {
            break;
        };
        let data = bytes
            .iter()
            .rev()
            .fold(0u32, |v, &b| (v << 8) | u32::from(b));
        i += 1 + size;
        // A usage with 4 bytes carries its page in the upper half.
        let full_usage = |u: u32| {
            if size == 4 {
                u
            } else {
                (g.usage_page << 16) | u
            }
        };
        match (prefix >> 2) & 0x03 {
            // Main items: Input is tag 8; every main item clears the local items.
            0 => {
                if prefix >> 4 == 0x8 {
                    let constant = data & 0x01 != 0;
                    let variable = data & 0x02 != 0;
                    let offset = &mut offsets[usize::from(g.report_id)];
                    if !constant && variable && g.report_size == 1 {
                        for n in 0..g.report_count {
                            let usage = match (usage_min, usage_max) {
                                _ if (n as usize) < usages.len() => Some(usages[n as usize]),
                                (Some(min), Some(max)) if min + n <= max => Some(min + n),
                                // Fewer usages than fields: the rest repeat the last one.
                                _ => usages.last().copied(),
                            };
                            if let Some(u) = usage
                                && u >> 16 == PAGE_BUTTON
                                && u & 0xFFFF != 0
                            {
                                found.push(Button {
                                    report: g.report_id,
                                    bit: *offset + n,
                                    usage: (u & 0xFFFF) as u16,
                                });
                            }
                        }
                    }
                    *offset += g.report_size * g.report_count;
                }
                usages.clear();
                usage_min = None;
                usage_max = None;
            }
            // Global items.
            1 => match prefix >> 4 {
                0x0 => g.usage_page = data,
                0x7 => g.report_size = data,
                0x8 => {
                    g.report_id = data as u8;
                    has_ids = true;
                }
                0x9 => g.report_count = data,
                0xA => stack.push(g),
                0xB => g = stack.pop().unwrap_or_default(),
                _ => {}
            },
            // Local items: Usage, Usage Minimum, Usage Maximum.
            2 => match prefix >> 4 {
                0x0 => usages.push(full_usage(data)),
                0x1 => usage_min = Some(full_usage(data)),
                0x2 => usage_max = Some(full_usage(data)),
                _ => {}
            },
            _ => {}
        }
    }
    let mut reports: Vec<(u8, usize)> = Vec::new();
    for b in &found {
        if !reports.iter().any(|(id, _)| *id == b.report) {
            let bits = offsets[usize::from(b.report)];
            reports.push((b.report, bits.div_ceil(8) as usize));
        }
    }
    Layout {
        buttons: found,
        has_ids,
        reports,
    }
}

/// The report descriptor of hidraw node `hidraw` (`/dev/hidrawN`).
pub fn descriptor(hidraw: &Path) -> Option<Vec<u8>> {
    let name = hidraw.file_name()?;
    std::fs::read(
        Path::new("/sys/class/hidraw")
            .join(name)
            .join("device/report_descriptor"),
    )
    .ok()
}

/// Buttons read from an open hidraw node.
pub struct Raw {
    file: File,
    layout: Layout,
}

impl Raw {
    /// Opens `path` for non-blocking reads of the buttons in `layout`.
    pub fn open(path: &Path, layout: Layout) -> io::Result<Self> {
        let file = super::open_node(path)?;
        Ok(Self { file, layout })
    }

    /// Asks the device for its current input reports, so buttons held when it is opened (a
    /// toggle switch) count from the start; calls `set` like [`Self::read`]. Devices or kernels
    /// that cannot answer leave the buttons released until their first report.
    pub fn read_current(&self, set: &mut dyn FnMut(u16, bool)) {
        let mut buf = [0u8; MAX_REPORT];
        for &(id, len) in &self.layout.reports {
            let len = (len + 1).min(MAX_REPORT);
            buf[0] = id;
            // SAFETY: `buf` is valid for `len` bytes; the request writes at most `len`.
            let n = unsafe { libc::ioctl(self.fd(), hidiocginput(len) as _, buf.as_mut_ptr()) };
            if n > 0 {
                // The answer starts with the report id byte, also without report ids (0).
                let data = &buf[1..(n as usize).min(len)];
                apply_report(&self.layout.buttons, id, data, set);
            }
        }
    }

    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Reads every waiting report and calls `set(button, pressed)` for each button in it
    /// (button 1-based). `Err` when the device is gone.
    pub fn read(&self, set: &mut dyn FnMut(u16, bool)) -> io::Result<()> {
        let mut buf = [0u8; MAX_REPORT];
        loop {
            // SAFETY: `buf` is valid for `buf.len()` bytes.
            let n = unsafe { libc::read(self.fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n < 0 {
                let err = io::Error::last_os_error();
                return match err.kind() {
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted => Ok(()),
                    _ => Err(err),
                };
            }
            if n == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            self.apply(&buf[..n as usize], set);
        }
    }

    /// One input report (with its id byte when the device uses report ids).
    fn apply(&self, report: &[u8], set: &mut dyn FnMut(u16, bool)) {
        apply(&self.layout, report, set);
    }
}

/// An input report as hidraw reads it: with its id byte when the device uses report ids.
fn apply(layout: &Layout, report: &[u8], set: &mut dyn FnMut(u16, bool)) {
    match (layout.has_ids, report.split_first()) {
        (true, Some((&id, data))) => apply_report(&layout.buttons, id, data, set),
        (true, None) => {}
        (false, _) => apply_report(&layout.buttons, 0, report, set),
    }
}

/// The data of input report `id` (after the id byte).
fn apply_report(buttons: &[Button], id: u8, data: &[u8], set: &mut dyn FnMut(u16, bool)) {
    for b in buttons.iter().filter(|b| b.report == id) {
        let byte = (b.bit / 8) as usize;
        if let Some(&v) = data.get(byte) {
            set(b.usage, v & (1 << (b.bit % 8)) != 0);
        }
    }
}

#[cfg(test)]
mod tests;
