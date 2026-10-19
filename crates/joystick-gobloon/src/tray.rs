//! Notification area (tray) icon and the Windows autostart entry.
//!
//! While the window is hidden eframe does not run `update`, so the tray callbacks show
//! the window directly through Win32 and only then ask egui to repaint. "Activate" works
//! without the window: it starts the engine through [`Activation`] and updates the icon itself.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::activation::Activation;

/// What shows the profile state: the icon, its tooltip and the "Activate" check mark. Kept on
/// the UI thread, where the menu callbacks run (muda handles the menu in the tray's window).
struct Live {
    icon: TrayIcon,
    activate: CheckMenuItem,
    /// Tooltips for inactive and active.
    tooltips: [String; 2],
    shown: Option<bool>,
}

impl Live {
    fn show(&mut self, active: bool) {
        if self.shown == Some(active) {
            return;
        }
        self.shown = Some(active);
        match tray_image(active) {
            Ok(icon) => {
                let _ = self.icon.set_icon(Some(icon));
            }
            Err(e) => log::error!("tray icon: {e}"),
        }
        let _ = self
            .icon
            .set_tooltip(Some(&self.tooltips[usize::from(active)]));
        self.activate.set_checked(active);
    }
}

thread_local! {
    static LIVE: RefCell<Option<Live>> = const { RefCell::new(None) };
}

/// Tray images (assets/icon/tray-*.svg): the logo's joystick on a green circle while a
/// profile is active, on a light grey one otherwise; one per Windows tray size.
const ICONS: [(u32, &[u8], &[u8]); 4] = [
    (
        16,
        include_bytes!("../../../assets/icon/tray/active-16.png"),
        include_bytes!("../../../assets/icon/tray/idle-16.png"),
    ),
    (
        20,
        include_bytes!("../../../assets/icon/tray/active-20.png"),
        include_bytes!("../../../assets/icon/tray/idle-20.png"),
    ),
    (
        24,
        include_bytes!("../../../assets/icon/tray/active-24.png"),
        include_bytes!("../../../assets/icon/tray/idle-24.png"),
    ),
    (
        32,
        include_bytes!("../../../assets/icon/tray/active-32.png"),
        include_bytes!("../../../assets/icon/tray/idle-32.png"),
    ),
];

/// The tray image for the current display scaling (16 px at 100 %, 20 at 125 %, ...).
fn tray_image(active: bool) -> Result<tray_icon::Icon, String> {
    #[cfg(windows)]
    // SAFETY: a plain metrics query.
    let want = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
            windows::Win32::UI::WindowsAndMessaging::SM_CXSMICON,
        )
    }
    .max(16) as u32;
    #[cfg(not(windows))]
    let want = 16;
    let (_, on, off) = ICONS
        .iter()
        .rev()
        .find(|(size, ..)| *size <= want)
        .unwrap_or(&ICONS[0]);
    let png = eframe::icon_data::from_png_bytes(if active { on } else { off })
        .map_err(|e| e.to_string())?;
    tray_icon::Icon::from_rgba(png.rgba, png.width, png.height).map_err(|e| e.to_string())
}

pub struct Tray {
    /// "Quit" was chosen in the tray menu; the app closes (asking to save first).
    pub quit: Arc<AtomicBool>,
    /// "Device hiding" item, when HidHide is installed.
    cloak_item: Option<CheckMenuItem>,
    /// The tray menu switched device hiding; the app reloads HidHide's state.
    cloak_changed: Arc<AtomicBool>,
}

