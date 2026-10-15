//! HidHide control through the official `HidHideCLI.exe`.
//!
//! The CLI ships with the HidHide driver and converts application paths to
//! the form the driver expects, so JoyGob does not talk to the driver directly.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

#[derive(Debug)]
pub enum Error {
    /// HidHide is not installed.
    NotInstalled,
    /// The CLI could not be started or reported an error.
    Cli(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "HidHide is not installed"),
            Self::Cli(e) => write!(f, "HidHideCLI: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// A HID collection as reported by `--dev-gaming`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HidDevice {
    pub present: bool,
    pub gaming_device: bool,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub product: String,
    #[serde(default)]
    pub usage: String,
    pub device_instance_path: String,
    /// Parent USB device; HidHide's own client hides it together with the HID collection.
    #[serde(default)]
    pub base_container_device_instance_path: String,
}

impl HidDevice {
    pub fn name(&self) -> &str {
        let product = self.product.trim();
        if product.is_empty() {
            &self.device_instance_path
        } else {
            product
        }
    }

    /// `(vendor id, product id)` parsed from the instance path, e.g. `HID\VID_231D&PID_0200\...`.
    pub fn vid_pid(&self) -> Option<(u16, u16)> {
        vid_pid(&self.device_instance_path)
    }

    /// Instance paths to hide or unhide for this device.
    pub fn instance_paths(&self) -> Vec<&str> {
        let mut paths = vec![self.device_instance_path.as_str()];
        if !self.base_container_device_instance_path.is_empty() {
            paths.push(&self.base_container_device_instance_path);
        }
        paths
    }
}

#[derive(Debug, Deserialize)]
struct Group {
    devices: Vec<HidDevice>,
}

pub struct HidHide {
    cli: PathBuf,
    pub version: String,
}

impl HidHide {
    /// Locates HidHideCLI.exe through HidHide's registry key.
    pub fn find() -> Result<Self, Error> {
        let (dir, version) = install_info().ok_or(Error::NotInstalled)?;
        let cli = dir.join("x64").join("HidHideCLI.exe");
        if !cli.exists() {
            return Err(Error::NotInstalled);
        }
        Ok(Self { cli, version })
    }

