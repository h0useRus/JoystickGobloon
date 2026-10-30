//! SystemGobloon for Linux: a root service (systemd unit `system-gobloon.service`) that does what
//! the HidHide and vJoy drivers do on Windows. It listens on `gob_system::SOCKET_PATH` and speaks
//! the gob-system protocol; only root and the user of an active seat may use it (`seat.rs`).
//! No GUI code: Joystick Gobloon talks to it through gob-system's client.
#![cfg(target_os = "linux")]

pub mod journal;
pub mod seat;
pub mod server;

/// The version reported in `Hello`: the release's, else the crate's.
pub const VERSION: &str = match option_env!("JOYSTICK_GOBLOON_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};
