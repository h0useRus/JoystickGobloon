//! Key combinations: down when the last of their keys goes down, up when one goes up.

use std::collections::HashMap;
use std::time::Instant;

use gob_core::KEYBOARD_GUID;
use gob_engine::{Engine, InputKey, InputKind, Output, Value};

#[derive(Default)]
struct Recorder(Vec<(u8, bool)>);

impl Output for Recorder {
    fn axis(&mut self, _device: u32, _axis: u8, _value: f32) {}
    fn button(&mut self, _device: u32, button: u8, pressed: bool) {
        self.0.push((button, pressed));
    }
    fn hat(&mut self, _device: u32, _hat: u8, _direction: Option<u16>) {}
}

fn prop(ty: &str, name: &str, value: &str) -> String {
    format!(r#"<property type="{ty}"><name>{name}</name><value>{value}</value></property>"#)
}

fn vjoy(n: u32) -> String {
    format!(
        r#"<action id="v{n}" type="map-to-vjoy">{}{}{}{}{}</action>
        <action id="r{n}" type="root"><actions><action-id>v{n}</action-id></actions></action>"#,
        prop("int", "vjoy-device-id", "1"),
        prop("int", "vjoy-input-id", &n.to_string()),
        prop("input_type", "vjoy-input-type", "button"),
        prop("bool", "button-inverted", "False"),
        prop("activation-mode", "activation-mode", "both"),
    )
}

fn binding(kind: &str, id: u32, root: &str) -> String {
    format!(
        "<input><device-id>{KEYBOARD_GUID}</device-id><input-type>{kind}</input-type>\
         <mode>Default</mode><input-id>{id}</input-id><action-configuration>\
         <root-action>{root}</root-action><behavior>button</behavior></action-configuration>\
         </input>"
    )
}

const CTRL: u32 = 0x1d;
const SHIFT: u32 = 0x2a;
const F1: u32 = 0x3b;

fn key(e: &mut Engine, id: u32, pressed: bool, out: &mut Recorder) {
    let k = InputKey::new(KEYBOARD_GUID, InputKind::Button, id);
    e.process(&k, Value::Button(pressed), Instant::now(), out);
}

#[test]
fn combination_runs_while_all_its_keys_are_held() {
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><inputs>{}{}</inputs>
        <settings><startup-mode>Default</startup-mode></settings><library>{}{}</library>
        <modes><mode>Default</mode></modes>
        <key-combos><combo id="1"><key scan-code="{CTRL}" extended="false"/>
        <key scan-code="{F1}" extended="false"/></combo>
        <combo id="2"><key scan-code="{CTRL}" extended="false"/>
        <key scan-code="{SHIFT}" extended="false"/></combo></key-combos></profile>"#,
        binding("combo", 1, "r1"),
        binding("key", F1, "r2"),
        vjoy(1),
        vjoy(2),
    );
    let p = gob_gremlin::from_str(&xml).unwrap();
    assert_eq!(p.key_combos.len(), 2);
    let again = gob_gremlin::from_str(&gob_gremlin::to_string(&p)).unwrap();
    assert_eq!(again, p, "written and read back the same");
    let mut e = Engine::new(&p, &HashMap::new());
    let mut out = Recorder::default();

    // In any order: F1 first runs its own binding, Ctrl then completes the combination.
    key(&mut e, F1, true, &mut out);
    key(&mut e, CTRL, true, &mut out);
    assert_eq!(out.0, [(2, true), (1, true)]);
    out.0.clear();
    key(&mut e, CTRL, false, &mut out);
    assert_eq!(out.0, [(1, false)], "up when one key goes up");
    out.0.clear();
    key(&mut e, CTRL, true, &mut out);
    assert_eq!(out.0, [(1, true)], "down again");
    key(&mut e, F1, false, &mut out);
    key(&mut e, CTRL, false, &mut out);
    assert_eq!(out.0[1..], [(2, false), (1, false)]);
    // Combination 2 has no binding: Ctrl+Shift does nothing.
    out.0.clear();
    key(&mut e, CTRL, true, &mut out);
    key(&mut e, SHIFT, true, &mut out);
    assert!(out.0.is_empty());
}
