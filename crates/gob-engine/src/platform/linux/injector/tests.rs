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
    let mut keyboard = open_node(output::KEYBOARD_NAME);
    let mut mouse = open_node(output::MOUSE_NAME);

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
