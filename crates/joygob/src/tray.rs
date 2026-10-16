//! Notification area (tray) icon and the Windows autostart entry.
//!
//! While the window is hidden eframe does not run `update`, so the tray callbacks show
//! the window directly through Win32 and only then ask egui to repaint.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub struct Tray {
    _icon: TrayIcon,
    /// "Quit" was chosen in the tray menu; the app closes (asking to save first).
    pub quit: Arc<AtomicBool>,
}

impl Tray {
    /// `hwnd` is the main window, `tooltip` the app name; `show`/`quit` are menu captions.
    pub fn new(
        ctx: &egui::Context,
        hwnd: isize,
        tooltip: &str,
        show: &str,
        quit: &str,
    ) -> Result<Self, String> {
        let png = eframe::icon_data::from_png_bytes(include_bytes!(
            "../../../assets/icon/joygob-256.png"
        ))
        .map_err(|e| e.to_string())?;
        let icon = tray_icon::Icon::from_rgba(png.rgba, png.width, png.height)
            .map_err(|e| e.to_string())?;
        let show_item = MenuItem::new(show, true, None);
        let quit_item = MenuItem::new(quit, true, None);
        let menu = Menu::new();
        menu.append(&show_item).map_err(|e| e.to_string())?;
        menu.append(&quit_item).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_tooltip(tooltip)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| e.to_string())?;

        let quit_flag = Arc::new(AtomicBool::new(false));
        {
            let ctx = ctx.clone();
            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    show_window(hwnd);
                    ctx.request_repaint();
                }
            }));
        }
        {
            let ctx = ctx.clone();
            let (show_id, quit_id) = (show_item.id().clone(), quit_item.id().clone());
            let quit_flag = quit_flag.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if event.id == quit_id {
                    quit_flag.store(true, Ordering::Relaxed);
                }
                if event.id == show_id || event.id == quit_id {
                    show_window(hwnd);
                    ctx.request_repaint();
                }
            }));
        }
        Ok(Self {
            _icon: tray,
            quit: quit_flag,
        })
    }

    pub fn take_quit(&self) -> bool {
        self.quit.swap(false, Ordering::Relaxed)
    }
}

/// Shows and focuses the main window.
pub fn show_window(hwnd: isize) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
        };
        let hwnd = HWND(hwnd as *mut _);
        // SAFETY: `hwnd` is the app's own top-level window, alive while the app runs.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
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
