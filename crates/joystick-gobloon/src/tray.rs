//! Notification area (tray) icon and the autostart entry.
//!
//! The tray callbacks show the window directly through the platform (`platform::show_window`)
//! and only then ask egui to repaint. "Activate" works without the window: it starts the engine through
//! [`Activation`] and updates the icon itself.
//!
//! Left click: show the window, or minimize it when it is on screen (to the tray with
//! "minimize to tray"). Double click (if on in the settings): activate / deactivate the
//! profile; a single click then waits for the double-click time. Where the tray reports no
//! clicks (Linux's AppIndicator) the menu does all of it.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::activation::Activation;
use crate::platform::{self, Window};

/// What shows the profile state: the icon, its tooltip and the check marks. Kept on the tray
/// thread (`platform::on_tray_thread`: Windows' UI thread, where muda runs the menu in the
/// tray's window; Linux's GTK thread), where the menu callbacks run too.
struct Live {
    icon: TrayIcon,
    activate: CheckMenuItem,
    /// "Device hiding", where hiding is available.
    cloak: Option<CheckMenuItem>,
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
    /// The tray menu switched device hiding; the app reloads HidHide's state.
    cloak_changed: Arc<AtomicBool>,
    /// A double click on the icon activates / deactivates the profile (setting).
    double_click_activates: Arc<AtomicBool>,
}

/// Runs `f` with the tray's state on the tray thread, if the icon is there.
fn with_live(f: impl FnOnce(&mut Live) + Send + 'static) {
    platform::on_tray_thread(Box::new(move || {
        LIVE.with(|l| {
            if let Some(l) = l.borrow_mut().as_mut() {
                f(l);
            }
        });
    }));
}

impl Tray {
    /// `hwnd` is the main window, `tooltip` the app name; `show`/`hiding`/`quit` are menu
    /// captions. `restored` is set whenever the window is shown from the tray (see
    /// [`show_window`]). "Activate" starts and stops the profile through `activation`.
    /// The icon is made on the tray thread; where that is another thread (Linux), an error
    /// there is only logged.
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
        let quit_flag = Arc::new(AtomicBool::new(false));
        let double_click_activates = Arc::new(AtomicBool::new(true));
        let cloak_changed = Arc::new(AtomicBool::new(false));
        let (done, result) = std::sync::mpsc::channel();
        let build = {
            let ctx = ctx.clone();
            let (quit_flag, cloak_changed) = (quit_flag.clone(), cloak_changed.clone());
            let activates = double_click_activates.clone();
            let captions = [tooltip, show, hiding, quit].map(str::to_string);
            move || {
                let built = build(
                    &ctx,
                    hwnd,
                    restored,
                    activation,
                    captions,
                    quit_flag,
                    cloak_changed,
                    activates,
                );
                if let Err(e) = &built {
                    log::error!("tray icon: {e}");
                }
                let _ = done.send(built);
            }
        };
        platform::on_tray_thread(Box::new(build));
        // Made right here (tray on this thread): pass its error on.
        if let Ok(Err(e)) = result.try_recv() {
            return Err(e);
        }
        Ok(Self {
            quit: quit_flag,
            cloak_changed,
            double_click_activates,
        })
    }

    /// Green circle while a profile runs, grey otherwise, with the tooltip for each state
    /// (`[inactive, active]`); no-op if unchanged.
    pub fn set_active(&mut self, active: bool, tooltips: [String; 2]) {
        with_live(move |l| {
            if l.tooltips != tooltips {
                l.tooltips = tooltips;
                l.shown = None;
            }
            l.show(active);
        });
    }

    /// Caption of "Activate" (it names the profile) and whether a profile is loaded.
    pub fn set_profile(&self, caption: &str, loaded: bool) {
        let caption = caption.to_string();
        with_live(move |l| {
            if l.activate.text() != caption {
                l.activate.set_text(&caption);
            }
            if l.activate.is_enabled() != loaded {
                l.activate.set_enabled(loaded);
            }
        });
    }

    /// Check mark of the "Device hiding" item.
    pub fn set_cloaked(&self, on: bool) {
        with_live(move |l| {
            if let Some(item) = &l.cloak
                && item.is_checked() != on
            {
                item.set_checked(on);
            }
        });
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

/// Makes the icon, its menu and their handlers; runs on the tray thread.
/// `captions`: tooltip, "Show", "Device hiding", "Quit".
#[allow(clippy::too_many_arguments)]
fn build(
    ctx: &egui::Context,
    hwnd: Window,
    restored: Arc<AtomicBool>,
    activation: Activation,
    captions: [String; 4],
    quit_flag: Arc<AtomicBool>,
    cloak_changed: Arc<AtomicBool>,
    activates: Arc<AtomicBool>,
) -> Result<(), String> {
    let [tooltip, show, hiding, quit] = captions;
    let icon = tray_image(false)?;
    let show_item = MenuItem::new(&show, true, None);
    // Caption and enabled state follow the loaded profile (`set_profile`).
    let activate_item = CheckMenuItem::new("", false, false, None);
    let quit_item = MenuItem::new(&quit, true, None);
    let menu = Menu::new();
    menu.append(&show_item).map_err(|e| e.to_string())?;
    menu.append(&activate_item).map_err(|e| e.to_string())?;
    // Device hiding on/off without opening the window (where hiding is available).
    let hidhide = gob_hide::load().ok();
    let cloak_item = hidhide.as_ref().map(|hh| {
        let on = hh.enabled().unwrap_or(false);
        CheckMenuItem::new(&hiding, true, on, None)
    });
    if let Some(item) = &cloak_item {
        menu.append(item).map_err(|e| e.to_string())?;
    }
    menu.append(&quit_item).map_err(|e| e.to_string())?;
    let tray = TrayIconBuilder::new()
        .with_tooltip(&tooltip)
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| e.to_string())?;

    {
        let ctx = ctx.clone();
        let restored = restored.clone();
        let activation = activation.clone();
        // Every left click and double click bumps it: a pending single click only acts if
        // nothing followed it within the double-click time.
        let clicks = Arc::new(AtomicU64::new(0));
        let double_at: Arc<Mutex<Option<Instant>>> = Arc::default();
        let wait = platform::double_click_time();
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
                let _ = std::thread::Builder::new()
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
            cloak: cloak_item,
            tooltips: [tooltip.clone(), tooltip],
            shown: None,
        });
    });
    Ok(())
}

impl Drop for Tray {
    /// The icon lives in [`LIVE`]; dropping it there removes it from the notification area.
    fn drop(&mut self) {
        platform::on_tray_thread(Box::new(|| {
            LIVE.with(|l| l.borrow_mut().take());
        }));
    }
}

/// Starts or stops the profile and updates the icon (tray thread: tray callbacks).
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
