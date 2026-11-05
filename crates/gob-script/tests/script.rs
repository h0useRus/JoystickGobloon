//! Scripts against a recording API: variables, handlers per mode, periodic functions, limits.

use std::time::{Duration, Instant};

use gob_core::{InputType, Key, MouseButton, ScriptValue};
use gob_script::{
    EventValue, InputKind, LoadedScript, ScriptApi, ScriptHost, ScriptInput, VarKind, describe,
};

const STICK: &str = "aaaaaaaa-0000-0000-0000-000000000001";

#[derive(Default)]
struct Api {
    out: Vec<String>,
    mode: String,
}

impl ScriptApi for Api {
    fn vjoy_axis(&mut self, device: u32, axis: u8, value: f32) {
        self.out.push(format!("axis {device} {axis} {value:.2}"));
    }
    fn vjoy_button(&mut self, device: u32, button: u8, pressed: bool) {
        self.out.push(format!("button {device} {button} {pressed}"));
    }
    fn vjoy_hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        self.out.push(format!("hat {device} {hat} {direction:?}"));
    }
    fn gamepad_button(&mut self, gamepad: u32, control: &str, pressed: bool) {
        self.out.push(format!("pad {gamepad} {control} {pressed}"));
    }
    fn gamepad_axis(&mut self, gamepad: u32, control: &str, value: f32) {
        self.out.push(format!("pad {gamepad} {control} {value:.2}"));
    }
    fn key(&mut self, key: Key, pressed: bool) {
        self.out.push(format!("key {} {pressed}", key.scan_code));
    }
    fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        self.out.push(format!("mouse {} {pressed}", button.name()));
    }
    fn mouse_move(&mut self, dx: i32, dy: i32) {
        self.out.push(format!("move {dx} {dy}"));
    }
    fn mouse_wheel(&mut self, notches: i32) {
        self.out.push(format!("wheel {notches}"));
    }
    fn current_mode(&self) -> String {
        self.mode.clone()
    }
    fn switch_mode(&mut self, mode: &str) {
        self.out.push(format!("mode {mode}"));
    }
    fn input(&self, input: &ScriptInput) -> Option<EventValue> {
        (input.kind == InputKind::Axis).then_some(EventValue::Axis(0.25))
    }
    fn speak(&mut self, text: &str) {
        self.out.push(format!("say {text}"));
    }
    fn play_sound(&mut self, file: &str, volume: u8) {
        self.out.push(format!("sound {file} {volume}"));
    }
    fn run_action(&mut self, name: &str) -> bool {
        self.out.push(format!("run {name}"));
        name == "Lights"
    }
}

fn script(code: &str, values: Vec<(String, ScriptValue)>) -> LoadedScript {
    LoadedScript {
        name: "Test".into(),
        code: code.into(),
        values,
    }
}

const AXIS_SCRIPT: &str = r#"
local speed = script.float{ name = "Speed", description = "Scale", min = 0, max = 2, default = 1 }
local stick = script.physical_input{ name = "Stick", types = { "axis" } }
local out   = script.virtual_output{ name = "Out", types = { "axis" } }
local mode  = script.mode{ name = "Mode" }
local pick  = script.selection{ name = "Pick", options = { "a", "b" }, default = "b" }
stick:on(mode, function(e) out:set(e.value * speed.value) end)
"#;

#[test]
fn describe_lists_the_variables() {
    let vars = describe("Test", AXIS_SCRIPT).unwrap();
    let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Speed", "Stick", "Out", "Mode", "Pick"]);
    assert_eq!(vars[0].kind, VarKind::Float { min: 0.0, max: 2.0 });
    assert_eq!(vars[0].default, Some(ScriptValue::Float(1.0)));
    assert_eq!(vars[0].description, "Scale");
    assert_eq!(vars[1].kind, VarKind::PhysicalInput(vec![InputType::Axis]));
    assert_eq!(vars[4].default, Some(ScriptValue::Selection(1)));
}

fn axis_values(mode: &str) -> Vec<(String, ScriptValue)> {
    vec![
        ("Speed".into(), ScriptValue::Float(5.0)),
        (
            "Stick".into(),
            ScriptValue::PhysicalInput {
                device: STICK.to_uppercase(),
                input_type: InputType::Axis,
                id: 2,
            },
        ),
        (
            "Out".into(),
            ScriptValue::Vjoy {
                device: 1,
                input_type: InputType::Axis,
                id: 3,
            },
        ),
        ("Mode".into(), ScriptValue::Mode(mode.into())),
    ]
}

