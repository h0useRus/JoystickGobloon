//! The service and gob-system's client over a real socket (no root needed).
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::sync::Arc;

use gob_system::protocol::{Code, ErrorCode, PROTOCOL};
use gob_system::{Client, Error};
use system_gobloon::server;

fn start(name: &str, allow: bool) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("system-gobloon-{name}-{}.sock", std::process::id()));
    let listener = server::bind(&path).unwrap();
    std::thread::spawn(move || server::serve(listener, Arc::new(move |_| allow)));
    path
}

#[test]
fn hello_and_unsupported_requests() {
    let path = start("hello", true);
    let mut client = Client::connect_to(&path).unwrap();
    let hello = client.hello("test").unwrap();
    assert_eq!(hello.protocol, PROTOCOL);
    assert_eq!(hello.service_version.as_str(), system_gobloon::VERSION);
    // Not there yet; the connection stays usable.
    assert!(matches!(
        client.call(Code::HideState, &[]),
        Err(Error::Service(ErrorCode::Unsupported))
    ));
    assert!(client.hello("test").is_ok());
    assert!(matches!(
        client.call(Code::Hello, &[1, 2]),
        Err(Error::Service(ErrorCode::BadRequest))
    ));
    // A second socket in the same place replaces the stale one.
    drop(client);
    server::bind(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn users_without_a_seat_are_refused() {
    let path = start("denied", false);
    let mut client = Client::connect_to(&path).unwrap();
    assert!(matches!(
        client.hello("test"),
        Err(Error::Service(ErrorCode::Denied))
    ));
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn no_service() {
    let path = std::env::temp_dir().join("system-gobloon-none.sock");
    assert!(matches!(Client::connect_to(&path), Err(Error::Io(_))));
}
