//! Lua scripts in the engine: handlers before actions, the API, periodic functions, modes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_core::{InputType, Profile, ScriptSource, ScriptValue};
use gob_engine::{Engine, InputKey, InputKind, Output, Value};

const DEV: &str = "aaaaaaaa-0000-0000-0000-000000000001";

#[derive(Default)]
struct Recorder(Vec<String>);

impl Output for Recorder {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        self.0.push(format!("axis {device} {axis} {value:.2}"));
    }
    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        self.0.push(format!("button {device} {button} {pressed}"));
    }
    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        self.0.push(format!("hat {device} {hat} {direction:?}"));
    }
    fn speak(&mut self, request: gob_engine::SpeechRequest) {
        self.0.push(format!("say {}", request.text));
    }
}

fn prop(ty: &str, name: &str, value: &str) -> String {
    format!(r#"<property type="{ty}"><name>{name}</name><value>{value}</value></property>"#)
}

fn act(id: &str, kind: &str, body: &str) -> String {
    format!(
        r#"<action id="{id}" type="{kind}">{body}{}</action>"#,
        prop("activation-mode", "activation-mode", "both")
    )
}

fn vjoy(id: &str, n: u32, label: &str) -> String {
    act(
        id,
        "map-to-vjoy",
        &format!(
            "{}{}{}{}{}",
            prop("string", "action-label", label),
            prop("int", "vjoy-device-id", "1"),
            prop("int", "vjoy-input-id", &n.to_string()),
            prop("input_type", "vjoy-input-type", "button"),
            prop("bool", "button-inverted", "False")
        ),
    )
}

/// Button 1 presses virtual button 7 (named "Lights"); modes Default and its child Landing.
fn profile() -> Profile {
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><inputs><input><device-id>{DEV}</device-id>
        <input-type>button</input-type><mode>Default</mode><input-id>1</input-id>
        <action-configuration><root-action>r</root-action><behavior>button</behavior>
        </action-configuration></input></inputs><settings/><library>{}{}</library>
        <modes><mode>Default</mode><mode parent="Default">Landing</mode></modes></profile>"#,
        act("r", "root", "<actions><action-id>v</action-id></actions>"),
        vjoy("v", 7, "Lights"),
    );
    gob_gremlin::from_str(&xml).unwrap()
}

fn engine_with(code: &str, values: &[(&str, ScriptValue)]) -> Engine {
    let mut p = profile();
    let id = p.add_script("Test", ScriptSource::Code(code.into()));
    for (name, value) in values {
        p.set_script_variable(&id, name, Some(value));
    }
    Engine::new(&p, &HashMap::new())
}

fn button(n: u32) -> InputKey {
    InputKey::new(DEV, InputKind::Button, n)
}

#[test]
fn handlers_run_before_the_actions() {
    let code = r#"
local b = script.physical_input{ name = "Button", types = { "button" } }
local out = script.virtual_output{ name = "Out", types = { "button" } }
b:on(function(e) out:set(e.pressed) end)
"#;
    let values = [
        (
            "Button",
            ScriptValue::PhysicalInput {
                device: DEV.to_uppercase(),
                input_type: InputType::Button,
                id: 1,
            },
        ),
        (
            "Out",
            ScriptValue::Vjoy {
                device: 1,
                input_type: InputType::Button,
                id: 9,
            },
        ),
    ];
    let mut e = engine_with(code, &values);
    assert!(e.take_script_errors().is_empty());
    let mut out = Recorder::default();
    let t0 = Instant::now();
    e.process(&button(1), Value::Button(true), t0, &mut out);
    e.process(&button(1), Value::Button(false), t0, &mut out);
    assert_eq!(
        out.0,
        [
            "button 1 9 true",
            "button 1 7 true",
            "button 1 9 false",
            "button 1 7 false"
        ]
    );
}

#[test]
fn modes_actions_and_periodic_functions() {
    let code = r#"
script.on_input("AAAAAAAA-0000-0000-0000-000000000001", "button", 2, "Default", function(e)
    if not e.pressed then return end
    if mode.current() == "Default" then
        mode.switch("Landing")
        speak("gear")
    else
        run_action("Lights")
    end
end)
local n = 0
script.every(0.1, function() n = n + 1; vjoy.axis(1, 1, n / 10) end)
"#;
    let mut e = engine_with(code, &[]);
    let mut out = Recorder::default();
    let t0 = Instant::now();
    e.activate(&mut out);
    e.process(&button(2), Value::Button(true), t0, &mut out);
    assert_eq!(e.current_mode(), "Landing");
    // Landing inherits the handler of its parent Default.
    e.process(&button(2), Value::Button(true), t0, &mut out);
    assert_eq!(out.0, ["say gear", "button 1 7 true"]);
    out.0.clear();
    // The run action's press goes up after 50 ms, the periodic function runs every 100 ms.
    assert!(e.next_deadline().is_some());
    e.tick(Instant::now() + Duration::from_millis(150), &mut out);
    assert_eq!(out.0, ["button 1 7 false", "axis 1 1 0.10"]);
}

#[test]
fn script_errors_reach_the_runner() {
    let mut e = engine_with("local x = (", &[]);
    let errors = e.take_script_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].script, "Test");
    assert!(e.take_script_errors().is_empty(), "taken once");
    // The profile's actions still work.
    let mut out = Recorder::default();
    e.process(&button(1), Value::Button(true), Instant::now(), &mut out);
    assert_eq!(out.0, ["button 1 7 true"]);
}
