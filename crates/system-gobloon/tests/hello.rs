//! The service and gob-system's client over a real socket, with a fake system (no root, no
//! devices needed).
#![cfg(target_os = "linux")]

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gob_input::Nodes;
use gob_system::protocol::{
    Code, DeviceKind, ErrorCode, Hidden, PROTOCOL, Record, SetHidden, SetHideEnabled, Text, caps,
    hidden_records,
};
use gob_system::{Client, Error};
use system_gobloon::hide::{Hide, System};
use system_gobloon::virtuals::{Factory, Held, Virtuals};
use system_gobloon::{Service, server};

/// One attached stick; records what the service did.
#[derive(Clone, Default)]
struct Fake {
    log: Arc<Mutex<Vec<String>>>,
}

impl System for Fake {
    fn controllers(&self) -> Vec<Nodes> {
        vec![Nodes {
            guid: "aaaa".into(),
            name: "Stick".into(),
            vendor: 0x044f,
            product: 0xb10a,
            usb_serial: Some("SN1".into()),
            paths: vec!["/dev/input/event4".into(), "/dev/hidraw3".into()],
        }]
    }

    fn trigger(&self) -> io::Result<()> {
        self.log.lock().unwrap().push("trigger".into());
        Ok(())
    }

    fn close(&self, path: &Path) -> io::Result<bool> {
        self.log
            .lock()
            .unwrap()
            .push(format!("close {}", path.display()));
        Ok(true)
    }

    /// Any readable file stands in for the node.
    fn open(&self, _path: &Path) -> io::Result<OwnedFd> {
        Ok(std::fs::File::open(std::env::current_exe()?)?.into())
    }
}

/// A kept device: a file stands in for its descriptor, events go to the log.
struct FakeDevice {
    name: String,
    file: std::fs::File,
    log: Arc<Mutex<Vec<String>>>,
}

impl Held for FakeDevice {
    fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        let line = format!("{} {} events", self.name, events.len());
        self.log.lock().unwrap().push(line);
        Ok(())
    }

    fn fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

impl Factory for Fake {
    fn joystick(&self, config: &gob_virtual::DeviceConfig) -> io::Result<Box<dyn Held>> {
        self.device(format!("joystick {}", config.id))
    }

    fn gamepad(&self, id: u32) -> io::Result<Box<dyn Held>> {
        self.device(format!("gamepad {id}"))
    }

    fn keyboard(&self) -> io::Result<Box<dyn Held>> {
        self.device("keyboard".into())
    }

    fn mouse(&self) -> io::Result<Box<dyn Held>> {
        self.device("mouse".into())
    }
}

impl Fake {
    fn device(&self, name: String) -> io::Result<Box<dyn Held>> {
        self.log.lock().unwrap().push(format!("make {name}"));
        Ok(Box::new(FakeDevice {
            name,
            file: std::fs::File::open(std::env::current_exe()?)?,
            log: self.log.clone(),
        }))
    }
}

struct Started {
    socket: PathBuf,
    dir: PathBuf,
    fake: Fake,
}

