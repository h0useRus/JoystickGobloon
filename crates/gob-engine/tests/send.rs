//! Send modes of button outputs (hold, press, release, pulse, repeat, toggle) with explicit
//! times; output is recorded.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_engine::{Engine, InputKey, InputKind, Output, Value};

const DEV: &str = "aaaaaaaa-0000-0000-0000-000000000001";

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Button(u8, bool),
    Key(u16, bool),
    Mouse(gob_core::MouseButton, bool),
    Wheel(i32),
    Pad(&'static str, bool),
}

#[derive(Default)]
struct Recorder(Vec<Out>);

impl Output for Recorder {
    fn axis(&mut self, _device: u32, _axis: u8, _value: f32) {}
    fn button(&mut self, _device: u32, button: u8, pressed: bool) {
        self.0.push(Out::Button(button, pressed));
    }
    fn hat(&mut self, _device: u32, _hat: u8, _direction: Option<u16>) {}
    fn key(&mut self, scan_code: u16, _extended: bool, pressed: bool) {
        self.0.push(Out::Key(scan_code, pressed));
    }
    fn mouse_button(&mut self, button: gob_core::MouseButton, pressed: bool) {
        self.0.push(Out::Mouse(button, pressed));
    }
    fn mouse_wheel(&mut self, notches: i32) {
        self.0.push(Out::Wheel(notches));
    }
    fn gamepad_button(
        &mut self,
        _gamepad: u32,
        control: gob_engine::GamepadControl,
        pressed: bool,
    ) {
        self.0.push(Out::Pad(control.name(), pressed));
    }
}

fn prop(ty: &str, name: &str, value: &str) -> String {
    format!(r#"<property type="{ty}"><name>{name}</name><value>{value}</value></property>"#)
}

/// `body` plus the send properties; `activation` as the action's activation mode.
fn action(id: &str, kind: &str, body: &str, send: &str, activation: &str) -> String {
    format!(
        r#"<action id="{id}" type="{kind}">{body}{}{}{}{}{}</action>"#,
        prop("activation-mode", "activation-mode", activation),
        prop("string", "send-mode", send),
        prop("int", "send-pulse-ms", "50"),
        prop("int", "send-delay-ms", "300"),
        prop("int", "send-period-ms", "100"),
    )
}

fn vjoy_button(n: u32) -> String {
    format!(
        "{}{}{}{}",
        prop("int", "vjoy-device-id", "1"),
        prop("int", "vjoy-input-id", &n.to_string()),
        prop("input_type", "vjoy-input-type", "button"),
        prop("bool", "button-inverted", "False")
    )
}

/// One profile: button `n` runs action `a<n>` of `actions` (id, type, body, send, activation).
fn rig(actions: &[(&str, &str, String, &str, &str)]) -> Rig {
    let mut inputs = String::new();
    let mut library = String::new();
    for (i, (id, kind, body, send, activation)) in actions.iter().enumerate() {
        let n = i + 1;
        inputs += &format!(
            "<input><device-id>{DEV}</device-id><input-type>button</input-type><mode>Default</mode>\
             <input-id>{n}</input-id><action-configuration><root-action>r{n}</root-action>\
             <behavior>button</behavior></action-configuration></input>"
        );
        library += &format!(
            r#"<action id="r{n}" type="root"><actions><action-id>{id}</action-id></actions>{}</action>"#,
            prop("activation-mode", "activation-mode", "both")
        );
        library += &action(id, kind, body, send, activation);
    }
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><inputs>{inputs}</inputs><settings/><library>{library}</library>
        <modes><mode>Default</mode></modes></profile>"#
    );
    let p = gob_gremlin::from_str(&xml).unwrap();
    let own = actions.iter().any(|a| a.3 != "hold");
    assert_eq!(
        p.needs_own_format(),
        own,
        "send modes are Joystick Gobloon's own"
    );
    Rig {
        engine: Engine::new(&p, &HashMap::new()),
        out: Recorder::default(),
        t0: Instant::now(),
    }
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

    /// Button `n` goes down or up at `ms`.
    fn input(&mut self, n: u32, pressed: bool, ms: u64) {
        let now = self.at(ms);
        self.engine.process(
            &InputKey::new(DEV, InputKind::Button, n),
            Value::Button(pressed),
            now,
            &mut self.out,
        );
    }

    /// Runs the engine's timers up to `ms`, one tick per deadline like the runner.
    fn until(&mut self, ms: u64) {
        let end = self.at(ms);
        while let Some(d) = self.engine.next_deadline().filter(|&d| d <= end) {
            self.engine.tick(d, &mut self.out);
        }
        self.engine.tick(end, &mut self.out);
    }

    fn take(&mut self) -> Vec<Out> {
        std::mem::take(&mut self.out.0)
    }
}

