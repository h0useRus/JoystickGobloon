//! Platforms without keyboard/mouse output and speech yet (Linux: uinput and speech-dispatcher,
//! docs/LINUX_PLAN.md phases 3 and 5).

use gob_core::MouseButton;

use crate::media::SpeechRequest;

pub(crate) struct Injector;

impl super::Injector for Injector {
    fn mouse_move(&self, _dx: i32, _dy: i32) {}
    fn mouse_button(&self, _button: MouseButton, _pressed: bool) {}
    fn mouse_wheel(&self, _notches: i32) {}
    fn key(&self, _scan_code: u16, _extended: bool, _pressed: bool) {}
    fn key_pressed(&self, _scan_code: u16, _extended: bool) -> bool {
        false
    }
}

pub(crate) struct Speech;

impl super::SpeechEngine for Speech {
    fn new() -> Option<Self> {
        None
    }

    fn voices() -> Vec<String> {
        Vec::new()
    }

    fn speak(&mut self, _request: &SpeechRequest) {}
}