#[test]
fn handlers_run_in_their_mode_and_its_children() {
    let mut host = ScriptHost::load(&[script(AXIS_SCRIPT, axis_values("Flight"))]);
    assert!(host.take_errors().is_empty());
    let input = ScriptInput::new(STICK, InputKind::Axis, 2);
    assert!(host.handles(&input));
    let mut api = Api::default();
    host.event(&input, EventValue::Axis(0.4), &["Flight"], &mut api);
    // Speed 5 is clamped to its max 2.
    assert_eq!(api.out, ["axis 1 3 0.80"]);
    host.event(
        &input,
        EventValue::Axis(0.4),
        &["Landing", "Flight"],
        &mut api,
    );
    assert_eq!(api.out.len(), 2, "a child mode inherits the handler");
    host.event(&input, EventValue::Axis(0.4), &["Default"], &mut api);
    assert_eq!(api.out.len(), 2, "not in another mode");
}

#[test]
fn the_api_reaches_the_host() {
    let code = r#"
local key = script.key{ name = "Key" }
script.on_input("AAAAAAAA-0000-0000-0000-000000000001", "button", 1, function(e)
    if not e.pressed then return end
    vjoy.button(1, 5, true)
    vjoy.hat(1, 1, 90)
    vjoy.hat(1, 1, nil)
    gamepad.button(2, "a", true)
    keyboard.tap(key)
    mouse.button("Left", true)
    mouse.move(3.4, -2)
    mode.switch(mode.current() .. "2")
    speak(joystick.axis("AAAAAAAA-0000-0000-0000-000000000001", 1))
    if run_action("Lights") then log("ran", 1) end
end)
"#;
    let values = vec![(
        "Key".into(),
        ScriptValue::Keyboard(Key {
            scan_code: 0x22,
            extended: false,
        }),
    )];
    let mut host = ScriptHost::load(&[script(code, values)]);
    assert!(host.take_errors().is_empty());
    let mut api = Api {
        mode: "Default".into(),
        ..Api::default()
    };
    let input = ScriptInput::new(STICK, InputKind::Button, 1);
    host.event(&input, EventValue::Button(true), &["Default"], &mut api);
    host.event(&input, EventValue::Button(false), &["Default"], &mut api);
    assert_eq!(
        api.out,
        [
            "button 1 5 true",
            "hat 1 1 Some(9000)",
            "hat 1 1 None",
            "pad 2 a true",
            "key 34 true",
            "key 34 false",
            "mouse Left true",
            "move 3 -2",
            "mode Default2",
            "say 0.25",
            "run Lights",
        ]
    );
    assert!(host.take_errors().is_empty());
}

#[test]
fn periodic_functions_keep_their_rhythm() {
    let code = "local n = 0\nscript.every(0.1, function() n = n + 1; vjoy.button(1, n, true) end)";
    let mut host = ScriptHost::load(&[script(code, Vec::new())]);
    let mut api = Api::default();
    let t0 = Instant::now();
    assert!(host.starting());
    host.tick(t0, &mut api);
    assert_eq!(host.next_deadline(), Some(t0 + Duration::from_millis(100)));
    host.tick(t0 + Duration::from_millis(99), &mut api);
    assert!(api.out.is_empty());
    host.tick(t0 + Duration::from_millis(100), &mut api);
    host.tick(t0 + Duration::from_millis(450), &mut api);
    assert_eq!(
        api.out,
        ["button 1 1 true", "button 1 2 true"],
        "late: once"
    );
    assert_eq!(host.next_deadline(), Some(t0 + Duration::from_millis(550)));
}

#[test]
fn the_sandbox_has_no_files_or_programs() {
    for code in ["io.open('x', 'w')", "os.execute('calc')"] {
        let e = describe("Test", code).unwrap_err();
        assert_eq!(e.line, Some(1), "{code}: {e}");
    }
    assert!(describe("Test", "require('socket')").is_err());
    // os.clock is there for timing.
    describe("Test", "local t = os.clock()").unwrap();
}

#[test]
fn a_runaway_script_is_stopped_and_switched_off() {
    let code = "script.on_input('dev', 'button', 1, function() while true do end end)";
    let mut host = ScriptHost::load(&[script(code, Vec::new())]);
    let mut api = Api::default();
    let input = ScriptInput::new("dev", InputKind::Button, 1);
    let start = Instant::now();
    for _ in 0..12 {
        host.event(&input, EventValue::Button(true), &["Default"], &mut api);
    }
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "each call is cut short"
    );
    let errors = host.take_errors();
    assert_eq!(errors.len(), 1, "reported once");
    assert!(errors[0].message.contains("too long"), "{}", errors[0]);

    let e = describe("Test", "while true do end").unwrap_err();
    assert!(e.message.contains("too long"), "{e}");
}

#[test]
fn errors_name_their_line() {
    let e = describe("Gear", "local a = 1\nlocal b = nil + 1\n").unwrap_err();
    assert_eq!((e.script.as_str(), e.line), ("Gear", Some(2)), "{e}");
    let e = describe("Gear", "vjoy.button(1, 1, true)").unwrap_err();
    assert!(e.message.contains("only inside handlers"), "{e}");
    let mut host = ScriptHost::load(&[script("local x = (", Vec::new())]);
    assert!(host.is_empty());
    assert_eq!(host.take_errors().len(), 1);
}