    fn run(&self, args: &[&str]) -> Result<String, Error> {
        let mut cmd = Command::new(&self.cli);
        cmd.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let out = cmd.output().map_err(|e| Error::Cli(e.to_string()))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(Error::Cli(if err.is_empty() {
                format!("exit {}", out.status)
            } else {
                err
            }));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Whether hiding is active.
    pub fn cloaked(&self) -> Result<bool, Error> {
        Ok(self.run(&["--cloak-state"])?.contains("--cloak-on"))
    }

    pub fn set_cloaked(&self, on: bool) -> Result<(), Error> {
        self.run(&[if on { "--cloak-on" } else { "--cloak-off" }])
            .map(drop)
    }

    /// Instance paths of hidden devices.
    pub fn hidden(&self) -> Result<Vec<String>, Error> {
        Ok(quoted_args(&self.run(&["--dev-list"])?, "--dev-hide"))
    }

    pub fn set_hidden(&self, instance_path: &str, hidden: bool) -> Result<(), Error> {
        self.run(&[
            if hidden { "--dev-hide" } else { "--dev-unhide" },
            instance_path,
        ])
        .map(drop)
    }

    /// Hides or shows a device together with its USB container.
    pub fn set_device_hidden(&self, device: &HidDevice, hidden: bool) -> Result<(), Error> {
        device
            .instance_paths()
            .into_iter()
            .try_for_each(|p| self.set_hidden(p, hidden))
    }

    /// Applications allowed to see hidden devices.
    pub fn apps(&self) -> Result<Vec<PathBuf>, Error> {
        Ok(quoted_args(&self.run(&["--app-list"])?, "--app-reg")
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    pub fn set_app(&self, exe: &Path, allowed: bool) -> Result<(), Error> {
        let path = exe.to_string_lossy();
        self.run(&[if allowed { "--app-reg" } else { "--app-unreg" }, &path])
            .map(drop)
    }

    /// Adds `exe` to the whitelist unless it is there already; returns whether it was added.
    pub fn ensure_app(&self, exe: &Path) -> Result<bool, Error> {
        if self.apps()?.iter().any(|a| same_path(a, exe)) {
            return Ok(false);
        }
        self.set_app(exe, true).map(|()| true)
    }

    /// Game controllers known to Windows (present or not).
    pub fn gaming_devices(&self) -> Result<Vec<HidDevice>, Error> {
        parse_gaming(&self.run(&["--dev-gaming"])?)
    }
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

fn parse_gaming(json: &str) -> Result<Vec<HidDevice>, Error> {
    let groups: Vec<Group> = serde_json::from_str(json).map_err(|e| Error::Cli(e.to_string()))?;
    Ok(groups.into_iter().flat_map(|g| g.devices).collect())
}

/// Values of `flag "value"` lines as printed by the CLI.
fn quoted_args(output: &str, flag: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|l| l.trim().strip_prefix(flag))
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn vid_pid(instance_path: &str) -> Option<(u16, u16)> {
    let upper = instance_path.to_ascii_uppercase();
    let hex = |tag: &str| {
        let start = upper.find(tag)? + tag.len();
        u16::from_str_radix(upper.get(start..start + 4)?, 16).ok()
    };
    Some((hex("VID_")?, hex("PID_")?))
}

#[cfg(windows)]
fn install_info() -> Option<(PathBuf, String)> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    let read = |name: PCWSTR| {
        let mut buf = [0u16; 512];
        let mut len = (buf.len() * 2) as u32;
        unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!(r"SOFTWARE\Nefarius Software Solutions e.U.\HidHide"),
                name,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut len),
            )
        }
        .ok()
        .ok()?;
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    };
    Some((
        PathBuf::from(read(w!("Path"))?),
        read(w!("Version")).unwrap_or_default(),
    ))
}

#[cfg(not(windows))]
fn install_info() -> Option<(PathBuf, String)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cli_lists() {
        let out = "--dev-hide \"HID\\VID_231D&PID_0200\\a&2f28a505&1&0000\"\r\n--dev-hide \"USB\\VID_231D&PID_0200\\9&7B3A559&0&2\"\r\n";
        assert_eq!(
            quoted_args(out, "--dev-hide"),
            [
                "HID\\VID_231D&PID_0200\\a&2f28a505&1&0000",
                "USB\\VID_231D&PID_0200\\9&7B3A559&0&2"
            ]
        );
        assert!(quoted_args("--cloak-on", "--dev-hide").is_empty());
    }

    #[test]
    fn parses_gaming_json() {
        let json = r#" [ { "friendlyName" : "VKB" , "devices" : [
            { "present" : true , "gamingDevice" : true , "symbolicLink" : "x" ,
              "vendor" : "VKB-Sim" , "product" : " VKBsim Gladiator EVO R  " , "serialNumber" : "" ,
              "usage" : "Joystick" , "description" : "HID-compliant game controller" ,
              "deviceInstancePath" : "HID\\VID_231D&PID_0200\\a&2f28a505&1&0000" ,
              "baseContainerDeviceInstancePath" : "USB\\VID_231D&PID_0200\\9&7B3A559&0&2" ,
              "baseContainerClassGuid" : "{00000000-0000-0000-0000-000000000000}" ,
              "baseContainerDeviceCount" : 1 } ] } ] "#;
        let devices = parse_gaming(json).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name(), "VKBsim Gladiator EVO R");
        assert_eq!(devices[0].vid_pid(), Some((0x231D, 0x0200)));
        assert_eq!(devices[0].instance_paths().len(), 2);
    }
}
