//! Windows updates: the Inno Setup installer (installed copies) and the portable zip.

use std::path::Path;
use std::time::Duration;

/// Copies installed by the setup have its uninstaller next to the exe.
pub(super) fn installed_copy(exe_dir: &Path) -> bool {
    exe_dir.join("unins000.exe").exists()
}

pub(super) fn update_asset(installed: bool) -> &'static str {
    if installed {
        "-setup.exe"
    } else {
        "-portable.zip"
    }
}

/// Runs the new installer silently; it closes this app and starts the new one (`/UPDATE=1`).
/// ShellExecute "open": the installer's manifest asks for administrator rights (UAC).
pub(super) fn run_installer(setup: &Path) -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR};
    let params = "/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /UPDATE=1";
    let (file, params) = (HSTRING::from(setup.as_os_str()), HSTRING::from(params));
    // SAFETY: the strings outlive the call.
    let h = unsafe {
        ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &file,
            &params,
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success; 5 = access denied (UAC refused).
    if h.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!(
            "could not start the installer (code {})",
            h.0 as isize
        ))
    }
}

/// tar.exe (Windows 10 1803+) unpacks zip files; it runs without a console window.
pub(super) fn unpack_zip(zip: &Path, to: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let status = std::process::Command::new("tar")
        .creation_flags(CREATE_NO_WINDOW)
        .arg("-xf")
        .arg(zip)
        .arg("-C")
        .arg(to)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tar: {status}"))
    }
}

pub(super) fn wait_for_process(pid: u32, timeout: Duration) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    // SAFETY: the handle is closed after the wait.
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, timeout.as_millis().min(u128::from(u32::MAX)) as u32);
            let _ = CloseHandle(h);
        }
    }
}
