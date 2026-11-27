//! Running programs for switching profiles by program: the one in the foreground, and every
//! one with a window (to pick from on the Applications page).

use std::path::PathBuf;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::{BOOL, PWSTR};

use crate::platform::Program;

/// The program file of process `pid` (also of elevated ones: limited query rights suffice).
fn process_path(pid: u32) -> Option<PathBuf> {
    // SAFETY: the handle is checked and closed; the buffer outlives the call and its length is
    // passed in.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let named = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        named.ok()?;
        Some(PathBuf::from(String::from_utf16_lossy(
            &buffer[..len as usize],
        )))
    }
}

fn window_program(window: HWND) -> Option<Program> {
    let mut pid = 0;
    // SAFETY: plain query of a window handle; `pid` is a valid out pointer.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    let path = process_path(pid)?;
    Some(Program::new(pid, path))
}

pub(super) fn foreground_program() -> Option<Program> {
    // SAFETY: no arguments; a null handle is checked.
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }
    window_program(window)
}

/// The window in front as an opaque number (its handle), `None` when there is none.
pub(super) fn front_window() -> Option<u64> {
    // SAFETY: no arguments; a null handle is checked.
    let window = unsafe { GetForegroundWindow() };
    (!window.is_invalid()).then_some(window.0 as u64)
}

/// Whether `window` belongs to this process.
pub(super) fn own_window(window: u64) -> bool {
    let mut pid = 0;
    // SAFETY: plain query of a window handle (a stale one gives pid 0); `pid` is a valid out
    // pointer.
    unsafe { GetWindowThreadProcessId(HWND(window as *mut _), Some(&mut pid)) };
    pid == std::process::id()
}

/// Brings `window` to the front, restoring it when minimized; false when it is gone or Windows
/// kept the window in front where it is.
pub(super) fn bring_to_front(window: u64) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, IsWindow, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    let hwnd = HWND(window as *mut _);
    // SAFETY: plain window calls on a handle checked with IsWindow first; SendInput gets one
    // initialized INPUT and its size.
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return false;
        }
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        // Windows lets the process that sent the last input change the window in front: a
        // move by nothing makes that this one.
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: MOUSEEVENTF_MOVE,
                    ..Default::default()
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        let _ = SetForegroundWindow(hwnd);
        GetForegroundWindow() == hwnd
    }
}

pub(super) fn running_programs() -> Vec<Program> {
    unsafe extern "system" fn add(window: HWND, list: LPARAM) -> BOOL {
        // SAFETY: `list` is the Vec passed to EnumWindows below, alive during the call.
        let list = unsafe { &mut *(list.0 as *mut Vec<HWND>) };
        // Top-level windows a user can see and switch to: visible, titled, not owned by another.
        // SAFETY: plain queries of the window EnumWindows hands in.
        let shown = unsafe {
            IsWindowVisible(window).as_bool()
                && GetWindowTextLengthW(window) > 0
                && GetWindow(window, GW_OWNER).is_err()
        };
        if shown {
            list.push(window);
        }
        true.into()
    }
    let mut windows: Vec<HWND> = Vec::new();
    // SAFETY: the callback only touches `windows`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(add), LPARAM(&mut windows as *mut _ as isize)) };
    let mut programs: Vec<Program> = Vec::new();
    for window in windows {
        if let Some(program) = window_program(window)
            && !programs.iter().any(|p| p.pid == program.pid)
        {
            programs.push(program);
        }
    }
    programs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_has_its_program_file() {
        let path = process_path(std::process::id()).unwrap();
        assert_eq!(path, std::env::current_exe().unwrap());
    }

    /// By hand: `cargo test -p joystick-gobloon programs -- --ignored --nocapture` prints the
    /// program in front and the programs with windows.
    #[test]
    #[ignore]
    fn prints_the_running_programs() {
        println!("front: {:?}", foreground_program());
        for p in running_programs() {
            println!("{} {} {}", p.pid, p.name, p.path.display());
        }
    }
}
