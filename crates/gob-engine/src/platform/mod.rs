//! Keyboard/mouse output and speech per platform behind one interface, so the engine and the
//! runner stay the same everywhere (docs/LINUX_PLAN.md, "Архитектура"). `windows/`: SendInput
//! and SAPI; `linux/`: uinput (speech-dispatcher later); `none.rs`: nothing.

use gob_core::MouseButton;

use crate::media::SpeechRequest;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(windows))]
mod none;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as current;
#[cfg(not(any(windows, target_os = "linux")))]
use none as current;
#[cfg(windows)]
use windows as current;

/// This platform's speech engine.
pub(crate) type Speech = current::Speech;

/// Keyboard and mouse output, as the user's own devices would produce it.
pub(crate) trait Injector {
    /// Gets ready before the first output (e.g. makes virtual devices); called when a profile
    /// starts.
    fn prepare(&self) {}
    /// Relative cursor movement in pixels.
    fn mouse_move(&self, dx: i32, dy: i32);
    fn mouse_button(&self, button: MouseButton, pressed: bool);
    /// Wheel notches: positive scrolls up.
    fn mouse_wheel(&self, notches: i32);
    /// Keyboard key by set 1 scan code; `extended` adds the E0 prefix.
    fn key(&self, scan_code: u16, extended: bool, pressed: bool);
    /// Whether a keyboard key is held right now, for conditions.
    fn key_pressed(&self, scan_code: u16, extended: bool) -> bool;
}

/// Text-to-speech, created on the speech thread and used only there.
pub(crate) trait SpeechEngine: Sized {
    /// `None` when the system has no speech (logged by the caller).
    fn new() -> Option<Self>;
    /// Installed voice names, in any order.
    fn voices() -> Vec<String>;
    fn speak(&mut self, request: &SpeechRequest);
}

/// This platform's keyboard and mouse output.
pub(crate) fn injector() -> &'static impl Injector {
    static INJECTOR: current::Injector = current::Injector;
    &INJECTOR
}
