//! The real service binary with real uinput devices: it makes and keeps the virtual devices,
//! lends their descriptors, and what the client writes reaches the device. Needs root and
//! /dev/uinput, so it is ignored by default (it also writes the service's state and runtime
//! udev rule, so run it in a throwaway container):
//!   tools\linux\check.ps1 --devices sh -c "cargo test -p system-gobloon --test devices -- --ignored"
#![cfg(target_os = "linux")]

use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use gob_system::Client;
use gob_system::protocol::{ApplyVirtual, ChangeKind, DeviceKind, Owner};
use gob_uinput::{Emitter, codes};

/// The service on its own socket, stopped when dropped.
struct Running {
    child: Child,
    socket: PathBuf,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn start() -> Running {
    let socket = std::env::temp_dir().join(format!("system-gobloon-{}.sock", std::process::id()));
    let child = Command::new(env!("CARGO_BIN_EXE_SystemGobloon"))
        .arg("--socket")
        .arg(&socket)
        .spawn()
        .expect("start SystemGobloon");
    let running = Running { child, socket };
    wait("the socket", || Client::connect_to(&running.socket).is_ok());
    running
}

fn wait(what: &str, mut done: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < end, "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The input device at this `phys`: its sysfs folder.
fn device_at(phys: &str) -> Option<PathBuf> {
    std::fs::read_dir("/sys/class/input")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            std::fs::read_to_string(p.join("phys")).is_ok_and(|t| t.trim_end() == phys)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("input"))
        })
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file))
        .unwrap_or_default()
        .trim_end()
        .to_string()
}

/// The event node of the input device in `dir`.
fn event_node(dir: &Path) -> PathBuf {
    let event = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .find_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.starts_with("event").then_some(name)
        })
        .expect("event node");
    Path::new("/dev/input").join(event)
}

#[test]
#[ignore = "needs root and /dev/uinput"]
fn service_keeps_and_lends_real_devices() {
    let s = start();
    let mut client = Client::connect_to(&s.socket).unwrap();
    client.hello("test").unwrap();

    let mut config = gob_virtual::DeviceConfig::new(7, &gob_virtual::capabilities());
    config.buttons = 8;
    config.name = Some("Gobloon Test Stick".into());
    config.product = Some(0xBE42);
    let set = ApplyVirtual {
        kind: ChangeKind::SetJoystick as u8,
        reserved: [0; 3],
        joystick: gob_virtual::uinput::joystick_record(&config, Owner::Free),
    };
    client.apply_virtual(set).unwrap();
    let phys = "joystick-gobloon/joystick-7";
    wait("joystick 7", || device_at(phys).is_some());
    let dir = device_at(phys).unwrap();
    assert_eq!(read(&dir, "name"), "Gobloon Test Stick");
    assert_eq!(read(&dir, "id/vendor"), "1234");
    assert_eq!(read(&dir, "id/product"), "be42");

    let mut node = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(event_node(&dir))
        .unwrap();
    let joystick = Emitter::new(client.acquire(DeviceKind::Joystick, 7).unwrap());
    joystick
        .emit(&[(codes::EV_KEY, codes::BTN_JOYSTICK, 1)])
        .unwrap();
    let mut seen = Vec::new();
    wait("the button", || {
        let mut buf = [0u8; 24 * 16];
        if let Ok(n) = node.read(&mut buf) {
            seen.extend_from_slice(&buf[..n]);
        }
        let (events, _) = seen.as_chunks::<{ size_of::<libc::input_event>() }>();
        events.iter().any(|c| {
            // SAFETY: the kernel wrote whole `input_event`s.
            let e: libc::input_event = unsafe { std::ptr::read_unaligned(c.as_ptr().cast()) };
            (e.type_, e.code, e.value) == (codes::EV_KEY, codes::BTN_JOYSTICK, 1)
        })
    });

    // The keyboard of keyboard output is made on first use and kept.
    client.acquire(DeviceKind::Keyboard, 0).unwrap();
    wait("the keyboard", || {
        device_at("joystick-gobloon/keyboard").is_some()
    });

    // The client goes away: the devices stay.
    drop(client);
    std::thread::sleep(Duration::from_millis(200));
    assert!(device_at(phys).is_some(), "kept without an owner");
    let mut other = Client::connect_to(&s.socket).unwrap();
    other.acquire(DeviceKind::Joystick, 7).unwrap();
}
