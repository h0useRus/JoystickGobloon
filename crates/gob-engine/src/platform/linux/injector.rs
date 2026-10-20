//! Keyboard and mouse output through uinput: a virtual keyboard and a virtual mouse that the
//! desktop (X11 or Wayland) and games read like real ones. Their names start with
//! "Joystick Gobloon", so gob-input does not read our own keys back as input.

use std::sync::Mutex;

use gob_core::MouseButton;
use gob_uinput::{Spec, VirtualDevice, codes, keys};

const KEYBOARD_NAME: &str = "Joystick Gobloon Keyboard";
const MOUSE_NAME: &str = "Joystick Gobloon Mouse";
const MOUSE_BUTTONS: [u16; 5] = [
    codes::BTN_LEFT,
    codes::BTN_RIGHT,
    codes::BTN_MIDDLE,
    codes::BTN_SIDE,
    codes::BTN_EXTRA,
];

pub(crate) struct Injector;

/// The devices, made on first use and kept for the rest of the program.
#[derive(Default)]
struct Devices {
    keyboard: Option<VirtualDevice>,
    mouse: Option<VirtualDevice>,
    /// Creating them failed (e.g. no access to /dev/uinput): logged, and not tried again until
    /// the next profile start.
    failed: bool,
}

static DEVICES: Mutex<Option<Devices>> = Mutex::new(None);

impl Devices {
    fn create(&mut self) {
        if self.failed || (self.keyboard.is_some() && self.mouse.is_some()) {
            return;
        }
        let keys: Vec<u16> = keys::all_codes().collect();
        let keyboard = Spec {
            name: KEYBOARD_NAME,
            bus: codes::BUS_VIRTUAL,
            keys: &keys,
            ..Spec::default()
        };
        let mouse = Spec {
            name: MOUSE_NAME,
            bus: codes::BUS_VIRTUAL,
            keys: &MOUSE_BUTTONS,
            rel: &[
                codes::REL_X,
                codes::REL_Y,
                codes::REL_WHEEL,
                codes::REL_HWHEEL,
            ],
            ..Spec::default()
        };
        let created =
            VirtualDevice::create(&keyboard).and_then(|k| Ok((k, VirtualDevice::create(&mouse)?)));
        match created {
            Ok((k, m)) => (self.keyboard, self.mouse) = (Some(k), Some(m)),
            Err(e) => {
                self.failed = true;
                log::warn!("keyboard and mouse output is off: /dev/uinput: {e}");
            }
        }
    }
}

fn devices<R>(f: impl FnOnce(&mut Devices) -> R) -> R {
    let mut guard = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
    let devices = guard.get_or_insert_default();
    devices.create();
    f(devices)
}

/// Sends one report to the mouse (`mouse`) or the keyboard, making them if needed.
fn emit(mouse: bool, events: &[(u16, u16, i32)]) {
    devices(|d| {
        let device = if mouse { &d.mouse } else { &d.keyboard };
        if let Some(device) = device
            && let Err(e) = device.emit(events)
        {
            log::warn!("keyboard/mouse output: {e}");
        }
    });
}

impl crate::platform::Injector for Injector {
    /// Makes the devices before the first key: the desktop needs a moment to open a new
    /// device, and events sent before that are lost.
    fn prepare(&self) {
        let mut guard = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
        let devices = guard.get_or_insert_default();
        devices.failed = false;
        devices.create();
    }

    fn mouse_move(&self, dx: i32, dy: i32) {
        let events: Vec<_> = [(codes::REL_X, dx), (codes::REL_Y, dy)]
            .into_iter()
            .filter(|&(_, v)| v != 0)
            .map(|(code, v)| (codes::EV_REL, code, v))
            .collect();
        if !events.is_empty() {
            emit(true, &events);
        }
    }

    fn mouse_button(&self, button: MouseButton, pressed: bool) {
        let code = match button {
            MouseButton::Left => codes::BTN_LEFT,
            MouseButton::Right => codes::BTN_RIGHT,
            MouseButton::Middle => codes::BTN_MIDDLE,
            MouseButton::Back => codes::BTN_SIDE,
            MouseButton::Forward => codes::BTN_EXTRA,
            MouseButton::WheelUp | MouseButton::WheelDown => return,
        };
        emit(true, &[(codes::EV_KEY, code, i32::from(pressed))]);
    }

