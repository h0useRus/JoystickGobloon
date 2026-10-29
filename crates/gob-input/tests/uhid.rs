//! A stick with 128 buttons as a real HID device on uhid: the kernel's HID driver gives evdev
//! only the first 80, gob-input reads all of them from the device's hidraw node. Needs
//! /dev/uhid (the uhid module; WSL2 kernels lack it) and the hidraw node (root), so it is
//! ignored by default:
//!   tools\linux\check.ps1 --devices cargo test -p gob-input --test uhid -- --ignored
#![cfg(target_os = "linux")]

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use gob_input::{InputEvent, InputService};

const UHID_DESTROY: u32 = 1;
const UHID_CREATE2: u32 = 11;
const UHID_INPUT2: u32 = 12;
const BUS_USB: u16 = 3;
/// `struct uhid_event` (packed): type, then the largest request, `uhid_create2_req`.
const EVENT_SIZE: usize = 4 + 128 + 64 + 64 + 2 + 2 + 4 * 4 + 4096;
const NAME: &str = "Test HID stick 128";
const BUTTONS: usize = 128;

/// Joystick, report 1: X and Y (16 bits each), 128 buttons, one constant byte.
const DESCRIPTOR: &[u8] = &[
    0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, // Generic Desktop, Joystick, Collection (Application)
    0x85, 0x01, // Report ID (1)
    0x09, 0x30, 0x09, 0x31, // Usage X, Y
    0x15, 0x00, 0x26, 0xFF, 0x7F, // Logical 0..32767
    0x75, 0x10, 0x95, 0x02, 0x81, 0x02, // 2 x 16 bits, Input (Data, Variable)
    0x05, 0x09, 0x19, 0x01, 0x29, 0x80, // Button page, usages 1..128
    0x15, 0x00, 0x25, 0x01, // Logical 0..1
    0x75, 0x01, 0x96, 0x80, 0x00, 0x81, 0x02, // 128 x 1 bit, Input (Data, Variable)
    0x75, 0x08, 0x95, 0x01, 0x81, 0x03, // 1 x 8 bits, Input (Constant)
    0xC0,
];

/// A HID device made through /dev/uhid; destroyed on drop.
struct Uhid {
    file: File,
}

impl Uhid {
    fn create() -> Self {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uhid")
            .expect("open /dev/uhid");
        let mut ev = vec![0u8; EVENT_SIZE];
        ev[0..4].copy_from_slice(&UHID_CREATE2.to_le_bytes());
        let req = &mut ev[4..];
        req[..NAME.len()].copy_from_slice(NAME.as_bytes());
        let at = 128 + 64 + 64;
        req[at..at + 2].copy_from_slice(&(DESCRIPTOR.len() as u16).to_le_bytes());
        req[at + 2..at + 4].copy_from_slice(&BUS_USB.to_le_bytes());
        req[at + 4..at + 8].copy_from_slice(&0x1209u32.to_le_bytes()); // vendor
        req[at + 8..at + 12].copy_from_slice(&0x0128u32.to_le_bytes()); // product
        let rd = at + 4 * 4 + 4;
        req[rd..rd + DESCRIPTOR.len()].copy_from_slice(DESCRIPTOR);
        file.write_all(&ev).expect("UHID_CREATE2");
        Self { file }
    }

    /// Sends report 1 with `pressed` buttons (1-based) down and the axes centered.
    fn press(&mut self, pressed: &[u32]) {
        let mut report = vec![1u8, 0xFF, 0x3F, 0xFF, 0x3F];
        let mut bits = [0u8; BUTTONS / 8];
        for &b in pressed {
            bits[((b - 1) / 8) as usize] |= 1 << ((b - 1) % 8);
        }
        report.extend_from_slice(&bits);
        report.push(0);
        let mut ev = vec![0u8; EVENT_SIZE];
        ev[0..4].copy_from_slice(&UHID_INPUT2.to_le_bytes());
        ev[4..6].copy_from_slice(&(report.len() as u16).to_le_bytes());
        ev[6..6 + report.len()].copy_from_slice(&report);
        self.file.write_all(&ev).expect("UHID_INPUT2");
    }
}

impl Drop for Uhid {
    fn drop(&mut self) {
        let mut ev = vec![0u8; EVENT_SIZE];
        ev[0..4].copy_from_slice(&UHID_DESTROY.to_le_bytes());
        let _ = self.file.write_all(&ev);
    }
}

fn wait_for(events: &Receiver<InputEvent>, want: impl Fn(&InputEvent) -> bool) {
    let end = Instant::now() + Duration::from_secs(2);
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        if let Ok(e) = events.recv_timeout(left)
            && want(&e)
        {
            return;
        }
    }
    panic!("event not seen in time");
}

#[test]
#[ignore = "needs /dev/uhid"]
fn buttons_past_80_come_from_hidraw() {
    let mut stick = Uhid::create();
    let input = InputService::start(|| {});
    input.set_event_mode(true);
    let events = input.subscribe();

    let end = Instant::now() + Duration::from_secs(5);
    let state = loop {
        if let Some(d) = input.devices().iter().find(|d| d.info.name == NAME) {
            break d.clone();
        }
        assert!(Instant::now() < end, "HID stick not found");
        std::thread::sleep(Duration::from_millis(50));
    };
    println!("{:?}", state.info);
    assert_eq!(state.info.buttons, BUTTONS, "all buttons, not evdev's 80");
    assert_eq!(state.info.unread_buttons, 0);
    assert!(state.info.hid_path.starts_with("/dev/hidraw"));
    let guid = state.info.guid.clone();
    // Let the event mode reopen settle before sending.
    std::thread::sleep(Duration::from_millis(300));

    for (button, pressed) in [(3, true), (100, true), (128, true), (100, false)] {
        let down: Vec<u32> = match (button, pressed) {
            (3, _) => vec![3],
            (100, true) => vec![3, 100],
            (128, _) => vec![3, 100, 128],
            _ => vec![3, 128],
        };
        stick.press(&down);
        wait_for(&events, |e| {
            matches!(e, InputEvent::Button { device, id, pressed: p }
                if *device == guid && *id == button && *p == pressed)
        });
    }
    let d = input
        .devices()
        .into_iter()
        .find(|d| d.info.guid == guid)
        .unwrap();
    let down: Vec<usize> = (0..BUTTONS)
        .filter(|&i| d.buttons[i])
        .map(|i| i + 1)
        .collect();
    assert_eq!(down, vec![3, 128]);
}
