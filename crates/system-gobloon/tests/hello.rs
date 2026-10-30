//! The service and gob-system's client over a real socket, with a fake system (no root, no
//! devices needed).
#![cfg(target_os = "linux")]

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gob_input::Nodes;
use gob_system::protocol::{
    Code, ErrorCode, Hidden, PROTOCOL, Record, SetHidden, SetHideEnabled, Text, caps,
    hidden_records,
};
use gob_system::{Client, Error};
use system_gobloon::hide::{Hide, System};
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
    let listener = server::bind(&socket).unwrap();
    let service = Service::new(hide);
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
    // Not there yet; the connection stays usable.
    assert!(matches!(
        client.call(Code::VirtualState, &[]),
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