    fn mouse_wheel(&self, notches: i32) {
        if notches != 0 {
            emit(true, &[(codes::EV_REL, codes::REL_WHEEL, notches)]);
        }
    }

    fn key(&self, scan_code: u16, extended: bool, pressed: bool) {
        match keys::key_code(scan_code, extended) {
            Some(code) => emit(false, &[(codes::EV_KEY, code, i32::from(pressed))]),
            None => log::debug!("no Linux key for scan code {scan_code:#x} (E0: {extended})"),
        }
    }

    /// Whether any keyboard holds the key, our own virtual one included (like Windows'
    /// GetAsyncKeyState, which sees injected keys too).
    fn key_pressed(&self, scan_code: u16, extended: bool) -> bool {
        keys::key_code(scan_code, extended).is_some_and(super::keystate::pressed)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::platform::Injector as _;

    /// The event node of our device with this name, opened for non-blocking reads.
    fn open_node(name: &str) -> File {
        let dir = std::fs::read_dir("/sys/class/input").unwrap();
        let node = dir
            .flatten()
            .find_map(|e| {
                let file = e.file_name().into_string().ok()?;
                let found = file.starts_with("event")
                    && std::fs::read_to_string(e.path().join("device/name"))
                        .ok()?
                        .trim_end()
                        == name;
                found.then(|| PathBuf::from("/dev/input").join(file))
            })
            .unwrap_or_else(|| panic!("{name} not found"));
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(node)
            .unwrap()
    }

    /// Every (type, code, value) waiting, without SYN.
    fn read_events(node: &mut File) -> Vec<(u16, u16, i32)> {
        let size = size_of::<libc::input_event>();
        let mut buf = vec![0u8; size * 64];
        let mut out = Vec::new();
        let end = Instant::now() + Duration::from_millis(200);
        while Instant::now() < end {
            match node.read(&mut buf) {
                Ok(n) => {
                    for chunk in buf[..n].chunks_exact(size) {
                        // SAFETY: the kernel wrote whole `input_event`s.
                        let e: libc::input_event =
                            unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
                        if e.type_ != codes::EV_SYN {
                            out.push((e.type_, e.code, e.value));
                        }
                    }
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        out
    }

    /// Needs /dev/uinput:
    ///   tools\linux\check.ps1 --devices cargo test -p gob-engine injector -- --ignored
    #[test]
    #[ignore = "needs /dev/uinput"]
    fn keys_and_mouse_reach_the_devices() {
        let out = Injector;
        out.prepare();
        let mut keyboard = open_node(KEYBOARD_NAME);
        let mut mouse = open_node(MOUSE_NAME);

        out.key(0x1d, true, true); // right Ctrl
        assert!(
            out.key_pressed(0x1d, true),
            "held, as GetAsyncKeyState sees it"
        );
        assert!(!out.key_pressed(0x1d, false), "left Ctrl is not");
        out.key(0x1d, true, false);
        out.key(0x45, false, true); // Pause
        out.key(0x45, false, false);
        assert!(!out.key_pressed(0x1d, true));
        assert_eq!(
            read_events(&mut keyboard),
            vec![
                (codes::EV_KEY, 97, 1),
                (codes::EV_KEY, 97, 0),
                (codes::EV_KEY, 119, 1),
                (codes::EV_KEY, 119, 0),
            ]
        );

        out.mouse_move(5, 0);
        out.mouse_move(-3, 7);
        out.mouse_button(MouseButton::Back, true);
        out.mouse_button(MouseButton::Back, false);
        out.mouse_wheel(-2);
        assert_eq!(
            read_events(&mut mouse),
            vec![
                (codes::EV_REL, codes::REL_X, 5),
                (codes::EV_REL, codes::REL_X, -3),
                (codes::EV_REL, codes::REL_Y, 7),
                (codes::EV_KEY, codes::BTN_SIDE, 1),
                (codes::EV_KEY, codes::BTN_SIDE, 0),
                (codes::EV_REL, codes::REL_WHEEL, -2),
            ]
        );
    }
}
