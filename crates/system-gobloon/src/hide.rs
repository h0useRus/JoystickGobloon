//! Hiding: the nodes of hidden devices (event, js, hidraw) belong to root alone, so no program of
//! the user opens them; Joystick Gobloon gets them from the service. Two layers:
//!
//! - A runtime udev rule ([`RULE_PATH`]) per hidden USB device (vendor, product and serial
//!   number when it has one): no `uaccess`, root only. udev applies it whenever the device is
//!   plugged in. After the rules that give access (`70-`), before systemd applies `uaccess`
//!   (73-seat-late.rules). In `/run`: gone after a reboot or `--cleanup` (the unit's
//!   `ExecStopPost`), so a stopped service leaves nothing hidden; the service writes it again
//!   when it starts.
//! - The nodes of attached hidden devices are closed directly too (owner, mode, access list),
//!   and again whenever something changes them ([`watch`]): also devices off USB, which the
//!   rule cannot match.
//!
//! Descriptors opened before hiding stay usable: programs reading the stick then have to be
//! restarted.

use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use gob_input::Nodes;
use gob_system::protocol::{Hidden, Text};
use serde::{Deserialize, Serialize};

/// The runtime rule (a different name than Joystick Gobloon's own `/etc` hidraw rule).
pub const RULE_PATH: &str = "/run/udev/rules.d/72-system-gobloon-hidden.rules";
const STATE_FILE: &str = "hidden.toml";

const RULE_HEADER: &str = "\
# SystemGobloon: devices hidden in Joystick Gobloon, only root may open them (written by the
# service, removed when it stops).
";

/// `hidden.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    #[serde(default = "on")]
    enabled: bool,
    #[serde(default, rename = "device")]
    devices: Vec<Device>,
}

impl Default for State {
    fn default() -> Self {
        State {
            enabled: true,
            devices: Vec::new(),
        }
    }
}

fn on() -> bool {
    true
}

/// A hidden device as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Device {
    /// gob-input's device id.
    id: String,
    name: String,
    vendor: u16,
    product: u16,
    /// USB serial number, when the device has one (found when it was hidden while attached).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    serial: Option<String>,
}

/// What hiding does to the system; tests replace it.
pub trait System: Send {
    /// The attached controllers and their nodes.
    fn controllers(&self) -> Vec<Nodes>;
    /// Reloads the udev rules and applies them to the existing input and hidraw nodes.
    fn trigger(&self) -> io::Result<()>;
    /// Makes a node root's alone; returns whether it was open to others.
    fn close(&self, path: &Path) -> io::Result<bool>;
    /// Opens a node for non-blocking reads.
    fn open(&self, path: &Path) -> io::Result<OwnedFd>;
}

/// The service's hiding state.
pub struct Hide {
    state: State,
    state_path: PathBuf,
    rule_path: PathBuf,
    system: Box<dyn System>,
}

impl Hide {
    /// Reads the state from `state_dir` (missing or unreadable: nothing hidden).
    pub fn load(state_dir: &Path, rule_path: &Path, system: Box<dyn System>) -> Hide {
        let state_path = state_dir.join(STATE_FILE);
        let state = match std::fs::read_to_string(&state_path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                log::error!("{}: {e}", state_path.display());
                State::default()
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => State::default(),
            Err(e) => {
                log::error!("{}: {e}", state_path.display());
                State::default()
            }
        };
        Hide {
            state,
            state_path,
            rule_path: rule_path.to_path_buf(),
            system,
        }
    }

    pub fn enabled(&self) -> bool {
        self.state.enabled
    }

    /// The hidden devices, `present` when attached.
    pub fn hidden(&self) -> Vec<Hidden> {
        let attached = self.system.controllers();
        self.state
            .devices
            .iter()
            .map(|d| Hidden {
                id: Text::new(&d.id),
                name: Text::new(&d.name),
                vendor: d.vendor,
                product: d.product,
                present: attached
                    .iter()
                    .any(|n| n.guid.eq_ignore_ascii_case(&d.id))
                    .into(),
                reserved: [0; 3],
            })
            .collect()
    }

    pub fn set_enabled(&mut self, on: bool) -> io::Result<()> {
        self.state.enabled = on;
        log::info!("hiding {}", if on { "on" } else { "off" });
        self.save()?;
        self.apply()
    }

