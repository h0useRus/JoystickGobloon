//! What Lua scripts reach of the engine while one of their handlers runs.

use super::*;

/// What scripts reach while one of their functions runs: the engine and its outputs.
pub(super) struct ScriptContext<'a> {
    pub(super) engine: &'a mut Engine,
    pub(super) out: &'a mut dyn Output,
    pub(super) now: Instant,
}

impl ScriptApi for ScriptContext<'_> {
    fn vjoy_axis(&mut self, device: u32, axis: u8, value: f32) {
        self.engine.set_axis(device, axis, value, self.out);
    }
    fn vjoy_button(&mut self, device: u32, button: u8, pressed: bool) {
        self.engine.vjoy_button(device, button, pressed, self.out);
    }
    fn vjoy_hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        let held = &mut self.engine.script_hats;
        held.retain(|&h| h != (device, hat));
        if direction.is_some() {
            held.push((device, hat));
        }
        self.out.hat(device, hat, direction);
    }
    fn gamepad_button(&mut self, gamepad: u32, control: &str, pressed: bool) {
        if let Some(c) = GamepadControl::parse(control) {
            self.engine.pad_button(gamepad, c, pressed, self.out);
        }
    }
    fn gamepad_axis(&mut self, gamepad: u32, control: &str, value: f32) {
        if let Some(c) = GamepadControl::parse(control) {
            let low = if c.kind() == ControlKind::Trigger {
                0.0
            } else {
                -1.0
            };
            self.engine
                .pad_axis(gamepad, c, value.clamp(low, 1.0), self.out);
        }
    }
    fn key(&mut self, key: Key, pressed: bool) {
        let held = &mut self.engine.script_keys_down;
        held.retain(|&k| k != key);
        if pressed {
            held.push(key);
        }
        self.out.key(key.scan_code, key.extended, pressed);
    }
    fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::WheelUp => self.out.mouse_wheel(1),
            MouseButton::WheelDown => self.out.mouse_wheel(-1),
            b => self.engine.mouse_button(b, pressed, self.out),
        }
    }
    fn mouse_move(&mut self, dx: i32, dy: i32) {
        self.out.mouse_move(dx, dy);
    }
    fn mouse_wheel(&mut self, notches: i32) {
        self.out.mouse_wheel(notches);
    }
    fn current_mode(&self) -> String {
        self.engine.modes.current().to_string()
    }
    fn switch_mode(&mut self, mode: &str) -> bool {
        if !self.engine.script_modes.contains(mode) {
            return false;
        }
        self.engine.modes.switch_to(mode, false);
        true
    }
    fn input(&self, input: &ScriptInput) -> Option<EventValue> {
        let kind = match input.kind {
            ScriptKind::Axis => InputKind::Axis,
            ScriptKind::Button => InputKind::Button,
            ScriptKind::Hat => InputKind::Hat,
        };
        let key = InputKey {
            device: self.engine.device(&input.device),
            kind,
            id: input.id,
        };
        match kind {
            InputKind::Axis => self.engine.axes.get(&key).map(|v| EventValue::Axis(*v)),
            InputKind::Button => self
                .engine
                .buttons
                .get(&key)
                .map(|b| EventValue::Button(*b)),
            InputKind::Hat => self.engine.hats.get(&key).map(|d| EventValue::Hat(*d)),
        }
    }
    fn speak(&mut self, text: &str) {
        self.out.speak(SpeechRequest {
            text: text.to_string(),
            voice: None,
            rate: 0.0,
            volume: 1.0,
            pitch: 0.0,
            queue: SpeechQueue::Back,
        });
    }
    fn play_sound(&mut self, file: &str, volume: u8) {
        self.out.play_sound(file, volume);
    }
    fn run_action(&mut self, name: &str) -> bool {
        let node = self
            .engine
            .action_labels
            .get(name.trim())
            .and_then(|id| self.engine.action_nodes.get(id))
            .copied();
        match node {
            Some(node) => {
                self.engine.run_action(node, self.now, self.out);
                true
            }
            None => false,
        }
    }
}