fn vjoy(send: &str, activation: &str) -> Rig {
    rig(&[("a1", "map-to-vjoy", vjoy_button(5), send, activation)])
}

#[test]
fn hold_is_unchanged() {
    let mut r = vjoy("hold", "both");
    r.input(1, true, 0);
    r.input(1, false, 200);
    assert_eq!(r.take(), [Out::Button(5, true), Out::Button(5, false)]);
}

#[test]
fn press_and_release_only() {
    let mut r = rig(&[
        ("a1", "map-to-vjoy", vjoy_button(5), "press", "both"),
        ("a2", "map-to-vjoy", vjoy_button(5), "release", "both"),
    ]);
    r.input(1, true, 0);
    r.input(1, false, 100);
    assert_eq!(r.take(), [Out::Button(5, true)], "letting go keeps it down");
    r.input(2, true, 200);
    r.input(2, false, 300);
    assert_eq!(r.take(), [Out::Button(5, false)]);
}

#[test]
fn pulse_lasts_its_time_however_long_the_input_is_held() {
    let mut r = vjoy("pulse", "both");
    r.input(1, true, 0);
    r.until(40);
    assert_eq!(r.take(), [Out::Button(5, true)]);
    r.until(60);
    assert_eq!(r.take(), [Out::Button(5, false)], "up after 50 ms");
    r.input(1, false, 500);
    r.until(600);
    assert!(r.take().is_empty(), "the release does nothing");

    // A short tap still gives the whole pulse.
    r.input(1, true, 1000);
    r.input(1, false, 1005);
    r.until(1100);
    assert_eq!(r.take(), [Out::Button(5, true), Out::Button(5, false)]);
}

#[test]
fn pulse_on_release() {
    let mut r = vjoy("pulse", "release");
    r.input(1, true, 0);
    assert!(r.take().is_empty());
    r.input(1, false, 100);
    r.until(200);
    assert_eq!(r.take(), [Out::Button(5, true), Out::Button(5, false)]);
}

#[test]
fn repeat_while_held() {
    let mut r = vjoy("repeat", "both");
    r.input(1, true, 0);
    r.until(299);
    assert_eq!(
        r.take(),
        [Out::Button(5, true), Out::Button(5, false)],
        "first pulse"
    );
    r.until(450);
    // Second at the delay (300), third one period later (400).
    assert_eq!(
        r.take(),
        [
            Out::Button(5, true),
            Out::Button(5, false),
            Out::Button(5, true),
            Out::Button(5, false)
        ]
    );
    r.input(1, false, 520);
    r.until(2000);
    assert!(r.take().is_empty(), "stops when let go (it was up)");
    assert_eq!(r.engine.next_deadline(), None, "no timers left");

    // Let go during a pulse: it goes up at once.
    r.input(1, true, 3000);
    r.input(1, false, 3010);
    assert_eq!(r.take(), [Out::Button(5, true), Out::Button(5, false)]);
    assert_eq!(r.engine.next_deadline(), None);
}

#[test]
fn toggle_latches() {
    let mut r = vjoy("toggle", "both");
    r.input(1, true, 0);
    r.input(1, false, 100);
    assert_eq!(r.take(), [Out::Button(5, true)]);
    r.input(1, true, 200);
    r.input(1, false, 300);
    assert_eq!(r.take(), [Out::Button(5, false)]);
}

