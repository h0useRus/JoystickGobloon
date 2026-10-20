//! The main window and the tray on Linux.
//!
//! The window is driven through egui's viewport commands. To the tray: hidden on X11; on
//! Wayland, where a window cannot hide itself, minimized instead. The tray icon (tray-icon over
//! AppIndicator) lives on its own GTK thread, as GTK wants; everything that touches it runs
//! there ([`on_tray_thread`]).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;

/// The main window: its egui context is kept here, the handle carries nothing.
#[derive(Clone, Copy)]
pub(crate) struct Window;

static CONTEXT: OnceLock<egui::Context> = OnceLock::new();
/// Hidden to the tray (X11).
static HIDDEN: AtomicBool = AtomicBool::new(false);

pub(super) fn main_window(cc: &eframe::CreationContext<'_>) -> Option<Window> {
    let _ = CONTEXT.set(cc.egui_ctx.clone());
    Some(Window)
}

fn command(cmd: egui::ViewportCommand) {
    if let Some(ctx) = CONTEXT.get() {
        ctx.send_viewport_cmd(cmd);
        ctx.request_repaint();
    }
}

/// Wayland: windows cannot hide (or place) themselves.
fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty())
}

pub(super) fn hide_to_tray(_: Window) {
    if wayland() {
        command(egui::ViewportCommand::Minimized(true));
    } else {
        HIDDEN.store(true, Ordering::Relaxed);
        command(egui::ViewportCommand::Visible(false));
    }
}

pub(super) fn show(_: Window) {
    HIDDEN.store(false, Ordering::Relaxed);
    command(egui::ViewportCommand::Visible(true));
    command(egui::ViewportCommand::Minimized(false));
    command(egui::ViewportCommand::Focus);
}

pub(super) fn minimize(_: Window) {
    command(egui::ViewportCommand::Minimized(true));
}

pub(super) fn in_tray(_: Window) -> bool {
    HIDDEN.load(Ordering::Relaxed)
}

pub(super) fn shown(w: Window) -> bool {
    let minimized = CONTEXT
        .get()
        .is_some_and(|ctx| ctx.input(|i| i.viewport().minimized.unwrap_or(false)));
    !in_tray(w) && !minimized
}

/// Runs `f` on the GTK thread, which is started (with `gtk::init` and its main loop) on first
/// use. Without a display GTK does not start and `f` never runs (logged once).
pub(super) fn on_tray_thread(f: Box<dyn FnOnce() + Send>) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let spawned = std::thread::Builder::new()
            .name("gob-tray".into())
            .spawn(|| {
                if let Err(e) = gtk::init() {
                    log::error!("tray icon: GTK: {e}");
                    return;
                }
                gtk::main();
            });
        if let Err(e) = spawned {
            log::error!("tray thread: {e}");
        }
    });
    // Runs when the GTK main loop next looks at its sources: always on that thread.
    gtk::glib::idle_add_once(f);
}
