//! Linux backend: the input service reads hidden devices exclusively (gob-input's `exclusive`,
//! `EVIOCGRAB`), so games get nothing from them through evdev or joydev while Joystick Gobloon
//! runs. Programs that open the raw HID device (hidraw: Steam, SDL's HIDAPI drivers, Wine) still
//! see them when the user may open its node, as udev rules of many gaming setups allow: then a
//! udev rule closes the node of hidden sticks again (gob-uinput's `set_hidden_sticks`, with the
//! administrator password, in the background). Until it does, or when that was refused, such
//! devices are marked `exposed`. No program list: the hiding program is the only one that reads.
//!
//! The switch and the hidden devices are kept in `hidden.gob` (TOML) in the data directory;
//! hidden devices stay listed while unplugged.
//!
//! With SystemGobloon running, `service.rs` is used instead: the service closes hidden devices
//! to every program and keeps the state; this program still grabs them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use gob_input::DeviceInfo;
use serde::{Deserialize, Serialize};

use crate::{Backend, Capabilities, Device, Error, Part};

pub(crate) mod service;

pub const CAPABILITIES: Capabilities = Capabilities {
    name: "Linux evdev",
    app_list: false,
    hides_everywhere: false,
};
const STATE_FILE: &str = "hidden.gob";

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    if !gob_input::exclusive::supported() {
        return Err(Error::Unsupported);
    }
    if let Some(service) = service::Service::connect() {
        return Ok(Arc::new(service));
    }
    let hide = Grab;
    // The service may have been removed since: nodes open as usual again.
    gob_input::set_opener(None);
    // The stored state applies from the start, also before the window is opened.
    apply(&read_state()?);
    Ok(Arc::new(hide))
}

/// `hidden.gob`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    #[serde(default = "on")]
    enabled: bool,
    #[serde(default, rename = "device")]
    devices: Vec<Hidden>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: true,
            devices: Vec::new(),
        }
    }
}

fn on() -> bool {
    true
}

/// A hidden device, remembered with what is needed to list it while unplugged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Hidden {
    /// gob-input's device id.
    id: String,
    name: String,
    vendor: u16,
    product: u16,
}

fn state_path() -> Option<PathBuf> {
    let dir = crate::data_dir().or_else(|| {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(config.join("JoystickGobloon"))
    })?;
    Some(dir.join(STATE_FILE))
}

fn parse(text: &str) -> Result<State, Error> {
    toml::from_str(text).map_err(|e| Error::Failed(format!("{STATE_FILE}: {e}")))
}

fn read_state() -> Result<State, Error> {
    match state_path().map(std::fs::read_to_string) {
        Some(Ok(text)) => parse(&text),
        Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(Error::Failed(format!("{STATE_FILE}: {e}")))
        }
        _ => Ok(State::default()),
    }
}

/// The sticks (vendor, product) whose raw HID node local hiding keeps closed: every hidden stick
/// while hiding is on.
pub fn closed_sticks_wanted() -> Vec<(u16, u16)> {
    let Ok(state) = read_state() else {
        return Vec::new();
    };
    let mut sticks: Vec<(u16, u16)> = state
        .devices
        .iter()
        .filter(|_| state.enabled)
        .map(|h| (h.vendor, h.product))
        .filter(|&ids| ids != (0, 0))
        .collect();
    sticks.sort_unstable();
    sticks.dedup();
    sticks
}