#[test]
fn keys_mouse_and_gamepad_follow_the_same_modes() {
    let keys = format!(
        "<input>{}{}</input><input>{}{}</input>",
        prop("int", "scan-code", "29"),
        prop("bool", "is-extended", "False"),
        prop("int", "scan-code", "30"),
        prop("bool", "is-extended", "False")
    );
    let mouse = |button: &str| {
        format!(
            "{}{}",
            prop("string", "mode", "Button"),
            prop("string", "button", button)
        )
    };
    let pad = format!(
        "{}{}{}",
        prop("int", "gamepad", "1"),
        prop("string", "control", "a"),
        prop("bool", "inverted", "False")
    );
    let mut r = rig(&[
        ("k", "map-to-keyboard", keys, "pulse", "both"),
        ("m", "map-to-mouse", mouse("Right"), "toggle", "both"),
        ("w", "map-to-mouse", mouse("Wheel Down"), "repeat", "both"),
        ("g", "map-to-gamepad", pad, "pulse", "both"),
    ]);
    r.input(1, true, 0);
    r.until(100);
    assert_eq!(
        r.take(),
        [
            Out::Key(29, true),
            Out::Key(30, true),
            Out::Key(30, false),
            Out::Key(29, false)
        ],
        "a key combination pulses as one, released in reverse"
    );
    r.input(2, true, 200);
    r.input(2, false, 250);
    r.input(2, true, 300);
    assert_eq!(
        r.take(),
        [
            Out::Mouse(gob_core::MouseButton::Right, true),
            Out::Mouse(gob_core::MouseButton::Right, false)
        ]
    );
    r.input(3, true, 1000);
    r.until(1450);
    r.input(3, false, 1450);
    assert_eq!(
        r.take(),
        [Out::Wheel(-1), Out::Wheel(-1), Out::Wheel(-1)],
        "one notch per pulse: at 0, 300 and 400 ms"
    );
    r.input(4, true, 2000);
    r.until(2100);
    assert_eq!(r.take(), [Out::Pad("a", true), Out::Pad("a", false)]);
}

#[test]
fn release_all_lets_go_of_everything() {
    let keys = format!(
        "<input>{}{}</input>",
        prop("int", "scan-code", "29"),
        prop("bool", "is-extended", "False")
    );
    let mut r = rig(&[
        ("a1", "map-to-vjoy", vjoy_button(5), "toggle", "both"),
        ("a2", "map-to-vjoy", vjoy_button(6), "hold", "both"),
        ("a3", "map-to-keyboard", keys, "hold", "both"),
        ("a4", "map-to-vjoy", vjoy_button(7), "repeat", "both"),
    ]);
    for n in 1..=4 {
        r.input(n, true, 0);
    }
    r.take();
    r.engine.release_all(&mut r.out);
    let mut released = r.take();
    released.sort_by_key(|o| format!("{o:?}"));
    assert_eq!(
        released,
        [
            Out::Button(5, false),
            Out::Button(6, false),
            Out::Button(7, false),
            Out::Key(29, false)
        ]
    );
    assert_eq!(r.engine.next_deadline(), None, "the repeat stopped");
}

#[test]
fn times_have_a_floor() {
    let action = format!(
        r#"<action id="a" type="map-to-vjoy">{}{}{}</action>"#,
        prop("string", "send-mode", "Repeat"),
        prop("int", "send-pulse-ms", "1"),
        prop("int", "send-period-ms", "0")
    );
    let a = gob_gremlin::from_str(&format!(
        r#"<profile app="Gobloon" version="1"><inputs/><settings/><library>{action}</library>
        <modes><mode>Default</mode></modes></profile>"#
    ))
    .unwrap();
    let send = a.library[0].button_send();
    assert_eq!(send.mode, gob_core::SendMode::Repeat, "names ignore case");
    assert_eq!(
        (send.pulse_ms, send.period_ms, send.delay_ms),
        (10, 10, 500)
    );
}
