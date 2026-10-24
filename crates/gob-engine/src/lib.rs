//! Executes a profile: physical input events in, vJoy output out.
//!
//! [`Engine`] is pure logic driven by [`Engine::process`] and [`Engine::tick`]
//! with explicit timestamps, so it can be tested without hardware. [`Runner`]
//! wires it to `gob-input` and `gob-virtual` (virtual joysticks) on a background thread.
//!
//! Semantics follow Joystick Gremlin R14: actions of a root run in order and
//! share one mutable value, modes form a stack with inheritance, vJoy buttons
//! are auto-released when their input is released in another mode. On top of that,
//! modifiers choose layers of an input's configurations (`gob_core::modifier`).

mod engine;
mod lighting;
mod media;
mod modes;
mod motion;
mod platform;
mod runner;

pub use engine::Engine;
pub use lighting::to_led;
pub use media::{Media, SpeechQueue, SpeechRequest, voices};
pub use runner::{Latency, Runner, RunnerStatus};

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

/// One LED of a joystick (light-led action).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Light {
    /// Attached device GUID, lowercase (profile aliases applied).
    pub device: String,
    pub led: u8,
    /// 0: the whole LED; 1, 2: one element of it (see `gob_lighting::Led::element`).
    pub element: u8,
    /// 8-bit RGB.
    pub color: [u8; 3],
    pub color2: [u8; 3],
    pub blend: gob_core::LedBlend,
    pub mode: gob_core::LedMode,
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
    /// Plays a sound file, `volume` 0..=100 (play-sound action).
    fn play_sound(&mut self, _file: &str, _volume: u8) {}
    /// Sets a joystick LED (light-led action). Must not block: the runner queues it.
    fn light_led(&mut self, _light: &Light) {}
    /// Text-to-speech action.
    fn speak(&mut self, _request: SpeechRequest) {}
    /// Load-profile action: the GUI switches to `file` and activates it.
    fn load_profile(&mut self, _file: &str) {}
    /// Last value sent to a vJoy input, for conditions; `None` if never set.
    fn vjoy_state(&self, _device: u32, _kind: InputKind, _id: u32) -> Option<Value> {
        None
    }
    /// Whether a keyboard key is held right now, for conditions.
    fn key_pressed(&self, _scan_code: u16, _extended: bool) -> bool {
        false
    }
}
