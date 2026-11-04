//! button-axis: buttons move an axis (set, velocity, stepped, encoder, smooth), with explicit
//! times; the axis values written are recorded.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_engine::{Engine, InputKey, InputKind, Output, Value};

const DEV: &str = "aaaaaaaa-0000-0000-0000-000000000001";
const OTHER: &str = "aaaaaaaa-0000-0000-0000-000000000002";

#[derive(Default)]
struct Recorder {
    axes: Vec<f32>,
    pad: Vec<(&'static str, f32)>,
}

impl Output for Recorder {
    fn axis(&mut self, _device: u32, _axis: u8, value: f32) {
        self.axes.push(value);
    }
    fn button(&mut self, _device: u32, _button: u8, _pressed: bool) {}
    fn hat(&mut self, _device: u32, _hat: u8, _direction: Option<u16>) {}
    fn gamepad_axis(&mut self, _gamepad: u32, control: gob_engine::GamepadControl, value: f32) {
        self.pad.push((control.name(), value));
    }
}

fn prop(name: &str, value: &str) -> String {
    format!(r#"<property type="string"><name>{name}</name><value>{value}</value></property>"#)
}

/// Button 1 of DEV runs a button-axis action with `props`; its decrease input is button 1 of
/// OTHER.
fn rig(props: &[(&str, &str)]) -> Rig {
    let mut body = prop("decrease-device-id", OTHER) + &prop("decrease-input-id", "1");
    for (n, v) in props {
        body += &prop(n, v);
    }
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><inputs><input><device-id>{DEV}</device-id>
        <input-type>button</input-type><mode>Default</mode><input-id>1</input-id>
        <action-configuration><root-action>r</root-action><behavior>button</behavior>
        </action-configuration></input></inputs><settings/><library>
        <action id="r" type="root"><actions><action-id>a</action-id></actions></action>
        <action id="a" type="button-axis">{body}</action></library>
        <modes><mode>Default</mode></modes></profile>"#
    );
    let p = gob_gremlin::from_str(&xml).unwrap();
    assert!(
        p.needs_own_format(),
        "button-axis is Joystick Gobloon's own"
    );
    let mut r = Rig {
        engine: Engine::new(&p, &HashMap::new()),
        out: Recorder::default(),
        t0: Instant::now(),
    };
    r.engine.activate(&mut r.out);
    r
}

struct Rig {
    engine: Engine,
    out: Recorder,
    t0: Instant,
}

impl Rig {
    fn at(&self, ms: u64) -> Instant {
        self.t0 + Duration::from_millis(ms)
    }

    /// The up (`true`) or down button goes down or up at `ms`.
    fn input(&mut self, up: bool, pressed: bool, ms: u64) {
        let now = self.at(ms);
        let device = if up { DEV } else { OTHER };
        self.engine.process(
            &InputKey::new(device, InputKind::Button, 1),
            Value::Button(pressed),
            now,
            &mut self.out,
        );
    }

    fn tap(&mut self, up: bool, ms: u64) {
        self.input(up, true, ms);
        self.input(up, false, ms + 1);
    }

    /// Runs the engine's timers up to `ms`, one tick per deadline like the runner.
    fn until(&mut self, ms: u64) {
        let end = self.at(ms);
        while let Some(d) = self.engine.next_deadline().filter(|&d| d <= end) {
            self.engine.tick(d, &mut self.out);
        }
        self.engine.tick(end, &mut self.out);
    }

    fn take(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.out.axes)
    }

