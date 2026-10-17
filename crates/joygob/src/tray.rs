//! Notification area (tray) icon and the Windows autostart entry.
//!
//! While the window is hidden eframe does not run `update`, so the tray callbacks show
//! the window directly through Win32 and only then ask egui to repaint.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

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
    icon: TrayIcon,
    /// Profile state the icon and tooltip show; `None` until the first update.
    active: Option<bool>,
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
    /// [`show_window`]).
    pub fn new(
        ctx: &egui::Context,
        hwnd: isize,
        restored: Arc<AtomicBool>,
        tooltip: &str,
        show: &str,
        hiding: &str,
        quit: &str,
    ) -> Result<Self, String> {
        let icon = tray_image(false)?;
        let show_item = MenuItem::new(show, true, None);
        let quit_item = MenuItem::new(quit, true, None);
        let menu = Menu::new();
        menu.append(&show_item).map_err(|e| e.to_string())?;
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
            let cloak_id = cloak_item.as_ref().map(|i| i.id().clone());
            let quit_flag = quit_flag.clone();
            let cloak_changed = cloak_changed.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
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
        Ok(Self {
            icon: tray,
            active: None,
            quit: quit_flag,
            cloak_item,
            cloak_changed,
        })
    }

    /// Green circle and `tooltip` while a profile runs, grey otherwise; no-op if unchanged.
    pub fn set_active(&mut self, active: bool, tooltip: &str) {
        if self.active == Some(active) {
            return;
        }
        self.active = Some(active);
        match tray_image(active) {
            Ok(icon) => {
                let _ = self.icon.set_icon(Some(icon));
            }
            Err(e) => log::error!("tray icon: {e}"),
        }
        let _ = self.icon.set_tooltip(Some(tooltip));
    }

    /// Check mark of the "Device hiding" item.
    pub fn set_cloaked(&self, on: bool) {
        if let Some(item) = &self.cloak_item {
            if item.is_checked() != on {
                item.set_checked(on);
            }
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

/// Shows the hidden main window. Called from tray callbacks, which run inside the window
/// message handling: the async variant only posts the request, so winit is not re-entered.
/// The app then syncs winit's state on its next frame (its `restored` flag); without
/// that winit still thinks the window is hidden and ignores the next hide request.
pub fn show_window(hwnd: isize) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            IsIconic, ShowWindowAsync, SW_RESTORE, SW_SHOW,
        };
        let hwnd = HWND(hwnd as *mut _);
        // SAFETY: `hwnd` is the app's own top-level window, alive while the app runs.
        unsafe {
            let _ = ShowWindowAsync(hwnd, SW_SHOW);
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Native handle of the main window, for the tray callbacks.
pub fn window_handle(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry starting JoyGob minimized.
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
            use windows::core::HSTRING;
            use windows::Win32::System::Registry::{
                RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
            };
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