    /// Hides or shows a device.
    pub fn set_hidden(&mut self, device: &Hidden, hidden: bool) -> io::Result<()> {
        let id = device.id.as_str();
        if id.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "no device id"));
        }
        let known = self
            .state
            .devices
            .iter()
            .find(|d| d.id.eq_ignore_ascii_case(id));
        let serial = self
            .system
            .controllers()
            .into_iter()
            .find(|n| n.guid.eq_ignore_ascii_case(id))
            .and_then(|n| n.usb_serial)
            .filter(|s| !s.is_empty())
            .or_else(|| known.and_then(|d| d.serial.clone()));
        self.state
            .devices
            .retain(|d| !d.id.eq_ignore_ascii_case(id));
        if hidden {
            self.state.devices.push(Device {
                id: id.to_string(),
                name: device.name.as_str().to_string(),
                vendor: device.vendor,
                product: device.product,
                serial,
            });
        }
        log::info!(
            "{} {} ({id})",
            if hidden { "hiding" } else { "showing" },
            device.name.as_str()
        );
        self.save()?;
        self.apply()
    }

    fn save(&self) -> io::Result<()> {
        let text = toml::to_string_pretty(&self.state).map_err(io::Error::other)?;
        if let Some(dir) = self.state_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        write_atomic(&self.state_path, &text)
    }

    /// Writes the rule for the state (or removes it), has udev apply it and closes the nodes of
    /// the attached hidden devices.
    pub fn apply(&mut self) -> io::Result<()> {
        let text = rules(&self.state);
        if text.is_empty() {
            match std::fs::remove_file(&self.rule_path) {
                // No rule before, none now: nothing for udev to do.
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e),
                Ok(()) => {}
            }
        } else {
            if let Some(dir) = self.rule_path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            write_atomic(&self.rule_path, &text)?;
        }
        self.system.trigger()?;
        self.enforce();
        Ok(())
    }

    /// Closes the nodes of attached hidden devices that are open to others; returns how many.
    pub fn enforce(&self) -> usize {
        if !self.state.enabled || self.state.devices.is_empty() {
            return 0;
        }
        let mut closed = 0;
        for nodes in self.system.controllers() {
            if !self.is_hidden(&nodes.guid) {
                continue;
            }
            for path in &nodes.paths {
                match self.system.close(path) {
                    Ok(true) => {
                        log::info!("{}: closed ({})", path.display(), nodes.name);
                        closed += 1;
                    }
                    Ok(false) => {}
                    Err(e) => log::warn!("{}: {e}", path.display()),
                }
            }
        }
        closed
    }

    /// Opens a node of an attached hidden device for the client; `NotFound` for any other path
    /// (the service hands out nothing else).
    pub fn open_hidden(&self, path: &Path) -> io::Result<OwnedFd> {
        let ours = self
            .system
            .controllers()
            .into_iter()
            .filter(|n| self.is_hidden(&n.guid))
            .any(|n| n.paths.iter().any(|p| p == path));
        if !ours {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "not a hidden device",
            ));
        }
        self.system.open(path)
    }

    fn is_hidden(&self, id: &str) -> bool {
        self.state
            .devices
            .iter()
            .any(|d| d.id.eq_ignore_ascii_case(id))
    }
}

/// The rule file for the state; empty when nothing is hidden (or hiding is off).
fn rules(state: &State) -> String {
    if !state.enabled {
        return String::new();
    }
    let mut lines: Vec<String> = state
        .devices
        .iter()
        .filter(|d| d.vendor != 0 && d.product != 0)
        .map(|d| {
            let serial = d
                .serial
                .as_deref()
                .filter(|s| is_rule_safe(s))
                .map(|s| format!(", ATTRS{{serial}}==\"{s}\""))
                .unwrap_or_default();
            format!(
                "SUBSYSTEM==\"input|hidraw\", KERNEL==\"event*|js*|hidraw*\", \
                 ATTRS{{idVendor}}==\"{:04x}\", ATTRS{{idProduct}}==\"{:04x}\"{serial}, \
                 TAG-=\"uaccess\", OWNER=\"root\", GROUP=\"root\", MODE=\"0600\"\n",
                d.vendor, d.product
            )
        })
        .collect();
    lines.dedup();
    if lines.is_empty() {
        return String::new();
    }
    let mut text = RULE_HEADER.to_string();
    lines.iter().for_each(|l| text.push_str(l));
    text
}

/// Whether a serial number can stand in a udev match as it is (no quotes, patterns or control
/// characters); otherwise the rule matches vendor and product alone.
fn is_rule_safe(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\\' | '*' | '?' | '[' | ']' | '|'))
}

