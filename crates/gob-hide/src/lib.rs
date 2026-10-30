//! Hiding physical devices from other programs (games then see only the virtual devices): the
//! common interface the GUI uses, whatever does the hiding. Each platform has its own backend:
//! HidHide on Windows (`windows.rs`, over gob-hidhide),
//! exclusive reading on Linux (`linux.rs`, over gob-input), `unsupported.rs` elsewhere.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(windows, target_os = "linux")))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[derive(Debug)]
pub enum Error {
    /// Nothing on this system hides devices (e.g. HidHide is not installed).
    NotInstalled,
    /// The backend reported an error.
    Failed(String),
    /// This platform cannot hide devices yet.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "device hiding is not installed"),
            Self::Failed(e) => write!(f, "device hiding: {e}"),
            Self::Unsupported => write!(f, "device hiding is not supported on this system"),
        }
    }
}

impl std::error::Error for Error {}

/// What a backend can do; the GUI adapts to it instead of checking the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Product name for the user, e.g. "HidHide"; empty when there is none to show.
    pub name: &'static str,
    /// Programs can be listed to see hidden devices (or, inverted, not to): the Applications
    /// page and "whitelist Joystick Gobloon itself".
    pub app_list: bool,
    /// Hidden devices are closed to every other program (no `Device::exposed` warnings).
    pub hides_everywhere: bool,
}

/// One part of a [`Device`] (HidHide: a HID collection), to match input devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub present: bool,
    pub vid_pid: Option<(u16, u16)>,
    pub product: String,
}

/// One physical device that can be hidden, with its current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub present: bool,
    pub hidden: bool,
    /// Hidden, yet other programs can still read it another way (Linux: its raw HID node is
    /// open to the user), so hiding cannot keep its input from them.
    pub exposed: bool,
    pub parts: Vec<Part>,
    /// Technical details for a tooltip (HidHide: instance paths).
    pub detail: String,
    /// What the backend needs to hide or show the device again.
    pub key: Vec<String>,
}

/// The device hiding of this system. Calls may block briefly (driver requests).
pub trait Backend: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    /// Version for the user, e.g. the driver version; may be empty.
    fn version(&self) -> String;
    /// Whether hiding is switched on as a whole.
    fn enabled(&self) -> Result<bool, Error>;
    fn set_enabled(&self, on: bool) -> Result<(), Error>;
    /// Devices known to the system (connected or not); only game controllers with
    /// `gaming_only`.
    fn devices(&self, gaming_only: bool) -> Result<Vec<Device>, Error>;
    fn set_hidden(&self, device: &Device, hidden: bool) -> Result<(), Error>;
    /// Listed programs (empty without [`Capabilities::app_list`]).
    fn apps(&self) -> Result<Vec<PathBuf>, Error>;
    /// Adds or removes a program; removing works for files that no longer exist too.
    fn set_app(&self, exe: &Path, listed: bool) -> Result<(), Error>;
    /// Whether the list is inverted: listed programs are the ones that do not see hidden
    /// devices.
    fn apps_inverted(&self) -> Result<bool, Error>;
    fn set_apps_inverted(&self, on: bool) -> Result<(), Error>;

    /// Adds `exe` unless it is listed already; returns whether it was added.
    fn ensure_app(&self, exe: &Path) -> Result<bool, Error> {
        if self.apps()?.iter().any(|a| same_path(a, exe)) {
            return Ok(false);
        }
        self.set_app(exe, true).map(|()| true)
    }

    /// Whether the state changed since the last call without a call here (a step finished in
    /// the background, e.g. after the administrator password): read it again.
    fn take_changed(&self) -> bool {
        false
    }
}

/// This platform's backend.
pub fn load() -> Result<Arc<dyn Backend>, Error> {
    #[cfg(windows)]
    return windows::load();
    #[cfg(target_os = "linux")]
    return linux::load();
    #[cfg(not(any(windows, target_os = "linux")))]
    return unsupported::load();
}