impl Tray {
    /// `hwnd` is the main window, `tooltip` the app name; `show`/`hiding`/`quit` are menu
    /// captions. `restored` is set whenever the window is shown from the tray (see
    /// [`show_window`]). "Activate" starts and stops the profile through `activation`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: &egui::Context,
        hwnd: isize,
        restored: Arc<AtomicBool>,
        activation: Activation,
        tooltip: &str,
        show: &str,
        hiding: &str,
        quit: &str,
    ) -> Result<Self, String> {
        let icon = tray_image(false)?;
        let show_item = MenuItem::new(show, true, None);
        // Caption and enabled state follow the loaded profile (`set_profile`).
        let activate_item = CheckMenuItem::new("", false, false, None);
        let quit_item = MenuItem::new(quit, true, None);
        let menu = Menu::new();
        menu.append(&show_item).map_err(|e| e.to_string())?;
        menu.append(&activate_item).map_err(|e| e.to_string())?;
        // Device hiding on/off without opening the window (HidHide installed only).
        let hidhide = gob_hidhide::HidHide::find().ok().map(Arc::new);
        let cloak_item = hidhide.as_ref().map(|hh| {
            let on = hh.cloaked().unwrap_or(false);
            CheckMenuItem::new(hiding, true, on, None)
        });
        if let Some(item) = &cloak_item {
            menu.append(item).map_err(|e| e.to_string())?;
        }
        menu.append(&quit_item).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_tooltip(tooltip)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| e.to_string())?;

        let quit_flag = Arc::new(AtomicBool::new(false));
        let cloak_changed = Arc::new(AtomicBool::new(false));
        {
            let ctx = ctx.clone();
            let restored = restored.clone();
            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    restored.store(true, Ordering::Relaxed);
                    show_window(hwnd);
                    ctx.request_repaint();
                }
            }));
        }
        {
            let ctx = ctx.clone();
            let (show_id, quit_id) = (show_item.id().clone(), quit_item.id().clone());
            let activate_id = activate_item.id().clone();
            let cloak_id = cloak_item.as_ref().map(|i| i.id().clone());
            let quit_flag = quit_flag.clone();
            let cloak_changed = cloak_changed.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if event.id == activate_id {
                    activation.toggle_from_tray();
                    let active = activation.is_active();
                    LIVE.with(|l| {
                        if let Some(l) = l.borrow_mut().as_mut() {
                            l.show(active);
                        }
                    });
                    ctx.request_repaint();
                }
                if event.id == quit_id {
                    quit_flag.store(true, Ordering::Relaxed);
                }
                if Some(&event.id) == cloak_id.as_ref() {
                    // HidHide runs a process: off the UI thread. The app syncs the check mark.
                    if let Some(hh) = hidhide.clone() {
                        let (ctx, changed) = (ctx.clone(), cloak_changed.clone());
                        std::thread::spawn(move || {
                            let result = hh.cloaked().and_then(|on| hh.set_cloaked(!on));
                            if let Err(e) = result {
                                log::error!("tray: switching device hiding: {e}");
                            }
                            changed.store(true, Ordering::Relaxed);
                            ctx.request_repaint();
                        });
                    }
                }
                if event.id == show_id || event.id == quit_id {
                    restored.store(true, Ordering::Relaxed);
                    show_window(hwnd);
                    ctx.request_repaint();
                }
            }));
        }
        LIVE.with(|l| {
            *l.borrow_mut() = Some(Live {
                icon: tray,
                activate: activate_item,
                tooltips: [tooltip.to_string(), tooltip.to_string()],
                shown: None,
            });
        });
        Ok(Self {
            quit: quit_flag,
            cloak_item,
            cloak_changed,
        })
    }

    /// Green circle while a profile runs, grey otherwise, with the tooltip for each state
    /// (`[inactive, active]`); no-op if unchanged.
    pub fn set_active(&mut self, active: bool, tooltips: [String; 2]) {
        LIVE.with(|l| {
            if let Some(l) = l.borrow_mut().as_mut() {
                if l.tooltips != tooltips {
                    l.tooltips = tooltips;
                    l.shown = None;
                }
                l.show(active);
            }
        });
    }

    /// Caption of "Activate" (it names the profile) and whether a profile is loaded.
    pub fn set_profile(&self, caption: &str, loaded: bool) {
        LIVE.with(|l| {
            if let Some(l) = l.borrow().as_ref() {
                if l.activate.text() != caption {
                    l.activate.set_text(caption);
                }
                if l.activate.is_enabled() != loaded {
                    l.activate.set_enabled(loaded);
                }
            }
        });
    }

    /// Check mark of the "Device hiding" item.
    pub fn set_cloaked(&self, on: bool) {
        if let Some(item) = &self.cloak_item
            && item.is_checked() != on
        {
            item.set_checked(on);
        }
    }

    /// Device hiding was switched from the tray menu since the last call.
    pub fn take_cloak_changed(&self) -> bool {
        self.cloak_changed.swap(false, Ordering::Relaxed)
    }

    pub fn take_quit(&self) -> bool {
        self.quit.swap(false, Ordering::Relaxed)
    }
}

