//! Installing, updating and removing the Linux service with administrator rights (`pkexec`, the
//! system asks for the password): the binary goes to [`BIN_PATH`] (`/var` stays writable on
//! systems with a read-only `/usr`), the systemd unit to [`UNIT_PATH`]. Joystick Gobloon carries
//! the binary next to its own (in the AppImage too).

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::linux::{Client, SOCKET_PATH};

pub const BIN_PATH: &str = "/var/lib/system-gobloon/bin/SystemGobloon";
pub const UNIT_PATH: &str = "/etc/systemd/system/system-gobloon.service";
/// The service's file name next to Joystick Gobloon's.
pub const BIN_NAME: &str = "SystemGobloon";

/// The systemd unit. Stopping or a crash runs `--cleanup`: hidden devices become visible again.
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

/// Copies the binary (`$1`), labels it a program for SELinux where there is one, writes the
/// unit (`$2`), enables the service and (re)starts it.
const INSTALL_SCRIPT: &str = "set -e
install -D -m 0755 \"$1\" /var/lib/system-gobloon/bin/SystemGobloon
if command -v chcon >/dev/null 2>&1; then
    chcon -t bin_t /var/lib/system-gobloon/bin/SystemGobloon 2>/dev/null || true
fi
printf '%s' \"$2\" > /etc/systemd/system/system-gobloon.service
systemctl daemon-reload
systemctl enable system-gobloon.service
systemctl restart system-gobloon.service";

/// Stops and disables the service (its `--cleanup` shows hidden devices again), deletes the unit
/// and the binary. The state (hidden devices, layouts) stays for a later install.
const REMOVE_SCRIPT: &str = "systemctl disable --now system-gobloon.service
rm -f /etc/systemd/system/system-gobloon.service
systemctl daemon-reload
rm -rf /var/lib/system-gobloon/bin
rm -f /run/udev/rules.d/70-system-gobloon-access.rules
udevadm control --reload
udevadm trigger --action=change --subsystem-match=input
true";

/// What the system has of the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// No systemd: the service cannot run here.
    Unsupported,
    NotInstalled,
    /// Installed but not answering (stopped, failed, or still starting).
    Stopped,
    Running {
        version: String,
    },
}

/// Asks the running service for its version; `client_version` goes to its log.
pub fn status(client_version: &str) -> Status {
    if let Ok(mut client) = Client::connect()
        && let Ok(hello) = client.hello(client_version)
    {
        return Status::Running {
            version: hello.service_version.as_str().to_string(),
        };
    }
    if Path::new(UNIT_PATH).exists() {
        Status::Stopped
    } else if !Path::new("/run/systemd/system").exists() {
        Status::Unsupported
    } else {
        Status::NotInstalled
    }
}

/// The service binary next to the running program.
pub fn bundled_binary() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?.parent()?.join(BIN_NAME);
    path.is_file().then_some(path)
}

/// Installs or updates the service from `binary` and waits until it answers. Blocking until the
/// user answered the password dialog: call it off the GUI thread.
pub fn install(binary: &Path) -> Result<(), String> {
    // Root cannot read the AppImage's FUSE mount: a copy in the temporary folder can be read.
    let dir = private_dir().map_err(|e| format!("temporary folder: {e}"))?;
    let copy = dir.join(BIN_NAME);
    let result = std::fs::copy(binary, &copy)
        .map_err(|e| format!("{}: {e}", binary.display()))
        .and_then(|_| pkexec(&[INSTALL_SCRIPT, "sh", &copy.to_string_lossy(), UNIT]));
    let _ = std::fs::remove_dir_all(&dir);
    result?;
    let end = Instant::now() + Duration::from_secs(10);
    while Client::connect().is_err() {
        if Instant::now() > end {
            return Err(format!("the service does not answer on {SOCKET_PATH}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Stops and removes the service, like [`install`].
pub fn remove() -> Result<(), String> {
    pkexec(&[REMOVE_SCRIPT, "sh"])
}

/// A new folder only this user may open.
fn private_dir() -> io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::env::temp_dir().join(format!("system-gobloon-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
    Ok(dir)
}

/// Runs `/bin/sh -c <args>` as root through pkexec.
fn pkexec(args: &[&str]) -> Result<(), String> {
    let output = Command::new("pkexec")
        .args(["/bin/sh", "-c"])
        .args(args)
        .output()
        .map_err(|e| format!("pkexec: {e}"))?;
    match output.status.code() {
        Some(0) => Ok(()),
        // pkexec: the dialog was dismissed (126) or the password was not accepted (127).
        Some(126 | 127) => Err("not authorized".into()),
        _ => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_runs_the_installed_binary() {
        assert!(UNIT.contains(&format!("ExecStart={BIN_PATH}\n")));
        assert!(UNIT.contains(&format!("ExecStopPost={BIN_PATH} --cleanup\n")));
        assert!(INSTALL_SCRIPT.contains(BIN_PATH) && INSTALL_SCRIPT.contains(UNIT_PATH));
        assert!(REMOVE_SCRIPT.contains(UNIT_PATH));
        assert!(BIN_PATH.ends_with(BIN_NAME));
    }

    #[test]
    fn private_dir_is_closed_to_others() {
        use std::os::unix::fs::PermissionsExt;
        let dir = private_dir().unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
