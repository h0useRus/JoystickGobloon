//! Devices on the real /dev/uinput, checked as the kernel reports them. Needs write access to
//! /dev/uinput (root or the udev rule), so ignored by default:
//!   tools\linux\check.ps1 --devices cargo test -p gob-uinput --test uinput -- --ignored
#![cfg(target_os = "linux")]

use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;

use gob_uinput::{VirtualDevice, codes, gamepad};

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file))
        .unwrap_or_else(|e| panic!("{file}: {e}"))
        .trim()
        .to_string()
}

/// Whether bit `n` is set in a sysfs capability bitmap ("1f 0 ..." = 64-bit words, highest first).
fn has_bit(bitmap: &str, n: u16) -> bool {
    let words: Vec<u64> = bitmap
        .split_whitespace()
        .rev()
        .map(|w| u64::from_str_radix(w, 16).unwrap())
        .collect();
    let (word, bit) = (usize::from(n) / 64, n % 64);
    words.get(word).is_some_and(|w| w >> bit & 1 == 1)
}

/// `EVIOCGABS(code)`: min and max of an axis.
fn abs_range(node: &File, code: u16) -> (i32, i32) {
    let size = size_of::<libc::input_absinfo>() as u32;
    let request = (2u32 << 30) | (size << 16) | (u32::from(b'E') << 8) | (0x40 + u32::from(code));
    // SAFETY: zeroes are a valid input_absinfo.
    let mut info: libc::input_absinfo = unsafe { std::mem::zeroed() };
    // SAFETY: the request writes one input_absinfo into `info`.
    let r = unsafe { libc::ioctl(node.as_raw_fd(), request as _, &mut info) };
    assert!(
        r >= 0,
        "EVIOCGABS {code}: {}",
        std::io::Error::last_os_error()
    );
    (info.minimum, info.maximum)
}

#[test]
#[ignore = "needs /dev/uinput"]
fn gamepad_looks_like_a_wired_xbox_360_pad() {
    let name = "Joystick Gobloon Test Gamepad";
    let phys = "joystick-gobloon/test-gamepad";
    let dev = VirtualDevice::create(&gamepad::spec(name, phys)).expect("create the gamepad");
    let sys = dev.sys_path().unwrap();
    assert_eq!(read(&sys, "name"), name);
    assert_eq!(read(&sys, "phys"), phys);
    assert_eq!(read(&sys, "id/bustype"), format!("{:04x}", codes::BUS_USB));
    assert_eq!(read(&sys, "id/vendor"), "045e");
    assert_eq!(read(&sys, "id/product"), "028e");
    assert_eq!(read(&sys, "id/version"), "0114");

    let keys = read(&sys, "capabilities/key");
    for code in gamepad::BUTTONS {
        assert!(has_bit(&keys, code), "button {code:#x}");
    }
    assert!(!has_bit(&keys, codes::BTN_JOYSTICK), "no joystick buttons");

    // The node appears a moment after the device.
    let node = (0..50)
        .find_map(|_| {
            let node = dev.event_node().ok().and_then(|n| File::open(n).ok());
            if node.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            node
        })
        .expect("open the event node");
    for axis in gamepad::AXES {
        assert_eq!(
            abs_range(&node, axis.code),
            (axis.min, axis.max),
            "axis {:#x}",
            axis.code
        );
    }
}