fn write_state(state: &State) -> Result<(), Error> {
    let path = state_path().ok_or_else(|| Error::Failed("no folder for the state".into()))?;
    let text = toml::to_string_pretty(state).map_err(|e| Error::Failed(e.to_string()))?;
    let failed = |e: std::io::Error| Error::Failed(format!("{}: {e}", path.display()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(failed)?;
    }
    std::fs::write(&path, text).map_err(failed)
}

/// Tells the input service what to read exclusively.
fn apply(state: &State) {
    let ids = state.devices.iter().map(|d| d.id.clone());
    if state.enabled {
        gob_input::exclusive::set(ids);
    } else {
        gob_input::exclusive::set(std::iter::empty());
    }
}

/// The attached devices, then the hidden ones that are unplugged.
fn list(attached: &[DeviceInfo], state: &State) -> Vec<Device> {
    let is_hidden = |id: &str| state.devices.iter().any(|d| d.id.eq_ignore_ascii_case(id));
    let device = |id: &str, name: &str, vid_pid, present, detail: String| Device {
        name: name.to_string(),
        present,
        hidden: is_hidden(id),
        exposed: false,
        parts: vec![Part {
            present,
            vid_pid: Some(vid_pid),
            product: name.to_string(),
        }],
        detail,
        key: vec![id.to_string()],
    };
    let mut devices: Vec<Device> = attached
        .iter()
        .map(|d| {
            let detail = [d.guid.as_str(), d.hid_path.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join("\n");
            let mut device = device(&d.guid, &d.name, (d.vendor_id, d.product_id), true, detail);
            device.exposed = device.hidden && readable(&d.hid_path);
            device
        })
        .collect();
    for h in &state.devices {
        if !attached.iter().any(|d| d.guid.eq_ignore_ascii_case(&h.id)) {
            devices.push(device(
                &h.id,
                &h.name,
                (h.vendor, h.product),
                false,
                h.id.clone(),
            ));
        }
    }
    devices
}

/// Whether this user may read `path` (an empty path: no).
fn readable(path: &str) -> bool {
    let Ok(path) = std::ffi::CString::new(path) else {
        return false;
    };
    // SAFETY: a valid NUL-terminated path; access() only checks permissions.
    !path.as_bytes().is_empty() && unsafe { libc::access(path.as_ptr(), libc::R_OK) } == 0
}

/// The sticks whose raw HID node stays closed: hidden ones (while hiding is on) the user may
/// open now, or closed already.
fn closed_sticks(state: &State, attached: &[DeviceInfo], closed: &[(u16, u16)]) -> Vec<(u16, u16)> {
    if !state.enabled {
        return Vec::new();
    }
    let open = |h: &Hidden| {
        attached
            .iter()
            .any(|d| d.guid.eq_ignore_ascii_case(&h.id) && readable(&d.hid_path))
    };
    let mut sticks: Vec<(u16, u16)> = state
        .devices
        .iter()
        .filter(|h| (h.vendor, h.product) != (0, 0))
        .filter(|h| closed.contains(&(h.vendor, h.product)) || open(h))
        .map(|h| (h.vendor, h.product))
        .collect();
    sticks.sort_unstable();
    sticks.dedup();
    sticks
}

/// The sticks last asked for in [`close_raw_nodes`].
static ASKED: Mutex<Option<Vec<(u16, u16)>>> = Mutex::new(None);
/// The rule changed in the background (see `Backend::take_changed`).
static CHANGED: AtomicBool = AtomicBool::new(false);

/// Makes the hidden-stick rule match the state, in the background: the system may ask for the
/// password. `again`: also when the same sticks were asked for before (and refused); else a
/// refusal is not asked again until the user changes something.
fn close_raw_nodes(again: bool) {
    std::thread::spawn(move || {
        let mut asked = ASKED.lock().unwrap_or_else(PoisonError::into_inner);
        let Ok(state) = read_state() else { return };
        let closed = gob_uinput::access::hidden_sticks();
        let sticks = closed_sticks(&state, &gob_input::exclusive::devices(), &closed);
        if !again && asked.as_ref() == Some(&sticks) {
            return;
        }
        *asked = Some(sticks.clone());
        match gob_uinput::access::set_hidden_sticks(&sticks) {
            Ok(true) => {
                log::info!("raw HID nodes closed for {sticks:04x?}");
                CHANGED.store(true, Ordering::Relaxed);
            }
            Ok(false) => {}
            Err(e) => log::warn!("closing the raw HID nodes of hidden sticks: {e}"),
        }
    });
}

/// `state` with `device` hidden or shown.
fn changed(mut state: State, device: &Device, hidden: bool) -> Result<State, Error> {
    let id = device
        .key
        .first()
        .ok_or_else(|| Error::Failed("device without an id".into()))?;
    state.devices.retain(|d| !d.id.eq_ignore_ascii_case(id));
    if hidden {
        let (vendor, product) = device
            .parts
            .first()
            .and_then(|p| p.vid_pid)
            .unwrap_or_default();
        state.devices.push(Hidden {
            id: id.clone(),
            name: device.name.clone(),
            vendor,
            product,
        });
    }
    Ok(state)
}

struct Grab;

impl Backend for Grab {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    /// The kernel's version: evdev comes with it.
    fn version(&self) -> String {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|v| v.trim().to_string())
            .unwrap_or_default()
    }

    fn enabled(&self) -> Result<bool, Error> {
        Ok(read_state()?.enabled)
    }

    fn set_enabled(&self, on: bool) -> Result<(), Error> {
        let state = State {
            enabled: on,
            ..read_state()?
        };
        write_state(&state)?;
        apply(&state);
        close_raw_nodes(true);
        Ok(())
    }

    /// Every device here is a game controller, so `gaming_only` changes nothing.
    /// A hidden stick the user may open the raw node of (plugged in, or hidden before this
    /// closed it) gets it closed.
    fn devices(&self, _gaming_only: bool) -> Result<Vec<Device>, Error> {
        let devices = list(&gob_input::exclusive::devices(), &read_state()?);
        if devices.iter().any(|d| d.exposed) {
            close_raw_nodes(false);
        }
        Ok(devices)
    }

    fn set_hidden(&self, device: &Device, hidden: bool) -> Result<(), Error> {
        let state = changed(read_state()?, device, hidden)?;
        write_state(&state)?;
        apply(&state);
        close_raw_nodes(true);
        Ok(())
    }

    fn take_changed(&self) -> bool {
        CHANGED.swap(false, Ordering::Relaxed)
    }

    fn apps(&self) -> Result<Vec<PathBuf>, Error> {
        Ok(Vec::new())
    }

    /// There is no list: removing (the uninstaller's call) is done already, adding is not
    /// possible.
    fn set_app(&self, _exe: &Path, listed: bool) -> Result<(), Error> {
        if listed {
            Err(Error::Unsupported)
        } else {
            Ok(())
        }
    }

    fn apps_inverted(&self) -> Result<bool, Error> {
        Ok(false)
    }

    fn set_apps_inverted(&self, _on: bool) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(guid: &str, name: &str) -> DeviceInfo {
        DeviceInfo {
            guid: guid.into(),
            name: name.into(),
            vendor_id: 0x231d,
            product_id: 0x0200,
            ..DeviceInfo::default()
        }
    }

    #[test]
    fn state_file_and_device_list() {
        assert_eq!(parse("").unwrap(), State::default(), "on, nothing hidden");
        let attached = [info("aaaa", "EVO L"), info("bbbb", "EVO R")];
        let devices = list(&attached, &State::default());
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().all(|d| d.present && !d.hidden));

        let state = changed(State::default(), &devices[1], true).unwrap();
        let text = toml::to_string_pretty(&state).unwrap();
        assert_eq!(parse(&text).unwrap(), state);
        let devices = list(&attached, &state);
        assert!(!devices[0].hidden && devices[1].hidden);
        assert_eq!(devices[1].parts[0].vid_pid, Some((0x231d, 0x0200)));

        // Unplugged: still listed, as hidden and not present.
        let devices = list(&attached[..1], &state);
        assert_eq!(devices.len(), 2);
        assert!(!devices[1].present && devices[1].hidden);
        assert_eq!(devices[1].name, "EVO R");

        let state = changed(state, &devices[1], false).unwrap();
        assert!(state.devices.is_empty());
    }

    #[test]
    fn hidden_device_with_a_readable_raw_node_is_exposed() {
        // Any file the user can read stands in for an open hidraw node.
        let open = std::env::current_exe().unwrap().display().to_string();
        let attached = [
            DeviceInfo {
                hid_path: open.clone(),
                ..info("aaaa", "EVO L")
            },
            DeviceInfo {
                hid_path: "/nonexistent/hidraw0".into(),
                ..info("bbbb", "EVO R")
            },
            DeviceInfo {
                hid_path: open,
                ..info("cccc", "Pad")
            },
        ];
        let mut state = State::default();
        for d in &list(&attached, &state)[..2] {
            state = changed(state, d, true).unwrap();
        }
        let devices = list(&attached, &state);
        assert!(devices[0].exposed, "hidden, node open to the user");
        assert!(!devices[1].exposed, "hidden, no node");
        assert!(!devices[2].exposed, "not hidden");
        assert!(!readable(""));
    }

    #[test]
    fn raw_nodes_closed_for_open_or_closed_hidden_sticks() {
        let open = std::env::current_exe().unwrap().display().to_string();
        let stick = |guid: &str, product, hid_path: &str| DeviceInfo {
            hid_path: hid_path.into(),
            product_id: product,
            ..info(guid, "Stick")
        };
        let attached = [
            stick("aaaa", 1, &open),
            stick("bbbb", 2, "/nonexistent/hidraw0"),
            stick("cccc", 3, &open),
        ];
        let mut state = State::default();
        for d in &list(&attached, &state)[..2] {
            state = changed(state, d, true).unwrap();
        }
        assert_eq!(
            closed_sticks(&state, &attached, &[]),
            [(0x231d, 1)],
            "hidden and open"
        );
        assert_eq!(
            closed_sticks(&state, &attached, &[(0x231d, 2), (0x231d, 3)]),
            [(0x231d, 1), (0x231d, 2)],
            "closed stays closed while hidden; shown opens again"
        );
        assert_eq!(
            closed_sticks(&state, &[], &[(0x231d, 1)]),
            [(0x231d, 1)],
            "unplugged"
        );
        state.enabled = false;
        assert!(
            closed_sticks(&state, &attached, &[(0x231d, 1)]).is_empty(),
            "hiding off"
        );
    }
}
