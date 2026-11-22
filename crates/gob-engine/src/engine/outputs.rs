//! Outputs that hold writes back (`Deferred`) or note what was written (`Recording`), for layer changes.

use super::*;

/// A vJoy output written during a layer change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Axis(u32, u8),
    Button(u32, u8),
    Hat(u32, u8),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Write {
    Axis(u32, u8, f32),
    Button(u32, u8, bool),
    Hat(u32, u8, Option<u16>),
}

impl Write {
    pub(super) fn target(&self) -> Target {
        match *self {
            Write::Axis(d, a, _) => Target::Axis(d, a),
            Write::Button(d, b, _) => Target::Button(d, b),
            Write::Hat(d, h, _) => Target::Hat(d, h),
        }
    }

    pub(super) fn send(self, out: &mut dyn Output) {
        match self {
            Write::Axis(d, a, v) => out.axis(d, a, v),
            Write::Button(d, b, p) => out.button(d, b, p),
            Write::Hat(d, h, v) => out.hat(d, h, v),
        }
    }
}

/// Forwards to `out` but holds back vJoy writes (the neutral values of a layer change).
pub(super) struct Deferred<'a> {
    out: &'a mut dyn Output,
    pending: Vec<Write>,
}

impl<'a> Deferred<'a> {
    pub(super) fn new(out: &'a mut dyn Output) -> Self {
        Self {
            out,
            pending: Vec::new(),
        }
    }

    pub(super) fn into_parts(self) -> (Vec<Write>, &'a mut dyn Output) {
        (self.pending, self.out)
    }
}

/// Forwards to `out` and notes which vJoy outputs were written.
pub(super) struct Recording<'a> {
    out: &'a mut dyn Output,
    written: Vec<Target>,
}

impl<'a> Recording<'a> {
    pub(super) fn new(out: &'a mut dyn Output) -> Self {
        Self {
            out,
            written: Vec::new(),
        }
    }

    pub(super) fn into_parts(self) -> (Vec<Target>, &'a mut dyn Output) {
        (self.written, self.out)
    }
}

/// The non-vJoy outputs of a wrapper go straight through to `self.out`.
macro_rules! forward_output {
    () => {
        fn mouse_move(&mut self, dx: i32, dy: i32) {
            self.out.mouse_move(dx, dy)
        }
        fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
            self.out.mouse_button(button, pressed)
        }
        fn mouse_wheel(&mut self, notches: i32) {
            self.out.mouse_wheel(notches)
        }
        fn key(&mut self, scan_code: u16, extended: bool, pressed: bool) {
            self.out.key(scan_code, extended, pressed)
        }
        fn run_command(&mut self, executable: &str, arguments: &[String]) {
            self.out.run_command(executable, arguments)
        }
        fn play_sound(&mut self, file: &str, volume: u8) {
            self.out.play_sound(file, volume)
        }
        fn light_led(&mut self, light: &Light) {
            self.out.light_led(light)
        }
        fn speak(&mut self, request: SpeechRequest) {
            self.out.speak(request)
        }
        fn load_profile(&mut self, file: &str) {
            self.out.load_profile(file)
        }
        fn send_osc(&mut self, host: &str, port: u16, message: &gob_osc::Message) {
            self.out.send_osc(host, port, message)
        }
        fn vjoy_state(&self, device: u32, kind: InputKind, id: u32) -> Option<Value> {
            self.out.vjoy_state(device, kind, id)
        }
        fn key_pressed(&self, scan_code: u16, extended: bool) -> bool {
            self.out.key_pressed(scan_code, extended)
        }
    };
}

impl Output for Deferred<'_> {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        self.pending.push(Write::Axis(device, axis, value));
    }
    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        self.pending.push(Write::Button(device, button, pressed));
    }
    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        self.pending.push(Write::Hat(device, hat, direction));
    }
    forward_output!();
}

impl Output for Recording<'_> {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        self.written.push(Target::Axis(device, axis));
        self.out.axis(device, axis, value);
    }
    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        self.written.push(Target::Button(device, button));
        self.out.button(device, button, pressed);
    }
    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        self.written.push(Target::Hat(device, hat));
        self.out.hat(device, hat, direction);
    }
    forward_output!();
}