/// What this platform's backend can do, without loading it (e.g. for the settings page).
pub fn capabilities() -> Capabilities {
    #[cfg(windows)]
    return windows::CAPABILITIES;
    #[cfg(target_os = "linux")]
    return linux::CAPABILITIES;
    #[cfg(not(any(windows, target_os = "linux")))]
    return Capabilities {
        name: "",
        app_list: false,
        hides_everywhere: false,
    };
}

/// Before a system service that keeps the hidden devices is removed: copies them into this
/// program's own files, so hiding goes on without it. Nothing to do elsewhere.
pub fn keep_locally() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return linux::service::keep_locally();
    #[cfg(not(target_os = "linux"))]
    Ok(())
}

/// Linux: the USB sticks (vendor, product) whose raw HID node hiding without the service keeps
/// closed (a udev rule written as root), from this program's own hidden devices.
#[cfg(target_os = "linux")]
pub fn closed_sticks() -> Vec<(u16, u16)> {
    linux::closed_sticks_wanted()
}

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The program's data folder (next to its settings), where a backend that keeps its own files
/// (e.g. the hidden devices on Linux) stores them. Set once at start, before [`load`]; later
/// calls are ignored. Without it a backend uses the platform's usual place.
pub fn set_data_dir(dir: &Path) {
    let _ = DATA_DIR.set(dir.to_path_buf());
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn data_dir() -> Option<PathBuf> {
    DATA_DIR.get().cloned()
}

/// Where to get this platform's device hiding when it is missing; `None` when there is nothing to
/// install.
pub fn download_url() -> Option<&'static str> {
    #[cfg(windows)]
    return Some(windows::DOWNLOAD_URL);
    #[cfg(not(windows))]
    return None;
}

/// The connected device an input device belongs to: same VID/PID, and the same product name
/// when several devices share them. `None` when not found or still ambiguous.
pub fn find_device(devices: &[Device], vid_pid: (u16, u16), product: &str) -> Option<usize> {
    let matches: Vec<usize> = (0..devices.len())
        .filter(|&i| {
            let d = &devices[i];
            d.present
                && d.parts
                    .iter()
                    .any(|p| p.present && p.vid_pid == Some(vid_pid))
        })
        .collect();
    if matches.len() <= 1 {
        return matches.first().copied();
    }
    let product = product.trim();
    let named: Vec<usize> = matches
        .into_iter()
        .filter(|&i| {
            devices[i]
                .parts
                .iter()
                .any(|p| p.product.trim().eq_ignore_ascii_case(product))
        })
        .collect();
    (named.len() == 1).then(|| named[0])
}

/// Whether two paths name the same file, as this system compares them.
pub fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, present: bool, parts: &[((u16, u16), &str)]) -> Device {
        Device {
            name: name.into(),
            present,
            hidden: false,
            exposed: false,
            parts: parts
                .iter()
                .map(|&(vid_pid, product)| Part {
                    present,
                    vid_pid: Some(vid_pid),
                    product: product.into(),
                })
                .collect(),
            detail: String::new(),
            key: Vec::new(),
        }
    }

    #[test]
    fn finds_by_vid_pid_then_by_name() {
        let devices = [
            device(
                "EVO R",
                true,
                &[((0x231D, 0x0200), "VKBsim Gladiator EVO R")],
            ),
            device(
                "EVO L",
                true,
                &[((0x231D, 0x0200), "VKBsim Gladiator EVO L")],
            ),
            device("Old", false, &[((0x044F, 0xB10A), "T.16000M")]),
            device("Pad", true, &[((0x045E, 0x028E), "Controller")]),
        ];
        assert_eq!(find_device(&devices, (0x045E, 0x028E), "anything"), Some(3));
        assert_eq!(
            find_device(&devices, (0x231D, 0x0200), " VKBsim Gladiator EVO L "),
            Some(1)
        );
        assert_eq!(find_device(&devices, (0x231D, 0x0200), "Other"), None);
        // Disconnected devices never match.
        assert_eq!(find_device(&devices, (0x044F, 0xB10A), "T.16000M"), None);
    }
}
