//! Buttons from the raw HID device (`/dev/hidrawN`), for controllers with more buttons than the
//! kernel's HID driver passes on to evdev: it gives a joystick 16 `BTN_JOYSTICK` codes and 64
//! `BTN_TRIGGER_HAPPY` codes and drops the rest, so button 81 and up never reach evdev.
//!
//! [`buttons`] reads where each button sits in the input reports from the report descriptor
//! (`/sys/class/hidraw/hidrawN/device/report_descriptor`, readable by everyone); [`Raw`] reads
//! the reports. Buttons are numbered by their HID usage like on Windows, so profiles count them
//! the same way: usage 1 is button 1.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
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
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)?;
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
mod tests {
    use super::*;

    /// A joystick with 128 buttons in report 1 after two 16-bit axes, and a constant pad.
    const STICK: &[u8] = &[
        0x05, 0x01, // Usage Page (Generic Desktop)
        0x09, 0x04, // Usage (Joystick)
        0xA1, 0x01, // Collection (Application)
        0x85, 0x01, //   Report ID (1)
        0x09, 0x30, //   Usage (X)
        0x09, 0x31, //   Usage (Y)
        0x15, 0x00, //   Logical Minimum (0)
        0x26, 0xFF, 0x7F, // Logical Maximum (32767)
        0x75, 0x10, //   Report Size (16)
        0x95, 0x02, //   Report Count (2)
        0x81, 0x02, //   Input (Data, Variable, Absolute)
        0x05, 0x09, //   Usage Page (Button)
        0x19, 0x01, //   Usage Minimum (1)
        0x29, 0x80, //   Usage Maximum (128)
        0x15, 0x00, //   Logical Minimum (0)
        0x25, 0x01, //   Logical Maximum (1)
        0x75, 0x01, //   Report Size (1)
        0x96, 0x80, 0x00, // Report Count (128)
        0x81, 0x02, //   Input (Data, Variable, Absolute)
        0x75, 0x08, //   Report Size (8)
        0x95, 0x01, //   Report Count (1)
        0x81, 0x03, //   Input (Constant)
        0xC0, // End Collection
    ];

    #[test]
    fn buttons_follow_the_axes() {
        let layout = buttons(STICK);
        assert!(layout.has_ids);
        // Report 1: 4 bytes of axes, 16 of buttons, 1 constant.
        assert_eq!(layout.reports, vec![(1, 21)]);
        let buttons = layout.buttons;
        assert_eq!(buttons.len(), 128);
        assert_eq!(
            buttons[0],
            Button {
                report: 1,
                bit: 32,
                usage: 1
            }
        );
        assert_eq!(buttons[127].usage, 128);
        assert_eq!(buttons[127].bit, 32 + 127);
    }

    #[test]
    fn reports_set_buttons_by_usage() {
        let layout = buttons(STICK);
        // Id 1, axes (4 bytes), buttons: 1 and 81 and 128 pressed.
        let mut report = vec![1u8, 0, 0, 0, 0];
        let mut bits = [0u8; 16];
        for b in [1u32, 81, 128] {
            bits[((b - 1) / 8) as usize] |= 1 << ((b - 1) % 8);
        }
        report.extend_from_slice(&bits);
        let mut pressed = Vec::new();
        apply(&layout, &report, &mut |b, on| {
            if on {
                pressed.push(b)
            }
        });
        assert_eq!(pressed, vec![1, 81, 128]);
        // Another report id leaves them alone.
        let mut calls = 0;
        apply(&layout, &[2, 0xFF, 0xFF], &mut |_, _| calls += 1);
        assert_eq!(calls, 0);
    }

    #[test]
    fn without_report_ids_and_with_a_usage_list() {
        let desc = [
            0x05, 0x09, // Usage Page (Button)
            0x09, 0x02, // Usage (2)
            0x09, 0x01, // Usage (1)
            0x75, 0x01, // Report Size (1)
            0x95, 0x02, // Report Count (2)
            0x81, 0x02, // Input (Data, Variable)
            0x95, 0x06, // Report Count (6)
            0x81, 0x01, // Input (Constant)
        ];
        let layout = buttons(&desc);
        assert!(!layout.has_ids);
        assert_eq!(layout.reports, vec![(0, 1)]);
        let buttons = layout.buttons;
        assert_eq!(buttons.len(), 2);
        assert_eq!((buttons[0].usage, buttons[0].bit), (2, 0));
        assert_eq!((buttons[1].usage, buttons[1].bit), (1, 1));
    }

    #[test]
    fn push_pop_and_other_pages_and_arrays_are_skipped() {
        let desc = [
            0x05, 0x09, // Usage Page (Button)
            0xA4, // Push
            0x05, 0x07, // Usage Page (Keyboard)
            0x19, 0x04, 0x29, 0x05, // Usage 4..5
            0x75, 0x01, 0x95, 0x02, 0x81, 0x02, // two keyboard bits
            0xB4, // Pop: Button page again
            0x19, 0x01, 0x29, 0x08, // Usage 1..8
            0x75, 0x08, 0x95, 0x01, 0x81, 0x00, // an array (button index), skipped
            0x19, 0x01, 0x29, 0x02, // Usage 1..2
            0x75, 0x01, 0x95, 0x02, 0x81, 0x02, // two button bits
        ];
        assert_eq!(
            buttons(&desc).buttons,
            vec![
                Button {
                    report: 0,
                    bit: 10,
                    usage: 1
                },
                Button {
                    report: 0,
                    bit: 11,
                    usage: 2
                },
            ]
        );
    }

    #[test]
    fn truncated_descriptors_do_not_panic() {
        for n in 0..STICK.len() {
            let _ = buttons(&STICK[..n]);
        }
        let _ = buttons(&[0xFE, 0x10]);
    }
}
