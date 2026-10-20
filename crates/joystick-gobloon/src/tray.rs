//! Notification area (tray) icon and the autostart entry.
//!
//! The tray callbacks show the window directly through the platform (`platform::show_window`)
//! and only then ask egui to repaint. "Activate" works without the window: it starts the engine through
//! [`Activation`] and updates the icon itself.
//!
//! Left click: show the window, or minimize it when it is on screen (to the tray with
//! "minimize to tray"). Double click (if on in the settings): activate / deactivate the
//! profile; a single click then waits for the double-click time.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::activation::Activation;
use crate::platform::{self, Window};

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
    let want = platform::tray_icon_size().max(16);
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
    /// A double click on the icon activates / deactivates the profile (setting).
    double_click_activates: Arc<AtomicBool>,
}

impl Tray {
    /// `hwnd` is the main window, `tooltip` the app name; `show`/`hiding`/`quit` are menu
    /// captions. `restored` is set whenever the window is shown from the tray (see
    /// [`show_window`]). "Activate" starts and stops the profile through `activation`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: &egui::Context,
        hwnd: Window,
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
        let hidhide = gob_hide::load().ok();
        let cloak_item = hidhide.as_ref().map(|hh| {
            let on = hh.enabled().unwrap_or(false);
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
        let double_click_activates = Arc::new(AtomicBool::new(true));
        let cloak_changed = Arc::new(AtomicBool::new(false));
        {
            let ctx = ctx.clone();
            let restored = restored.clone();
            let activation = activation.clone();
            // Every left click and double click bumps it: a pending single click only acts if
            // nothing followed it within the double-click time.
            let clicks = Arc::new(AtomicU64::new(0));
            let double_at: Arc<Mutex<Option<Instant>>> = Arc::default();
            let wait = platform::double_click_time();
            let activates = double_click_activates.clone();
            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // The release that ends a double click.
                    let last = *double_at.lock().unwrap_or_else(|e| e.into_inner());
                    if last.is_some_and(|t| t.elapsed() < wait) {
                        return;
                    }
                    if !activates.load(Ordering::Relaxed) {
                        // No double-click action: nothing to wait for.
                        single_click(hwnd, &restored, &ctx);
                        return;
                    }
                    let n = clicks.fetch_add(1, Ordering::Relaxed) + 1;
                    let (clicks, ctx, restored) = (clicks.clone(), ctx.clone(), restored.clone());
                    let _ =
                        std::thread::Builder::new()
                            .name("tray-click".into())
                            .spawn(move || {
                                std::thread::sleep(wait);
                                if clicks.load(Ordering::Relaxed) == n {
                                    single_click(hwnd, &restored, &ctx);
                                }
                            });
                }
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    clicks.fetch_add(1, Ordering::Relaxed);
                    *double_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
                    if activates.load(Ordering::Relaxed) {
                        toggle_activation(&activation);
                        ctx.request_repaint();
                    }
                }
                _ => {}
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
                    toggle_activation(&activation);
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
                            let result = hh.enabled().and_then(|on| hh.set_enabled(!on));
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
                    platform::show_window(hwnd);
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
            double_click_activates,
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

    /// Whether a double click on the icon activates / deactivates the profile.
    pub fn set_double_click_activates(&self, on: bool) {
        self.double_click_activates.store(on, Ordering::Relaxed);
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

/// Starts or stops the profile and updates the icon (UI thread: tray callbacks).
fn toggle_activation(activation: &Activation) {
    activation.toggle_from_tray();
    let active = activation.is_active();
    LIVE.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            l.show(active);
        }
    });
}

/// A left click on the icon that was not part of a double click: minimizes the window when
/// it is on screen (the app moves it on to the tray with "minimize to tray"), otherwise
/// shows it. Runs on a helper thread: only async window calls.
fn single_click(window: Window, restored: &AtomicBool, ctx: &egui::Context) {
    if platform::window_shown(window) {
        platform::minimize_window(window);
    } else {
        restored.store(true, Ordering::Relaxed);
        platform::show_window(window);
    }
    ctx.request_repaint();
}

/// Entry that starts Joystick Gobloon at logon (Windows: the registry's Run key).
pub mod autostart {
    /// Started at logon: start hidden in the tray (or minimized).
    pub const ARG: &str = "--minimized";

    /// Adds or removes the entry for the running exe.
    pub fn set(enabled: bool) -> Result<(), String> {
        crate::platform::set_autostart(enabled, ARG)
    }
}
