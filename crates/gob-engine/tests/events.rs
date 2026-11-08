//! Profile events (start, stop, mode enter and leave) and buttons that send their state at
//! start, with explicit times; output is recorded.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_core::{EVENTS_GUID, ProfileEvent};
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

/// A root running a vJoy button `n` (id `v<n>`).
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

/// A root switching to `mode` on press (id `r<id>`).
fn switch(id: &str, mode: &str) -> String {
    format!(
        r#"<action id="m{id}" type="change-mode">{}<target-mode>{}</target-mode>{}</action>
        <action id="r{id}" type="root"><actions><action-id>m{id}</action-id></actions></action>"#,
        prop("string", "change-type", "Switch"),
        prop("string", "name", mode),
        prop("activation-mode", "activation-mode", "press"),
    )
}

fn binding(device: &str, kind: &str, id: u32, mode: &str, root: &str, attrs: &str) -> String {
    format!(
        "<input><device-id>{device}</device-id><input-type>{kind}</input-type><mode>{mode}</mode>\
         <input-id>{id}</input-id><action-configuration{attrs}><root-action>{root}</root-action>\
         <behavior>button</behavior></action-configuration></input>"
    )
}

fn event(e: ProfileEvent, mode: &str, root: &str) -> String {
    binding(EVENTS_GUID, "event", e.id(), mode, root, "")
}

fn button(id: u32, mode: &str, root: &str, attrs: &str) -> String {
    binding(DEV, "button", id, mode, root, attrs)
}

fn engine(inputs: &[String], library: &[String]) -> Engine {
    let xml = format!(
        r#"<profile version="14"><inputs>{}</inputs>
        <settings><startup-mode>Default</startup-mode></settings><library>{}</library>
        <modes><mode>Default</mode><mode parent="Default">Combat</mode><mode>Other</mode></modes>
        </profile>"#,
        inputs.concat(),
        library.concat()
    );
    Engine::new(&gob_gremlin::from_str(&xml).unwrap(), &HashMap::new())
}

fn press(e: &mut Engine, id: u32, pressed: bool, now: Instant, out: &mut Recorder) {
    let key = InputKey::new(DEV, InputKind::Button, id);
    e.process(&key, Value::Button(pressed), now, out);
}

#[test]
fn start_runs_start_and_enter_then_lets_go() {
    let mut e = engine(
        &[
            event(ProfileEvent::Start, "Default", "r1"),
            event(ProfileEvent::Enter, "Default", "r2"),
        ],
        &[vjoy(1), vjoy(2)],
    );
    let (t0, mut out) = (Instant::now(), Recorder::default());
    e.profile_started(t0, &mut out);
    assert_eq!(out.0, [(1, true), (2, true)]);
    assert!(e.has_timers());
    e.tick(t0 + Duration::from_millis(50), &mut out);
    assert_eq!(out.0.len(), 2, "held for a moment");
    e.tick(t0 + Duration::from_millis(100), &mut out);
    assert_eq!(out.0[2..], [(1, false), (2, false)]);
    assert!(!e.has_timers());
}

/// Leave runs in the mode left, enter in the one entered; a child mode without its own uses
/// its parent's.
#[test]
fn mode_change_runs_leave_then_enter() {
    let mut e = engine(
        &[
            event(ProfileEvent::Enter, "Default", "r1"),
            event(ProfileEvent::Leave, "Default", "r2"),
            event(ProfileEvent::Enter, "Other", "r3"),
            button(1, "Default", "rc", ""),
            button(1, "Combat", "ro", ""),
            button(1, "Other", "rd", ""),
        ],
        &[
            vjoy(1),
            vjoy(2),
            vjoy(3),
            switch("c", "Combat"),
            switch("o", "Other"),
            switch("d", "Default"),
        ],
    );
    let (t0, mut out) = (Instant::now(), Recorder::default());
    press(&mut e, 1, true, t0, &mut out);
    assert_eq!(e.current_mode(), "Combat");
    // Combat inherits Default's enter and leave.
    assert_eq!(out.0, [(2, true), (1, true)]);
    out.0.clear();
    press(&mut e, 1, false, t0, &mut out);
    press(&mut e, 1, true, t0 + Duration::from_millis(10), &mut out);
    assert_eq!(e.current_mode(), "Other");
    // A press still down goes up first (the mode change may release it too; the runner sends
    // a value once); Other has its own enter and no leave.
    let presses: Vec<_> = out.0.iter().filter(|o| o.1).copied().collect();
    assert_eq!(presses, [(2, true), (3, true)]);
    let first_press = out.0.iter().position(|o| o.1).unwrap();
    assert!(out.0[..first_press].contains(&(2, false)), "{:?}", out.0);
    assert!(
        out.0.contains(&(1, false)),
        "Combat's enter press went up: {:?}",
        out.0
    );
    out.0.clear();
    e.tick(t0 + Duration::from_millis(200), &mut out);
    assert!(
        out.0.contains(&(2, false)) && out.0.contains(&(3, false)),
        "{:?}",
        out.0
    );
    assert!(!e.has_timers());
}

#[test]
fn stop_runs_leave_and_stop_in_the_current_mode() {
    let mut e = engine(
        &[
            event(ProfileEvent::Leave, "Default", "r1"),
            event(ProfileEvent::Stop, "Default", "r2"),
        ],
        &[vjoy(1), vjoy(2)],
    );
    let (t0, mut out) = (Instant::now(), Recorder::default());
    assert!(e.profile_stopping(t0, &mut out));
    assert_eq!(out.0, [(1, true), (2, true)]);
    e.tick(t0 + Duration::from_millis(100), &mut out);
    assert_eq!(out.0[2..], [(1, false), (2, false)]);

    let mut none = engine(&[button(1, "Default", "r1", "")], &[vjoy(1)]);
    assert!(!none.profile_stopping(t0, &mut out), "nothing bound");
    assert!(!none.has_timers());
}

/// Enter actions that switch modes run the next mode's events too, up to a limit.
#[test]
fn mode_events_switching_modes_stop_at_a_limit() {
    let mut e = engine(
        &[
            event(ProfileEvent::Enter, "Default", "rc"),
            event(ProfileEvent::Enter, "Combat", "rd"),
            button(1, "Default", "rc", ""),
        ],
        &[switch("c", "Combat"), switch("d", "Default")],
    );
    let (t0, mut out) = (Instant::now(), Recorder::default());
    press(&mut e, 1, true, t0, &mut out);
    assert!(["Default", "Combat"].contains(&e.current_mode()));
}

#[test]
fn held_buttons_send_their_state_at_start() {
    let mut e = engine(
        &[
            button(7, "Default", "r7", r#" send-state="true""#),
            button(8, "Default", "r8", ""),
            button(9, "Default", "r9", r#" send-state="true""#),
        ],
        &[vjoy(7), vjoy(8), vjoy(9)],
    );
    for (id, down) in [(7, true), (8, true), (9, false)] {
        e.seed(
            &InputKey::new(DEV, InputKind::Button, id),
            Value::Button(down),
        );
    }
    let (t0, mut out) = (Instant::now(), Recorder::default());
    e.profile_started(t0, &mut out);
    assert_eq!(out.0, [(7, true)], "held and sending only");
    press(&mut e, 7, false, t0 + Duration::from_millis(10), &mut out);
    assert_eq!(out.0[1..], [(7, false)], "its release as usual");
}