/// Writes a file through a temporary one next to it, so readers never see half of it.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Removes the rule and has udev give access back (`--cleanup`, the unit's `ExecStopPost`).
pub fn cleanup(rule_path: &Path, system: &dyn System) -> io::Result<()> {
    match std::fs::remove_file(rule_path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    system.trigger()
}

/// The real system: gob-input finds the nodes, udevadm applies rules, the node's owner, mode and
/// access list are set directly.
pub struct Udev;

impl System for Udev {
    fn controllers(&self) -> Vec<Nodes> {
        gob_input::controller_nodes()
    }

    fn trigger(&self) -> io::Result<()> {
        udevadm(&["control", "--reload"])?;
        udevadm(&[
            "trigger",
            "--action=change",
            "--subsystem-match=input",
            "--subsystem-match=hidraw",
        ])?;
        udevadm(&["settle", "--timeout=5"])
    }

    fn open(&self, path: &Path) -> io::Result<OwnedFd> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)?;
        Ok(file.into())
    }

    fn close(&self, path: &Path) -> io::Result<bool> {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let meta = std::fs::metadata(path)?;
        let mut changed = false;
        if meta.uid() != 0 || meta.gid() != 0 {
            std::os::unix::fs::chown(path, Some(0), Some(0))?;
            changed = true;
        }
        if meta.mode() & 0o077 != 0 {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            changed = true;
        }
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        // SAFETY: valid NUL-terminated strings; removexattr only reads them.
        let r = unsafe { libc::removexattr(c_path.as_ptr(), c"system.posix_acl_access".as_ptr()) };
        if r == 0 {
            changed = true;
        } else {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::ENODATA) {
                return Err(e);
            }
        }
        Ok(changed)
    }
}

fn udevadm(args: &[&str]) -> io::Result<()> {
    let status = std::process::Command::new("udevadm").args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("udevadm {}: {status}", args[0])))
    }
}

/// Watches `/dev` and `/dev/input` (inotify: nodes made, owner or mode changed) and closes hidden
/// devices' nodes again when someone opened them: another rule, `setfacl`, a replugged device
/// off USB. Runs until the process ends.
pub fn watch(hide: Arc<Mutex<Hide>>) -> io::Result<()> {
    // SAFETY: plain syscalls on a descriptor we own; the buffer outlives every read.
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    for dir in [c"/dev", c"/dev/input"] {
        let mask = libc::IN_ATTRIB | libc::IN_CREATE;
        if unsafe { libc::inotify_add_watch(fd, dir.as_ptr(), mask) } < 0 {
            log::warn!("watching {dir:?}: {}", io::Error::last_os_error());
        }
    }
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 {
                log::error!("watching device nodes: {}", io::Error::last_os_error());
                return;
            }
            // Many events come at once (a device brings several nodes, udev sets them in
            // steps): let them settle, drop what came meanwhile, look once.
            std::thread::sleep(Duration::from_millis(200));
            drain(fd, &mut buf);
            hide.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .enforce();
        }
    });
    Ok(())
}

/// Reads what is waiting on the inotify descriptor without blocking.
fn drain(fd: libc::c_int, buf: &mut [u8]) {
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd; reads go into `buf`.
    while unsafe { libc::poll(&mut poll, 1, 0) } > 0 {
        if unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) } <= 0 {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, product: u16, serial: Option<&str>) -> Device {
        Device {
            id: id.into(),
            name: "Stick".into(),
            vendor: 0x044f,
            product,
            serial: serial.map(Into::into),
        }
    }

    #[test]
    fn rule_text() {
        assert_eq!(rules(&State::default()), "", "nothing hidden: no file");
        let state = State {
            enabled: true,
            devices: vec![
                device("a", 0xb10a, Some("ABC123")),
                device("b", 0xb687, None),
                device("c", 0xb687, Some("bad\"serial")),
                device("d", 0, None),
            ],
        };
        let text = rules(&state);
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(lines.len(), 2, "same match once, no ids: none");
        assert_eq!(
            lines[0],
            "SUBSYSTEM==\"input|hidraw\", KERNEL==\"event*|js*|hidraw*\", \
             ATTRS{idVendor}==\"044f\", ATTRS{idProduct}==\"b10a\", ATTRS{serial}==\"ABC123\", \
             TAG-=\"uaccess\", OWNER=\"root\", GROUP=\"root\", MODE=\"0600\""
        );
        assert!(!lines[1].contains("serial"), "unsafe serial left out");
        let off = State {
            enabled: false,
            ..state
        };
        assert_eq!(rules(&off), "");
        // Before systemd applies uaccess, after the rules giving it.
        let name = RULE_PATH.rsplit('/').next().unwrap();
        assert!(name > "71" && name < "73-seat-late.rules");
    }

    #[test]
    fn state_file() {
        let state = State {
            enabled: false,
            devices: vec![device("a", 1, Some("S1")), device("b", 2, None)],
        };
        let text = toml::to_string_pretty(&state).unwrap();
        assert!(!text.contains("serial = \"\""));
        assert_eq!(toml::from_str::<State>(&text).unwrap(), state);
        assert_eq!(toml::from_str::<State>("").unwrap(), State::default());
    }
}