impl Drop for Started {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn start(name: &str, allow: bool) -> Started {
    let dir = std::env::temp_dir().join(format!("system-gobloon-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join("socket");
    let fake = Fake::default();
    let hide = Hide::load(
        &dir.join("state"),
        &dir.join("rules/72.rules"),
        Box::new(fake.clone()),
    );
    let virtuals = Virtuals::load(&dir.join("state"), Box::new(fake.clone()));
    let listener = server::bind(&socket).unwrap();
    let service = Service::new(hide, virtuals);
    std::thread::spawn(move || server::serve(listener, Arc::new(move |_| allow), service));
    Started { socket, dir, fake }
}

fn payload<R: Record>(r: &R) -> Vec<u8> {
    let mut out = vec![0; R::SIZE];
    r.write(&mut out);
    out
}

#[test]
fn hello_and_unsupported_requests() {
    let s = start("hello", true);
    let mut client = Client::connect_to(&s.socket).unwrap();
    let hello = client.hello("test").unwrap();
    assert_eq!(hello.protocol, PROTOCOL);
    assert_eq!(hello.service_version.as_str(), system_gobloon::VERSION);
    assert_ne!(hello.capabilities & caps::HIDE_EVERYWHERE, 0);
    // Not a request; the connection stays usable.
    assert!(matches!(
        client.call(Code::Error, &[]),
        Err(Error::Service(ErrorCode::Unsupported))
    ));
    assert!(client.hello("test").is_ok());
    assert!(matches!(
        client.call(Code::Hello, &[1, 2]),
        Err(Error::Service(ErrorCode::BadRequest))
    ));
    // A second socket in the same place replaces the stale one.
    drop(client);
    server::bind(&s.socket).unwrap();
}

#[test]
fn hiding_writes_state_and_rule() {
    let s = start("hide", true);
    s.fake.log.lock().unwrap().clear();
    let mut client = Client::connect_to(&s.socket).unwrap();
    let stick = Hidden {
        id: Text::new("AAAA"),
        name: Text::new("Stick"),
        vendor: 0x044f,
        product: 0xb10a,
        present: 0,
        reserved: [0; 3],
    };
    let set = |hidden| SetHidden {
        hidden,
        reserved: [0; 3],
        device: stick,
    };
    client.call(Code::SetHidden, &payload(&set(1))).unwrap();

    let rule = std::fs::read_to_string(s.dir.join("rules/72.rules")).unwrap();
    assert!(
        rule.contains("ATTRS{idProduct}==\"b10a\", ATTRS{serial}==\"SN1\""),
        "{rule}"
    );
    let state = std::fs::read_to_string(s.dir.join("state/hidden.toml")).unwrap();
    assert!(state.contains("serial = \"SN1\""), "{state}");
    assert_eq!(
        *s.fake.log.lock().unwrap(),
        ["trigger", "close /dev/input/event4", "close /dev/hidraw3"]
    );

    let (reply, _) = client.call(Code::HideState, &[]).unwrap();
    let (state, records) = hidden_records(&reply).unwrap();
    let records: Vec<Hidden> = records.collect();
    assert_eq!(state.enabled, 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name.as_str(), "Stick");
    assert_eq!(
        records[0].present, 1,
        "attached (ids compare ignoring case)"
    );

    // Off: no rule, nothing closed, the device stays listed.
    s.fake.log.lock().unwrap().clear();
    let off = SetHideEnabled {
        enabled: 0,
        reserved: [0; 3],
    };
    client.call(Code::SetHideEnabled, &payload(&off)).unwrap();
    assert!(!s.dir.join("rules/72.rules").exists());
    assert_eq!(*s.fake.log.lock().unwrap(), ["trigger"]);
    let (reply, _) = client.call(Code::HideState, &[]).unwrap();
    assert_eq!(hidden_records(&reply).unwrap().0.enabled, 0);

    // Shown: gone from the list.
    client.call(Code::SetHidden, &payload(&set(0))).unwrap();
    let (reply, _) = client.call(Code::HideState, &[]).unwrap();
    assert_eq!(hidden_records(&reply).unwrap().0.count, 0);
    assert!(matches!(
        client.call(Code::SetHidden, &[0; 3]),
        Err(Error::Service(ErrorCode::BadRequest))
    ));
}

#[test]
fn hidden_nodes_are_handed_over() {
    let s = start("open", true);
    let mut client = Client::connect_to(&s.socket).unwrap();
    let node = Path::new("/dev/input/event4");
    let not_found =
        |r: Result<OwnedFd, Error>| matches!(r, Err(Error::Service(ErrorCode::NotFound)));
    assert!(not_found(client.open_hidden(node)), "not hidden");
    let stick = Hidden {
        id: Text::new("aaaa"),
        name: Text::new("Stick"),
        vendor: 0x044f,
        product: 0xb10a,
        present: 0,
        reserved: [0; 3],
    };
    client.set_hidden(stick, true).unwrap();
    let mut file = std::fs::File::from(client.open_hidden(node).unwrap());
    let mut first = [0u8; 4];
    io::Read::read_exact(&mut file, &mut first).unwrap();
    assert_eq!(&first, b"\x7fELF", "the file the fake opened");
    assert!(
        not_found(client.open_hidden(Path::new("/etc/shadow"))),
        "nothing else"
    );
    assert!(client.open_hidden(Path::new("/dev/hidraw3")).is_ok());
    let (on, hidden) = client.hide_state().unwrap();
    assert!(on && hidden.len() == 1);
}

#[test]
fn virtual_devices_are_kept_and_lent() {
    use gob_system::protocol::{ApplyVirtual, ChangeKind, Owner};

    let s = start("virtual", true);
    // No layout file: one joystick, as without the service.
    assert_eq!(*s.fake.log.lock().unwrap(), ["make joystick 1"]);
    let mut a = Client::connect_to(&s.socket).unwrap();
    let mut b = Client::connect_to(&s.socket).unwrap();
    let (joysticks, gamepads) = a.virtual_state().unwrap();
    assert_eq!(joysticks.len(), 1);
    assert_eq!(joysticks[0].buttons, 95);
    assert!(gamepads.is_empty());

    let pad = |kind: ChangeKind, id| ApplyVirtual {
        kind: kind as u8,
        reserved: [0; 3],
        joystick: gob_virtual::uinput::joystick_record(
            &gob_virtual::DeviceConfig::new(id, &gob_virtual::capabilities()),
            Owner::Free,
        ),
    };
    a.apply_virtual(pad(ChangeKind::AddGamepad, 2)).unwrap();
    let layout = std::fs::read_to_string(s.dir.join("state/virtual.toml")).unwrap();
    assert!(layout.contains("[[gamepad]]"), "{layout}");

    let fd = a.acquire(DeviceKind::Gamepad, 2).unwrap();
    drop(fd);
    assert!(
        matches!(
            b.acquire(DeviceKind::Gamepad, 2),
            Err(Error::Service(ErrorCode::Busy))
        ),
        "a has it"
    );
    let (_, gamepads) = b.virtual_state().unwrap();
    assert_eq!(gamepads[0].owner, Owner::Other as u8);
    let (_, gamepads) = a.virtual_state().unwrap();
    assert_eq!(gamepads[0].owner, Owner::You as u8);
    assert!(
        matches!(
            b.apply_virtual(pad(ChangeKind::DeleteGamepad, 2)),
            Err(Error::Service(ErrorCode::Busy))
        ),
        "in use: no change"
    );

    // a goes away: the gamepad is centered and free for b.
    s.fake.log.lock().unwrap().clear();
    drop(a);
    let free = (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(10));
        b.acquire(DeviceKind::Gamepad, 2).is_ok()
    });
    assert!(free);
    let centered = format!(
        "gamepad 2 {} events",
        gob_virtual::uinput::gamepad_reset().len()
    );
    assert_eq!(s.fake.log.lock().unwrap()[0], centered);
    b.release(DeviceKind::Gamepad, 2).unwrap();
    b.apply_virtual(pad(ChangeKind::DeleteGamepad, 2)).unwrap();
    assert!(b.virtual_state().unwrap().1.is_empty());
    assert!(matches!(
        b.acquire(DeviceKind::Gamepad, 2),
        Err(Error::Service(ErrorCode::NotFound))
    ));
}

#[test]
fn keyboard_and_mouse_are_made_once_and_released() {
    let s = start("keyboard", true);
    let mut a = Client::connect_to(&s.socket).unwrap();
    let mut b = Client::connect_to(&s.socket).unwrap();
    s.fake.log.lock().unwrap().clear();
    a.acquire(DeviceKind::Keyboard, 0).unwrap();
    a.acquire(DeviceKind::Mouse, 0).unwrap();
    assert!(matches!(
        b.acquire(DeviceKind::Keyboard, 0),
        Err(Error::Service(ErrorCode::Busy))
    ));
    drop(a);
    let free = (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(10));
        b.acquire(DeviceKind::Keyboard, 0).is_ok()
    });
    assert!(free, "free again when a went away");
    let log = s.fake.log.lock().unwrap().clone();
    assert_eq!(log[..2], ["make keyboard", "make mouse"], "{log:?}");
    let up = format!(
        "keyboard {} events",
        gob_uinput::output::keyboard_release().len()
    );
    assert!(log.contains(&up), "keys released: {log:?}");
    assert_eq!(log.iter().filter(|l| l.starts_with("make")).count(), 2);
}

#[test]
fn users_without_a_seat_are_refused() {
    let s = start("denied", false);
    let mut client = Client::connect_to(&s.socket).unwrap();
    assert!(matches!(
        client.hello("test"),
        Err(Error::Service(ErrorCode::Denied))
    ));
}

#[test]
fn no_service() {
    let path = std::env::temp_dir().join("system-gobloon-none.sock");
    assert!(matches!(Client::connect_to(&path), Err(Error::Io(_))));
}
