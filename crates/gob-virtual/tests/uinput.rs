//! The Linux backend end to end: acquire a device, drive it, read what the kernel reports.
//! Needs write access to /dev/uinput (root or a udev rule), so it is ignored by default:
//!   tools\linux\check.ps1 --devices cargo test -p gob-virtual --test uinput -- --ignored
#![cfg(target_os = "linux")]

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gob_virtual::Status;

const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const ABS_THROTTLE: u16 = 6;
const ABS_HAT1X: u16 = 0x12;
const ABS_HAT1Y: u16 = 0x13;

/// The event node of the input device with this name.
fn event_node(name: &str) -> Option<PathBuf> {
    std::fs::read_dir("/sys/class/input")
        .ok()?
        .flatten()
        .find_map(|e| {
            let file = e.file_name();
            let file = file.to_str()?;
            let found = file.starts_with("event")
                && std::fs::read_to_string(e.path().join("device/name"))
                    .ok()?
                    .trim_end()
                    == name;
            found.then(|| PathBuf::from("/dev/input").join(file))
        })
}

fn sys_id(node: &Path, field: &str) -> String {
    let name = node.file_name().unwrap();
    let path = PathBuf::from("/sys/class/input")
        .join(name)
        .join("device/id")
        .join(field);
    std::fs::read_to_string(path).unwrap().trim().to_string()
}

/// Every (type, code, value) waiting on the node, without SYN.
fn read_events(node: &mut File) -> Vec<(u16, u16, i32)> {
    let size = size_of::<libc::input_event>();
    let mut buf = vec![0u8; size * 64];
    let mut out = Vec::new();
    let end = Instant::now() + Duration::from_millis(300);
    while Instant::now() < end {
        match node.read(&mut buf) {
            Ok(n) => {
                for chunk in buf[..n].chunks_exact(size) {
                    // SAFETY: the kernel wrote whole `input_event`s.
                    let e: libc::input_event =
                        unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
                    if e.type_ != 0 {
                        out.push((e.type_, e.code, e.value));
                    }
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    out
}

#[test]
#[ignore = "needs /dev/uinput"]
fn acquire_drive_release() {
    let dir = std::env::temp_dir().join(format!("gob-virtual-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("virtual.gob"),
        "[[device]]\nid = 1\naxes = [1, 2, 7]\nbuttons = 95\nhats = 2\n\n\
         [[device]]\nid = 2\naxes = [1]\nbuttons = 8\nhats = 0\n",
    )
    .unwrap();
    gob_virtual::set_data_dir(&dir);

    let backend = gob_virtual::load().expect("uinput backend");
    let devices = backend.devices();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].axes, vec![1, 2, 7]);
    assert_eq!((devices[0].buttons, devices[0].cont_hats), (95, 2));
    assert_eq!(backend.status(1), Status::Free);
    assert_eq!(backend.status(3), Status::Missing);
    assert!(backend.acquire(3).is_err());

    let device = backend.acquire(1).expect("acquire 1");
    assert_eq!(backend.status(1), Status::Own);
    assert!(backend.acquire(1).is_err(), "already held");

    let name = "Joystick Gobloon Joystick 1";
    let path = event_node(name).expect("event node of device 1");
    assert_eq!(sys_id(&path, "vendor"), "1234");
    assert_eq!(sys_id(&path, "product"), "bead");
    let mut node = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
        .unwrap();
    read_events(&mut node);

    assert!(device.set_button(95, true));
    assert!(!device.set_button(96, true), "beyond the layout");
    assert!(device.set_hat(2, Some(13500)));
    assert!(device.set_axis(7, 1.0));
    assert!(!device.set_axis(3, 1.0), "Z is not in the layout");
    let events = read_events(&mut node);
    assert_eq!(
        events,
        vec![
            (EV_KEY, 0x10f, 1),
            (EV_ABS, ABS_HAT1X, 1),
            (EV_ABS, ABS_HAT1Y, 1),
            (EV_ABS, ABS_THROTTLE, 32767),
        ]
    );
    assert!(device.reset());
    let events = read_events(&mut node);
    assert!(events.contains(&(EV_KEY, 0x10f, 0)), "{events:?}");
    assert!(events.contains(&(EV_ABS, ABS_THROTTLE, 0)), "{events:?}");

    drop(device);
    let end = Instant::now() + Duration::from_secs(2);
    while event_node(name).is_some() {
        assert!(Instant::now() < end, "device still there after release");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(backend.status(1), Status::Free);

    // The configuration page's changes land in the file.
    let mut c = backend.read_config().unwrap().remove(1);
    c.buttons = 12;
    backend.apply(&gob_virtual::Change::Set(c)).unwrap();
    backend.apply(&gob_virtual::Change::Delete(1)).unwrap();
    let configs = backend.read_config().unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!((configs[0].id, configs[0].buttons), (2, 12));
    let _ = std::fs::remove_dir_all(&dir);
}