impl Drop for Tray {
    /// The icon lives in [`LIVE`]; dropping it there removes it from the notification area.
    fn drop(&mut self) {
        LIVE.with(|l| l.borrow_mut().take());
    }
}

/// Puts the main window into the tray: a transparent, click-through tool window (off the
/// taskbar and Alt+Tab), not minimized. It is not hidden (SW_HIDE): Windows does not paint
/// hidden windows, so eframe would never run a frame and its event loop would spin a CPU
/// core on every repaint request. Not minimized either: Windows draws a minimized tool
/// window as a small title bar above the taskbar, whatever its transparency.
pub fn hide_window(hwnd: isize) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SW_HIDE, SW_SHOWNOACTIVATE, ShowWindowAsync,
        };
        let window = handle(hwnd);
        set_tray_style(window, true);
        // SAFETY: `hwnd` is the app's own top-level window, alive while the app runs.
        // Async: callers may run inside winit's message handling. Hiding first makes the
        // taskbar notice the style change; SW_SHOWNOACTIVATE also un-minimizes.
        unsafe {
            let _ = ShowWindowAsync(window, SW_HIDE);
            let _ = ShowWindowAsync(window, SW_SHOWNOACTIVATE);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Shows the main window again (from the tray, or minimized). Called from tray callbacks,
/// which run inside the window message handling: the async variant only posts the request,
/// so winit is not re-entered. The app then syncs winit's state on its next frame (its
/// `restored` flag).
pub fn show_window(hwnd: isize) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            IsIconic, SW_HIDE, SW_RESTORE, SW_SHOW, ShowWindowAsync,
        };
        let hwnd = handle(hwnd);
        let from_tray = in_tray_handle(hwnd);
        set_tray_style(hwnd, false);
        // SAFETY: `hwnd` is the app's own top-level window, alive while the app runs.
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
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// The main window is in the tray (see [`hide_window`]).
pub fn in_tray(hwnd: isize) -> bool {
    #[cfg(windows)]
    return in_tray_handle(handle(hwnd));
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        false
    }
}

/// The main window is on screen: visible, not minimized, not in the tray.
pub fn window_shown(hwnd: isize) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};
        let hwnd = handle(hwnd);
        // SAFETY: `hwnd` is the app's own top-level window, alive while the app runs.
        unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        true
    }
}

#[cfg(windows)]
fn handle(hwnd: isize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(hwnd as *mut _)
}

#[cfg(windows)]
fn in_tray_handle(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TOOLWINDOW,
    };
    // SAFETY: reading the style of the app's own window.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    style & WS_EX_TOOLWINDOW.0 as isize != 0
}

/// Tray style: a tool window (off the taskbar and Alt+Tab), fully transparent and
/// click-through.
#[cfg(windows)]
fn set_tray_style(hwnd: windows::Win32::Foundation::HWND, on: bool) {
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

/// Native handle of the main window, for the tray callbacks.
pub fn window_handle(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry starting Joystick Gobloon minimized.
pub mod autostart {
    /// Started by Windows at logon: start hidden in the tray (or minimized).
    pub const ARG: &str = "--minimized";
    #[cfg(windows)]
    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    #[cfg(windows)]
    const VALUE: &str = "JoystickGobloon";

    /// Adds or removes the entry for the running exe.
    pub fn set(enabled: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Registry::{
                HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
            };
            use windows::core::HSTRING;
            let (key, value) = (HSTRING::from(RUN_KEY), HSTRING::from(VALUE));
            // SAFETY: plain registry calls with valid, NUL-terminated strings.
            let status = unsafe {
                if enabled {
                    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                    let command = format!("\"{}\" {ARG}", exe.display());
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
        #[cfg(not(windows))]
        {
            let _ = enabled;
            Ok(())
        }
    }
}