    fn last(&self) -> f32 {
        *self.out.axes.last().expect("an axis was written")
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn set_value_and_back() {
    let mut r = rig(&[("set-value", "0.5"), ("return-on-release", "True")]);
    assert!(r.take().is_empty(), "no start value: nothing written");
    r.input(true, true, 0);
    assert_eq!(r.take(), [0.5]);
    r.input(true, false, 100);
    assert_eq!(r.take(), [0.0], "back to where it was");
    r.input(false, true, 200);
    assert_eq!(r.take(), [-0.5], "the decrease input sets the opposite");
    assert_eq!(r.engine.next_deadline(), None);
}

#[test]
fn velocity_travels_while_held() {
    let mut r = rig(&[("motion", "velocity"), ("speed", "0.5")]);
    r.input(true, true, 0);
    assert!(r.engine.animating());
    r.until(500);
    assert!(
        close(r.last(), 0.5),
        "half a travel per second: {}",
        r.last()
    );
    let steps = r.take().len();
    assert!((90..=101).contains(&steps), "a write every 5 ms: {steps}");
    r.input(true, false, 500);
    r.until(2000);
    assert!(r.take().len() <= 1, "stops when let go");
    assert_eq!(r.engine.next_deadline(), None, "idle: no wake-ups");
    assert!(!r.engine.animating());

    // Down to the end, where it stops by itself.
    r.input(false, true, 3000);
    r.until(6000);
    assert_eq!(r.last(), -1.0);
    assert_eq!(r.engine.next_deadline(), None, "at the end it waits");
}

#[test]
fn velocity_accelerates_to_its_top_speed() {
    let mut r = rig(&[
        ("motion", "velocity"),
        ("speed", "0.1"),
        ("acceleration", "1"),
        ("max-speed", "0.3"),
        ("start-value", "-1"),
    ]);
    assert_eq!(r.take(), [-1.0], "start value written on activation");
    r.input(true, true, 0);
    r.until(1000);
    // 0.1 -> 0.3 in 0.2 s (0.04 travel), then 0.8 s at 0.3 (0.24): 0.28 travel = 0.56.
    assert!(close(r.last(), -1.0 + 0.56), "{}", r.last());
    // A late tick still integrates the real time.
    r.engine.tick(r.at(1500), &mut r.out);
    assert!(close(r.last(), -1.0 + 0.56 + 0.3), "{}", r.last());
}

#[test]
fn stepped_moves_between_positions_without_wrapping() {
    let mut r = rig(&[
        ("motion", "stepped"),
        ("positions", "-1 -0.5 0 0.5 1"),
        ("start-value", "-1"),
    ]);
    r.take();
    r.tap(true, 0);
    r.tap(true, 10);
    r.tap(false, 20);
    assert_eq!(r.take(), [-0.5, 0.0, -0.5]);
    for t in 0..6 {
        r.tap(true, 100 + t * 10);
    }
    assert_eq!(r.take(), [0.0, 0.5, 1.0], "stays at the top");
    assert_eq!(r.engine.next_deadline(), None);
}

#[test]
fn encoder_steps_grow_with_the_rate_and_spring_back() {
    let mut r = rig(&[
        ("motion", "encoder"),
        ("step", "0.01"),
        ("max-step", "0.1"),
        ("fast-rate", "10"),
        ("spring-ms", "500"),
    ]);
    r.tap(true, 0);
    assert!(close(r.last(), 0.02), "one slow step: {}", r.last());
    // One press in 5 s: hardly bigger.
    r.tap(true, 5000);
    assert!(close(r.last(), 0.044), "{}", r.last());
    // 20 presses per second: the biggest step.
    r.tap(true, 5050);
    assert!(close(r.last(), 0.244), "{}", r.last());
    r.take();
    r.until(5540);
    assert!(r.take().is_empty());
    r.until(5560);
    assert_eq!(r.take(), [0.0], "the spring takes it back to the center");
    assert_eq!(r.engine.next_deadline(), None);
}

#[test]
fn smooth_follows_its_target() {
    let mut r = rig(&[
        ("motion", "smooth"),
        ("step", "0.25"),
        ("response-ms", "200"),
    ]);
    r.tap(true, 0);
    r.tap(true, 20);
    r.until(40);
    let early = r.last();
    assert!(early > 0.0 && early < 1.0, "on its way: {early}");
    r.until(400);
    assert!(
        close(r.last(), 1.0),
        "there in the response time: {}",
        r.last()
    );
    r.until(700);
    assert_eq!(r.last(), 1.0, "two steps of a quarter travel each");
    assert_eq!(r.engine.next_deadline(), None, "settled: no wake-ups");
    r.tap(false, 1000);
    r.until(1700);
    assert_eq!(r.last(), 0.5);
}

#[test]
fn gamepad_triggers_rest_when_let_go() {
    let mut r = rig(&[
        ("output-type", "gamepad"),
        ("control", "rt"),
        ("motion", "encoder"),
        ("step", "0.5"),
        ("max-step", "0.5"),
        ("spring-ms", "100"),
    ]);
    r.tap(true, 0);
    r.until(200);
    assert_eq!(r.out.pad, [("rt", 0.5), ("rt", 0.0)], "trigger values 0..1");
}

#[test]
fn stopping_the_profile_stops_the_motion() {
    let mut r = rig(&[("motion", "velocity")]);
    r.input(true, true, 0);
    r.until(100);
    r.engine.release_all(&mut r.out);
    assert_eq!(r.engine.next_deadline(), None);
}

#[test]
fn velocity_goes_on_when_one_of_two_buttons_is_let_go() {
    let mut r = rig(&[("motion", "velocity"), ("speed", "0.5")]);
    r.input(true, true, 0);
    r.input(false, true, 100);
    r.until(300);
    let held = r.last();
    r.input(false, false, 300);
    r.until(500);
    assert!(r.last() > held + 0.1, "up again: {} -> {}", held, r.last());
}

#[test]
fn set_value_with_both_buttons_held() {
    let mut r = rig(&[("set-value", "0.5"), ("return-on-release", "True")]);
    r.input(true, true, 0);
    r.input(false, true, 100);
    r.input(true, false, 200);
    assert_eq!(r.take(), [0.5, -0.5, -0.5], "down still held: its value");
    r.input(false, false, 300);
    assert_eq!(r.take(), [0.0], "both let go: back");
}

#[test]
fn the_profile_acquires_the_device_it_writes() {
    let r = rig(&[]);
    assert!(r.engine.vjoy_devices().contains(&1));
    let r = rig(&[("output-type", "gamepad"), ("gamepad-id", "2")]);
    assert!(r.engine.gamepads().contains(&2));
}

#[test]
fn the_decrease_input_goes_through_the_same_containers() {
    let cond = format!(
        "<condition>{}<comparator>{}{}</comparator><input>{}{}{}</input></condition>",
        prop("condition-type", "joystick"),
        prop("comparator-type", "pressed"),
        prop("is-pressed", "True"),
        prop("device-guid", DEV),
        prop("input-type", "button"),
        prop("input-id", "20"),
    );
    let body = prop("decrease-device-id", OTHER) + &prop("decrease-input-id", "1");
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><inputs><input><device-id>{DEV}</device-id>
        <input-type>button</input-type><mode>Default</mode><input-id>1</input-id>
        <action-configuration><root-action>r</root-action><behavior>button</behavior>
        </action-configuration></input></inputs><settings/><library>
        <action id="r" type="root"><actions><action-id>c</action-id></actions></action>
        <action id="c" type="condition">{}{cond}<true-actions><action-id>a</action-id></true-actions>
        <false-actions/></action>
        <action id="a" type="button-axis">{body}</action></library>
        <modes><mode>Default</mode></modes></profile>"#,
        prop("logical-operator", "all")
    );
    let p = gob_gremlin::from_str(&xml).unwrap();
    let mut r = Rig {
        engine: Engine::new(&p, &HashMap::new()),
        out: Recorder::default(),
        t0: Instant::now(),
    };
    r.tap(false, 0);
    assert!(
        r.take().is_empty(),
        "condition false: the decrease input does nothing"
    );
    r.engine.process(
        &InputKey::new(DEV, InputKind::Button, 20),
        Value::Button(true),
        r.at(100),
        &mut r.out,
    );
    r.tap(false, 200);
    assert_eq!(r.take(), [-1.0]);
}

/// A profile from its inputs and library, starting in `start`, activated.
fn profile_rig(inputs: &str, library: &str, modes: &str, start: &str) -> Rig {
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><inputs>{inputs}</inputs>
        <settings><startup-mode>{start}</startup-mode></settings><library>{library}</library>
        <modes>{modes}</modes></profile>"#
    );
    let p = gob_gremlin::from_str(&xml).unwrap();
    let mut r = Rig {
        engine: Engine::new(&p, &HashMap::new()),
        out: Recorder::default(),
        t0: Instant::now(),
    };
    r.engine.activate(&mut r.out);
    r
}

