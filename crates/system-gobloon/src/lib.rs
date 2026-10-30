//! SystemGobloon for Linux: a root service (systemd unit `system-gobloon.service`) that does what
//! the HidHide and vJoy drivers do on Windows. It listens on `gob_system::SOCKET_PATH` and speaks
//! the gob-system protocol; only root and the user of an active seat may use it (`seat.rs`).
//! Hiding: `hide.rs`; virtual devices: `virtuals.rs`. No GUI code: Joystick Gobloon talks to it
//! through gob-system's client.
#![cfg(target_os = "linux")]

use std::sync::{Arc, Mutex};

pub mod hide;
pub mod journal;
pub mod seat;
pub mod server;
pub mod virtuals;

/// The version reported in `Hello`: Joystick Gobloon's of the same build (build.rs).
pub const VERSION: &str = env!("JOYSTICK_GOBLOON_VERSION");

/// The service's state (`hidden.toml`, `virtual.toml`).
pub const STATE_DIR: &str = "/var/lib/system-gobloon";

/// Runtime udev rule giving the user at the seat the game controllers' event nodes (hidden ones
/// excepted: `hide::RULE_PATH` comes after it), so nothing else needs installing. Virtual
/// devices need no access to /dev/uinput: the service lends them.
pub const ACCESS_RULE_PATH: &str = "/run/udev/rules.d/70-system-gobloon-access.rules";

pub const ACCESS_RULE: &str = "\
# SystemGobloon: the user at the seat may read game controllers (written by the service).
SUBSYSTEM==\"input\", KERNEL==\"event*\", ENV{ID_INPUT_JOYSTICK}==\"1\", TAG+=\"uaccess\"
";

/// Writes [`ACCESS_RULE`] to `path`; whether it changed (udev must then apply it).
pub fn write_access_rule(path: &std::path::Path) -> std::io::Result<bool> {
    if std::fs::read_to_string(path).is_ok_and(|t| t == ACCESS_RULE) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, ACCESS_RULE)?;
    Ok(true)
}

/// What the requests work on, shared by the client threads.
pub struct Service {
    pub hide: Arc<Mutex<hide::Hide>>,
    pub virtuals: Mutex<virtuals::Virtuals>,
}

impl Service {
    pub fn new(hide: hide::Hide, virtuals: virtuals::Virtuals) -> Arc<Service> {
        Arc::new(Service {
            hide: Arc::new(Mutex::new(hide)),
            virtuals: Mutex::new(virtuals),
        })
    }
}
