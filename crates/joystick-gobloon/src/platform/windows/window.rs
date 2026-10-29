//! The main window and the tray (Win32), and autostart at logon (the registry's Run key).

use std::time::Duration;

use windows::Win32::Foundation::HWND;

/// The main window's native handle; plain data, so tray callbacks and threads can hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Window(isize);

impl Window {
    fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

pub(super) fn main_window(cc: &eframe::CreationContext<'_>) -> Option<Window> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(Window(h.hwnd.get())),
        _ => None,
    }
}

/// Puts the main window into the tray: a transparent, click-through tool window (off the
/// taskbar and Alt+Tab), not minimized. It is not hidden (SW_HIDE): Windows does not paint
/// hidden windows, so eframe would never run a frame and its event loop would spin a CPU
/// core on every repaint request. Not minimized either: Windows draws a minimized tool
/// window as a small title bar above the taskbar, whatever its transparency.
pub(super) fn hide_to_tray(window: Window) {
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNOACTIVATE, ShowWindowAsync};
    let hwnd = window.hwnd();
    set_tray_style(hwnd, true);
    // SAFETY: the app's own top-level window, alive while the app runs. Async: callers may run
    // inside winit's message handling. Hiding first makes the taskbar notice the style change;
    // SW_SHOWNOACTIVATE also un-minimizes.
    unsafe {
        let _ = ShowWindowAsync(hwnd, SW_HIDE);
        let _ = ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
    }
}

/// Shows the main window again (from the tray, or minimized). The async variant only posts the
/// request, so winit is not re-entered when called from tray callbacks.
pub(super) fn show(window: Window) {
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, SW_HIDE, SW_RESTORE, SW_SHOW, ShowWindowAsync,
    };
    let hwnd = window.hwnd();
    let from_tray = in_tray(window);
    set_tray_style(hwnd, false);
    // SAFETY: the app's own top-level window, alive while the app runs.
    unsafe {
        if from_tray {
            // Hide first so the taskbar adds the button back.
            let _ = ShowWindowAsync(hwnd, SW_HIDE);
        }
        let _ = ShowWindowAsync(hwnd, SW_SHOW);
        if from_tray || IsIconic(hwnd).as_bool() {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        }
    }
}

pub(super) fn minimize(window: Window) {
    use windows::Win32::UI::WindowsAndMessaging::{SW_MINIMIZE, ShowWindowAsync};
    // SAFETY: the app's own window; the request is only posted to its thread.
    unsafe {
        let _ = ShowWindowAsync(window.hwnd(), SW_MINIMIZE);
    }
}

/// The main window is in the tray (see [`hide_to_tray`]).
pub(super) fn in_tray(window: Window) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TOOLWINDOW,
    };
    // SAFETY: reading the style of the app's own window.
    let style = unsafe { GetWindowLongPtrW(window.hwnd(), GWL_EXSTYLE) };
    style & WS_EX_TOOLWINDOW.0 as isize != 0
}

/// Visible and not minimized (the tray state is checked separately).
pub(super) fn visible(window: Window) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};
    let hwnd = window.hwnd();
    // SAFETY: the app's own top-level window, alive while the app runs.
    unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
}

/// Tray style: a tool window (off the taskbar and Alt+Tab), fully transparent and
/// click-through.
fn set_tray_style(hwnd: HWND, on: bool) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, LWA_ALPHA, SetLayeredWindowAttributes, SetWindowLongPtrW,
        WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    let tray = (WS_EX_TOOLWINDOW.0 | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0) as isize;
    // SAFETY: changing the extended style of the app's own window.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if on {
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                (style | tray) & !(WS_EX_APPWINDOW.0 as isize),
            );
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
        } else {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !tray);
        }
    }
}

/// The user's double-click time.
pub(super) fn double_click_time() -> Duration {
    // SAFETY: plain query without arguments.
    let ms = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() };
    Duration::from_millis(u64::from(ms))
}

/// Small icon size (16 px at 100 % scaling, 20 at 125 %, ...).
pub(super) fn small_icon_size() -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};
    // SAFETY: a plain metrics query.
    unsafe { GetSystemMetrics(SM_CXSMICON) }.max(0) as u32
}

/// The apps' light/dark setting (`AppsUseLightTheme`: 0 = dark); `None` when it is not set.
pub(super) fn dark_mode() -> Option<bool> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::w;
    let mut light = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: reads one DWORD into a local of that size.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut light).cast()),
            Some(&raw mut size),
        )
    };
    status.is_ok().then_some(light == 0)
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "JoystickGobloon";

/// Adds or removes the `HKCU\...\Run` entry that starts the running exe with `arg`.
pub(super) fn set_autostart(enabled: bool, arg: &str) -> Result<(), String> {
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
    };
    use windows::core::HSTRING;
    let (key, value) = (HSTRING::from(RUN_KEY), HSTRING::from(RUN_VALUE));
    // SAFETY: plain registry calls with valid, NUL-terminated strings.
    let status = unsafe {
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let command = format!("\"{}\" {arg}", exe.display());
            let data: Vec<u16> = command.encode_utf16().chain([0]).collect();
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                &value,
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * 2) as u32,
            )
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, &key, &value)
        }
    };
    // Deleting an entry that is not there is fine.
    if status.is_ok() || (!enabled && status.0 == 2) {
        Ok(())
    } else {
        Err(format!("registry error {}", status.0))
    }
}
