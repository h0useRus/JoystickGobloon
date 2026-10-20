//! Linux backend: the input service reads hidden devices exclusively (gob-input's `exclusive`,
//! `EVIOCGRAB`), so games get nothing from them through evdev or joydev while Joystick Gobloon
//! runs. Programs that open the raw HID device (hidraw: Steam, SDL's HIDAPI drivers) still see
//! them; a udev rule closes that (docs/LINUX_PLAN.md, phase 4). No program list: the hiding
//! program is the only one that reads.
//!
//! The switch and the hidden devices are kept in `hidden.gob` (TOML) in the data directory;
//! hidden devices stay listed while unplugged.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gob_input::DeviceInfo;
use serde::{Deserialize, Serialize};

use crate::{Backend, Capabilities, Device, Error, Part};

pub const CAPABILITIES: Capabilities = Capabilities {
    name: "Linux evdev",
    app_list: false,
};
const STATE_FILE: &str = "hidden.gob";

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    if !gob_input::exclusive::supported() {
        return Err(Error::Unsupported);
    }
    let hide = Grab;
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
            device(&d.guid, &d.name, (d.vendor_id, d.product_id), true, detail)
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
        Ok(())
    }

    /// Every device here is a game controller, so `gaming_only` changes nothing.
    fn devices(&self, _gaming_only: bool) -> Result<Vec<Device>, Error> {
        Ok(list(&gob_input::exclusive::devices(), &read_state()?))
    }

    fn set_hidden(&self, device: &Device, hidden: bool) -> Result<(), Error> {
        let state = changed(read_state()?, device, hidden)?;
        write_state(&state)?;
        apply(&state);
        Ok(())
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
}
