//! Executes a profile: physical input events in, vJoy output out.
//!
//! [`Engine`] is pure logic driven by [`Engine::process`] and [`Engine::tick`]
//! with explicit timestamps, so it can be tested without hardware. [`Runner`]
//! wires it to `gob-input` and `gob-vjoy` on a background thread.
//!
//! Semantics follow Joystick Gremlin R14: actions of a root run in order and
//! share one mutable value, modes form a stack with inheritance, vJoy buttons
//! are auto-released when their input is released in another mode.

mod engine;
mod modes;
mod motion;
mod runner;
mod sendinput;

pub use engine::Engine;
pub use runner::{Runner, RunnerStatus};

/// Value flowing through an action tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Button(bool),
    /// -1.0..=1.0
    Axis(f32),
    /// Hundredths of a degree, `None` when centered.
    Hat(Option<u16>),
}

impl Value {
    /// Python-style truthiness used by Gremlin's activation checks.
    pub fn truthy(self) -> bool {
        match self {
            Value::Button(b) => b,
            Value::Axis(v) => v != 0.0,
            Value::Hat(_) => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKind {
    Axis,
    Button,
    Hat,
}

/// A physical (or virtual) input: lowercase device GUID, kind and 1-based id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputKey {
    pub device: String,
    pub kind: InputKind,
    pub id: u32,
}

impl InputKey {
    pub fn new(device: &str, kind: InputKind, id: u32) -> Self {
        Self {
            device: device.to_ascii_lowercase(),
            kind,
            id,
        }
    }
}

/// Destination for engine output; implemented by vJoy and by test recorders.
pub trait Output {
    fn axis(&mut self, device: u32, axis: u8, value: f32);
    fn button(&mut self, device: u32, button: u8, pressed: bool);
    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>);
    /// Relative cursor movement in pixels.
    fn mouse_move(&mut self, _dx: i32, _dy: i32) {}
    fn mouse_button(&mut self, _button: gob_core::MouseButton, _pressed: bool) {}
    /// Wheel notches: positive scrolls up.
    fn mouse_wheel(&mut self, _notches: i32) {}
    /// Keyboard key by set 1 scan code; `extended` adds the E0 prefix.
    fn key(&mut self, _scan_code: u16, _extended: bool, _pressed: bool) {}
    /// Starts a program (run-command action).
    fn run_command(&mut self, _executable: &str, _arguments: &[String]) {}
}