fn binding(device: &str, kind: &str, id: u32, mode: &str, root: &str) -> String {
    format!(
        "<input><device-id>{device}</device-id><input-type>{kind}</input-type><mode>{mode}</mode>\
         <input-id>{id}</input-id><action-configuration><root-action>{root}</root-action>\
         <behavior>{kind}</behavior></action-configuration></input>"
    )
}

#[test]
fn a_child_mode_binding_the_decrease_input_still_moves_the_axis_down() {
    // Default: button 1 sets the axis, OTHER button 1 sets its opposite. Combat inherits that
    // and binds OTHER button 1 itself (to nothing in particular).
    let axis = prop("decrease-device-id", OTHER)
        + &prop("decrease-input-id", "1")
        + &prop("motion", "set")
        + &prop("set-value", "1.0");
    let inputs =
        binding(DEV, "button", 1, "Default", "r") + &binding(OTHER, "button", 1, "Combat", "own");
    let library = format!(
        r#"<action id="r" type="root"><actions><action-id>a</action-id></actions></action>
        <action id="a" type="button-axis">{axis}</action>
        <action id="own" type="root"><actions/></action>"#
    );
    let modes = r#"<mode>Default</mode><mode parent="Default">Combat</mode>"#;
    let mut r = profile_rig(&inputs, &library, modes, "Combat");
    assert_eq!(r.engine.current_mode(), "Combat");
    r.tap(true, 0);
    assert_eq!(r.last(), 1.0);
    r.tap(false, 100);
    assert_eq!(
        r.last(),
        -1.0,
        "the inherited action's decrease input works in Combat"
    );
}

