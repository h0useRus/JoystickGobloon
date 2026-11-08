//! Delay: its actions run some time after the press, while held or as a short press.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_engine::{Engine, InputKey, InputKind, Output, Value};

const DEV: &str = "aaaaaaaa-0000-0000-0000-000000000001";

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

/// Button 1 runs a Delay of 0.5 s around vJoy button 1; `while_held` as given.
fn engine(while_held: bool) -> Engine {
    let held = if while_held { "True" } else { "False" };
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><inputs><input><device-id>{DEV}</device-id>
        <input-type>button</input-type><mode>Default</mode><input-id>1</input-id>
        <action-configuration><root-action>r</root-action><behavior>button</behavior>
        </action-configuration></input></inputs>
        <settings><startup-mode>Default</startup-mode></settings><library>
        <action id="v" type="map-to-vjoy">{}{}{}{}{}</action>
        <action id="d" type="delay"><actions><action-id>v</action-id></actions>{}{}{}</action>
        <action id="r" type="root"><actions><action-id>d</action-id></actions></action>
        </library><modes><mode>Default</mode></modes></profile>"#,
        prop("int", "vjoy-device-id", "1"),
        prop("int", "vjoy-input-id", "1"),
        prop("input_type", "vjoy-input-type", "button"),
        prop("bool", "button-inverted", "False"),
        prop("activation-mode", "activation-mode", "both"),
        prop("float", "delay", "0.5"),
        prop("bool", "while-held", held),
        prop("activation-mode", "activation-mode", "disallowed"),
    );
    Engine::new(&gob_gremlin::from_str(&xml).unwrap(), &HashMap::new())
}

fn press(e: &mut Engine, pressed: bool, now: Instant, out: &mut Recorder) {
    let key = InputKey::new(DEV, InputKind::Button, 1);
    e.process(&key, Value::Button(pressed), now, out);
}

fn ms(t0: Instant, ms: u64) -> Instant {
    t0 + Duration::from_millis(ms)
}

#[test]
fn held_past_the_delay_presses_until_let_go() {
    let (mut e, mut out, t0) = (engine(true), Recorder::default(), Instant::now());
    press(&mut e, true, t0, &mut out);
    e.tick(ms(t0, 400), &mut out);
    assert!(out.0.is_empty(), "not yet");
    e.tick(ms(t0, 500), &mut out);
    assert_eq!(out.0, [(1, true)]);
    press(&mut e, false, ms(t0, 900), &mut out);
    assert_eq!(out.0, [(1, true), (1, false)]);
}

#[test]
fn let_go_early_cancels_while_it_needs_holding() {
    let (mut e, mut out, t0) = (engine(true), Recorder::default(), Instant::now());
    press(&mut e, true, t0, &mut out);
    press(&mut e, false, ms(t0, 200), &mut out);
    e.tick(ms(t0, 1000), &mut out);
    assert!(out.0.is_empty());
    assert!(e.next_deadline().is_none(), "no timer left");
}

#[test]
fn a_short_press_runs_late_as_a_short_press_otherwise() {
    let (mut e, mut out, t0) = (engine(false), Recorder::default(), Instant::now());
    press(&mut e, true, t0, &mut out);
    press(&mut e, false, ms(t0, 100), &mut out);
    assert!(out.0.is_empty());
    e.tick(ms(t0, 500), &mut out);
    assert_eq!(out.0, [(1, true)]);
    e.tick(ms(t0, 2000), &mut out);
    assert_eq!(out.0, [(1, true), (1, false)], "let go after a pulse");
    // Pressed again before the time is up: the time starts again.
    out.0.clear();
    press(&mut e, true, ms(t0, 3000), &mut out);
    press(&mut e, false, ms(t0, 3100), &mut out);
    press(&mut e, true, ms(t0, 3300), &mut out);
    e.tick(ms(t0, 3600), &mut out);
    assert!(out.0.is_empty(), "restarted at 3300");
    e.tick(ms(t0, 3800), &mut out);
    assert_eq!(out.0, [(1, true)]);
}
