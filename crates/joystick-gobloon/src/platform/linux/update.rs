//! Updates on Linux: a copy started from an AppImage replaces that file with the new release's
//! AppImage and starts it; other copies (a build, a distribution's package) do not update.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The AppImage file this copy runs from (set by the AppImage runtime).
fn appimage() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .filter(|a| !a.is_empty())
        .map(PathBuf::from)
}

/// "Installed" here means: started from an AppImage, which updates as a whole file.
pub(super) fn installed_copy() -> bool {
    appimage().is_some()
}

/// Puts `new` in place of the running AppImage and starts it; it waits for this copy to exit.
/// The running file can be replaced: the kernel keeps the old one until it is closed.
pub(super) fn replace_appimage(new: &Path) -> Result<(), String> {
    let target = appimage().ok_or("not running from an AppImage")?;
    // Copied next to the target first, so the rename stays on one file system.
    let next = target.with_file_name(format!(
        ".{}.new",
        target.file_name().unwrap_or_default().to_string_lossy()
    ));
    let failed = |e: std::io::Error| format!("{}: {e}", next.display());
    std::fs::copy(new, &next).map_err(failed)?;
    std::fs::set_permissions(&next, std::fs::Permissions::from_mode(0o755)).map_err(failed)?;
    std::fs::rename(&next, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    log::info!("update: {} replaced, restarting", target.display());
    std::process::Command::new(&target)
        .arg(crate::update::WAIT_PID)
        .arg(std::process::id().to_string())
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// Waits until process `pid` is gone (no `/proc/<pid>`), at most `timeout`.
pub(super) fn wait_for_process(pid: u32, timeout: Duration) {
    let proc = PathBuf::from(format!("/proc/{pid}"));
    let end = Instant::now() + timeout;
    while proc.exists() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_a_process_to_exit() {
        let mut child = std::process::Command::new("sleep")
            .arg("0.3")
            .spawn()
            .unwrap();
        let started = Instant::now();
        let pid = child.id();
        std::thread::spawn(move || child.wait());
        wait_for_process(pid, Duration::from_secs(5));
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(250), "{waited:?}");
        assert!(waited < Duration::from_secs(4), "{waited:?}");
    }
}