#[test]
fn a_gamepad_axis_steps_from_where_another_action_left_it() {
    // DEV axis 1 moves the gamepad's left stick X; button 1 steps the same stick by 0.1 (a
    // step is a share of the whole travel, -1..1).
    let pad = prop("gamepad-id", "1") + &prop("control", "left-x");
    let steps = prop("output-type", "gamepad")
        + &pad
        + &prop("motion", "encoder")
        + &prop("step", "0.05")
        + &prop("max-step", "0.05");
    let inputs =
        binding(DEV, "axis", 1, "Default", "ra") + &binding(DEV, "button", 1, "Default", "rb");
    let library = format!(
        r#"<action id="ra" type="root"><actions><action-id>m</action-id></actions></action>
        <action id="m" type="map-to-gamepad">{pad}</action>
        <action id="rb" type="root"><actions><action-id>b</action-id></actions></action>
        <action id="b" type="button-axis">{steps}</action>"#
    );
    let mut r = profile_rig(&inputs, &library, "<mode>Default</mode>", "Default");
    let now = r.at(0);
    r.engine.process(
        &InputKey::new(DEV, InputKind::Axis, 1),
        Value::Axis(0.8),
        now,
        &mut r.out,
    );
    r.tap(true, 100);
    let (control, v) = *r.out.pad.last().expect("the stick was written");
    assert_eq!(control, "left-x");
    assert!(
        close(v, 0.9),
        "one step up from 0.8, not from the center: {v}"
    );
}
