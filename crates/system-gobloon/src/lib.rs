//! SystemGobloon for Linux: a root service (systemd unit `system-gobloon.service`) that does what
//! the HidHide and vJoy drivers do on Windows. It listens on `gob_system::SOCKET_PATH` and speaks
//! the gob-system protocol; only root and the user of an active seat may use it (`seat.rs`).
//! Hiding: `hide.rs`. No GUI code: Joystick Gobloon talks to it through gob-system's client.
#![cfg(target_os = "linux")]

use std::sync::{Arc, Mutex};

pub mod hide;
pub mod journal;
pub mod seat;
pub mod server;

/// The version reported in `Hello`: the release's, else the crate's.
pub const VERSION: &str = match option_env!("JOYSTICK_GOBLOON_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// The service's state (`hidden.toml`, later `virtual.toml`).
pub const STATE_DIR: &str = "/var/lib/system-gobloon";

/// The systemd unit (`/etc/systemd/system/system-gobloon.service`). Stopping or a crash runs
/// `--cleanup`: hidden devices become visible again.
pub const UNIT: &str = "\
[Unit]
Description=SystemGobloon: device hiding and virtual devices for Joystick Gobloon
After=systemd-udevd.service

[Service]
ExecStart=/var/lib/system-gobloon/bin/SystemGobloon
ExecStopPost=/var/lib/system-gobloon/bin/SystemGobloon --cleanup
Restart=on-failure

[Install]
WantedBy=multi-user.target
";

/// What the requests work on, shared by the client threads.
pub struct Service {
    pub hide: Arc<Mutex<hide::Hide>>,
}

impl Service {
    pub fn new(hide: hide::Hide) -> Arc<Service> {
        Arc::new(Service {
            hide: Arc::new(Mutex::new(hide)),
        })
    }
}
