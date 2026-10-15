//! Engine behavior on a hand-written profile; output is recorded instead of sent to vJoy.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_engine::{Engine, InputKey, InputKind, Output, Value};

const DEV: &str = "aaaaaaaa-0000-0000-0000-000000000001";

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Axis(u8, f32),
    Button(u8, bool),
    Move(i32, i32),
    Mouse(gob_core::MouseButton, bool),
    Wheel(i32),
    Run(String, Vec<String>),
}

#[derive(Default)]
struct Recorder(Vec<Out>);

impl Output for Recorder {
    fn axis(&mut self, _device: u32, axis: u8, value: f32) {
        self.0
            .push(Out::Axis(axis, (value * 1000.0).round() / 1000.0));
    }
    fn button(&mut self, _device: u32, button: u8, pressed: bool) {
        self.0.push(Out::Button(button, pressed));
    }
    fn hat(&mut self, _device: u32, _hat: u8, _direction: Option<u16>) {}
    fn mouse_move(&mut self, dx: i32, dy: i32) {
        self.0.push(Out::Move(dx, dy));
    }
    fn mouse_button(&mut self, button: gob_core::MouseButton, pressed: bool) {
        self.0.push(Out::Mouse(button, pressed));
    }
    fn mouse_wheel(&mut self, notches: i32) {
        self.0.push(Out::Wheel(notches));
    }
    fn run_command(&mut self, executable: &str, arguments: &[String]) {
        self.0.push(Out::Run(executable.into(), arguments.to_vec()));
    }
}

fn prop(ty: &str, name: &str, value: &str) -> String {
    format!(r#"<property type="{ty}"><name>{name}</name><value>{value}</value></property>"#)
}

fn act(id: &str, kind: &str, body: &str) -> String {
    act_on(id, kind, body, "both")
}

fn act_on(id: &str, kind: &str, body: &str, activation: &str) -> String {
    format!(
        r#"<action id="{id}" type="{kind}">{body}{}</action>"#,
        prop("activation-mode", "activation-mode", activation)
    )
}

fn root(id: &str, children: &[&str]) -> String {
    let refs: String = children
        .iter()
        .map(|c| format!("<action-id>{c}</action-id>"))
        .collect();
    act(id, "root", &format!("<actions>{refs}</actions>"))
}

fn vjoy(id: &str, kind: &str, n: u32) -> String {
    act(
        id,
        "map-to-vjoy",
        &format!(
            "{}{}{}{}",
            prop("int", "vjoy-device-id", "1"),
            prop("int", "vjoy-input-id", &n.to_string()),
            prop("input_type", "vjoy-input-type", kind),
            prop("bool", "button-inverted", "False")
        ),
    )
}

fn change_mode(id: &str, change: &str, targets: &[&str], activation: &str) -> String {
    let t: String = targets
        .iter()
        .map(|m| format!("<target-mode>{}</target-mode>", prop("string", "name", m)))
        .collect();
    act_on(
        id,
        "change-mode",
        &format!("{}{t}", prop("string", "change-type", change)),
        activation,
    )
}

fn input(kind: &str, id: u32, mode: &str, root: &str, extra: &str) -> String {
    let behavior = if extra.is_empty() { kind } else { "button" };
    format!(
        "<input><device-id>{DEV}</device-id><input-type>{kind}</input-type><mode>{mode}</mode>\
         <input-id>{id}</input-id><action-configuration><root-action>{root}</root-action>\
         <behavior>{behavior}</behavior>{extra}</action-configuration></input>"
    )
}

fn profile() -> String {
    let curve = act(
        "curve",
        "response-curve",
        &format!(
            "<deadzone>{}{}{}{}</deadzone><control-points>{}{}</control-points>{}",
            prop("float", "low", "-1.0"),
            prop("float", "center-low", "-0.1"),
            prop("float", "center-high", "0.1"),
            prop("float", "high", "1.0"),
            prop("point2d", "point", "-1.0,-1.0"),
            prop("point2d", "point", "1.0,1.0"),
            prop("string", "curve-type", "PiecewiseLinear")
        ),
    );
    let tempo = act(
        "tempo",
        "tempo",
        &format!(
            "<short-actions><action-id>v10</action-id></short-actions>\
             <long-actions><action-id>v11</action-id></long-actions>{}{}",
            prop("float", "threshold", "0.5"),
            prop("string", "activate-on", "release")
        ),
    );
    let vb = "<virtual-button><lower-limit>0.5</lower-limit><upper-limit>1.0</upper-limit>\
              <axis-button-direction>anywhere</axis-button-direction></virtual-button>";
    let library = [
        root("r-b1", &["v1"]),
        root("r-b2", &["temp"]),
        root("r-b1c", &["v5"]),
        root("r-a1", &["curve", "va1"]),
        root("r-b3", &["tempo"]),
        root("r-a2", &["v20"]),
        root("r-b4", &["cycle"]),
        root("r-a3", &["vrel"]),
        root("r-b5", &["mlb"]),
        root("r-b6", &["mwu"]),
        root("r-a4", &["mmove"]),
        root("r-b7", &["toggle"]),
        root("r-b8", &["pause"]),
        root("r-a5", &["split"]),
        root("r-b9", &["run"]),
        act(
            "toggle",
            "smart-toggle",
            &format!(
                "<actions><action-id>v1</action-id></actions>{}",
                prop("float", "delay", "0.5")
            ),
        ),
        act_on(
            "pause",
            "pause-resume",
            &prop("string", "operation", "Toggle"),
            "press",
        ),
        act(
            "split",
            "split-axis",
            &format!(
                "{}<lower-actions><action-id>vsplit</action-id></lower-actions>\
                 <upper-actions><action-id>vsplit</action-id></upper-actions>",
                prop("float", "split-value", "0.0")
            ),
        ),
        vjoy("vsplit", "axis", 2),
        act_on(
            "run",
            "run-command",
            &format!(
                "{}{}",
                prop("string", "executable", "notepad.exe"),
                prop("string", "arguments", "a \"b c\"")
            ),
            "press",
        ),
        act(
            "mlb",
            "map-to-mouse",
            &format!(
                "{}{}",
                prop("string", "mode", "Button"),
                prop("string", "button", "Left")
            ),
        ),
        act(
            "mwu",
            "map-to-mouse",
            &format!(
                "{}{}",
                prop("string", "mode", "Button"),
                prop("string", "button", "Wheel Up")
            ),
        ),
        act(
            "mmove",
            "map-to-mouse",
            &format!(
                "{}{}{}{}{}",
                prop("string", "mode", "Motion"),
                prop("int", "direction", "90"),
                prop("int", "min-speed", "0"),
                prop("int", "max-speed", "1000"),
                prop("float", "time-to-max-speed", "1.0")
            ),
        ),
        act(
            "vrel",
            "map-to-vjoy",
            &format!(
                "{}{}{}{}{}",
                prop("int", "vjoy-device-id", "1"),
                prop("int", "vjoy-input-id", "3"),
                prop("input_type", "vjoy-input-type", "axis"),
                prop("axis_mode", "axis-mode", "relative"),
                prop("float", "axis-scaling", "1.0")
            ),
        ),
        vjoy("v1", "button", 1),
        vjoy("v5", "button", 5),
        vjoy("v10", "button", 10),
        vjoy("v11", "button", 11),
        vjoy("v20", "button", 20),
        vjoy("va1", "axis", 1),
        change_mode("temp", "Temporary", &["Combat"], "both"),
        change_mode("cycle", "Cycle", &["Combat", "Default"], "press"),
        curve,
        tempo,
    ]
    .concat();
    let inputs = [
        input("button", 1, "Default", "r-b1", ""),
        input("button", 2, "Default", "r-b2", ""),
        input("button", 1, "Combat", "r-b1c", ""),
        input("axis", 1, "Default", "r-a1", ""),
        input("button", 3, "Default", "r-b3", ""),
        input("axis", 2, "Default", "r-a2", vb),
        input("button", 4, "Default", "r-b4", ""),
        input("axis", 3, "Default", "r-a3", ""),
        input("button", 5, "Default", "r-b5", ""),
        input("button", 6, "Default", "r-b6", ""),
        input("axis", 4, "Default", "r-a4", ""),
        input("button", 7, "Default", "r-b7", ""),
        input("button", 8, "Default", "r-b8", ""),
        input("axis", 5, "Default", "r-a5", ""),
        input("button", 9, "Default", "r-b9", ""),
    ]
    .concat();
    format!(
        r#"<profile version="14"><inputs>{inputs}</inputs>
        <settings><startup-mode>Default</startup-mode></settings><library>{library}</library>
        <modes><mode>Default</mode><mode parent="Default">Combat</mode></modes></profile>"#
    )
}

struct Rig {
    engine: Engine,
    out: Recorder,
    t0: Instant,
}

impl Rig {
    fn new() -> Self {
        let p = gob_gremlin::from_str(&profile()).unwrap();
        Self {
            engine: Engine::new(&p, &HashMap::new()),
            out: Recorder::default(),
            t0: Instant::now(),
        }
    }

    fn at(&self, ms: u64) -> Instant {
        self.t0 + Duration::from_millis(ms)
    }

    fn button(&mut self, id: u32, pressed: bool, ms: u64) {
        let now = self.at(ms);
        self.engine.process(
            &InputKey::new(DEV, InputKind::Button, id),
            Value::Button(pressed),
            now,
            &mut self.out,
        );
    }

    fn axis(&mut self, id: u32, value: f32) {
        let now = self.t0;
        self.engine.process(
            &InputKey::new(DEV, InputKind::Axis, id),
            Value::Axis(value),
            now,
            &mut self.out,
        );
    }

    fn axis_at(&mut self, id: u32, value: f32, ms: u64) {
        let now = self.at(ms);
        self.engine.process(
            &InputKey::new(DEV, InputKind::Axis, id),
            Value::Axis(value),
            now,
            &mut self.out,
        );
    }

    fn tick(&mut self, ms: u64) {
        let now = self.at(ms);
        self.engine.tick(now, &mut self.out);
    }

    fn take(&mut self) -> Vec<Out> {
        std::mem::take(&mut self.out.0)
    }
}

#[test]
fn button_maps_to_vjoy() {
    let mut r = Rig::new();
    r.button(1, true, 0);
    r.button(1, false, 10);
    assert_eq!(r.take(), [Out::Button(1, true), Out::Button(1, false)]);
}

#[test]
fn temporary_mode_with_auto_release() {
    let mut r = Rig::new();
    r.button(2, true, 0);
    assert_eq!(r.engine.current_mode(), "Combat");
    r.button(1, true, 10);
    assert_eq!(
        r.take(),
        [Out::Button(5, true)],
        "Combat overrides button 1"
    );
    r.button(2, false, 20);
    assert_eq!(r.engine.current_mode(), "Default");
    r.take();
    // Released in Default: Default's binding releases vJoy 1, and the vJoy 5
    // press from Combat is auto-released because the mode changed.
    r.button(1, false, 30);
    let out = r.take();
    assert!(out.contains(&Out::Button(1, false)), "{out:?}");
    assert!(out.contains(&Out::Button(5, false)), "{out:?}");
}

#[test]
fn child_mode_inherits_parent_bindings_and_axes_refresh() {
    let mut r = Rig::new();
    r.axis(1, 0.55);
    assert_eq!(
        r.take(),
        [Out::Axis(1, 0.5)],
        "deadzone 0.1 rescales 0.55 to 0.5"
    );
    r.button(2, true, 0);
    assert_eq!(
        r.take(),
        [Out::Axis(1, 0.5)],
        "axis re-sent after mode change, inherited in Combat"
    );
    r.axis(1, 0.05);
    assert_eq!(r.take(), [Out::Axis(1, 0.0)], "inside the center deadzone");
}

#[test]
fn tempo_short_press_is_a_pulse() {
    let mut r = Rig::new();
    r.button(3, true, 0);
    r.button(3, false, 200);
    assert_eq!(r.take(), [Out::Button(10, true)]);
    r.tick(300);
    assert_eq!(r.take(), [Out::Button(10, false)]);
    r.tick(1000);
    assert!(
        r.take().is_empty(),
        "long action must not fire after a short press"
    );
}

#[test]
fn tempo_long_press() {
    let mut r = Rig::new();
    r.button(3, true, 0);
    r.tick(400);
    assert!(r.take().is_empty());
    r.tick(600);
    assert_eq!(r.take(), [Out::Button(11, true)]);
    r.button(3, false, 900);
    assert_eq!(r.take(), [Out::Button(11, false)]);
}

#[test]
fn axis_as_virtual_button() {
    let mut r = Rig::new();
    r.axis(2, 0.0);
    r.axis(2, 0.7);
    r.axis(2, 0.8);
    r.axis(2, 0.2);
    assert_eq!(r.take(), [Out::Button(20, true), Out::Button(20, false)]);
}

#[test]
fn cycle_mode_on_press() {
    let mut r = Rig::new();
    r.button(4, true, 0);
    assert_eq!(r.engine.current_mode(), "Combat");
    r.button(4, false, 10);
    assert_eq!(r.engine.current_mode(), "Combat", "release does not cycle");
    r.button(4, true, 20);
    assert_eq!(r.engine.current_mode(), "Default");
}

#[test]
fn vkb_profile_compiles_and_runs() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/local/vkb_nxt_r15.xml"
    );
    let Ok(p) = gob_gremlin::load(path) else {
        return;
    };
    let mut engine = Engine::new(&p, &HashMap::new());
    assert_eq!(engine.current_mode(), "SCM Mode");
    assert!(engine.vjoy_devices().contains(&1));
    let mut out = Recorder::default();
    let dev = &p.devices[0].id;
    let now = Instant::now();
    for id in 1..=32 {
        engine.process(
            &InputKey::new(dev, InputKind::Button, id),
            Value::Button(true),
            now,
            &mut out,
        );
        engine.process(
            &InputKey::new(dev, InputKind::Button, id),
            Value::Button(false),
            now,
            &mut out,
        );
        engine.tick(now + Duration::from_secs(1), &mut out);
    }
    assert!(
        !out.0.is_empty(),
        "pressing VKB buttons must produce vJoy output"
    );
}

#[test]
fn relative_axis_moves_by_speed() {
    let mut r = Rig::new();
    r.axis_at(3, 0.5, 0);
    assert!(
        r.take().is_empty(),
        "relative mode does not set the position directly"
    );
    assert!(
        r.engine.next_deadline().is_some(),
        "needs ticks while moving"
    );
    r.tick(100);
    // 10 steps of 0.5 * 1.0 * 0.001
    assert_eq!(r.take(), [Out::Axis(3, 0.005)]);
    r.axis_at(3, 0.0, 100);
    r.tick(1200);
    assert_eq!(
        r.take(),
        [Out::Axis(3, 0.005)],
        "holds its value when centered"
    );
    r.tick(1300);
    assert!(
        r.take().is_empty(),
        "stops a second after returning to center"
    );
    assert!(r.engine.next_deadline().is_none());
}

#[test]
fn map_to_mouse_buttons_wheel_and_motion() {
    use gob_core::MouseButton;
    let mut r = Rig::new();
    r.button(5, true, 0);
    r.button(5, false, 10);
    r.button(6, true, 20);
    r.button(6, false, 30);
    assert_eq!(
        r.take(),
        [
            Out::Mouse(MouseButton::Left, true),
            Out::Mouse(MouseButton::Left, false),
            Out::Wheel(1)
        ]
    );
    // Axis at 0.5 with 0..1000 px/s to the right: 500 px/s, 5 px per 10 ms step.
    r.axis_at(4, 0.5, 0);
    r.tick(10);
    r.tick(20);
    assert_eq!(r.take(), [Out::Move(5, 0), Out::Move(5, 0)]);
    r.axis_at(4, 0.0, 20);
    r.tick(30);
    assert!(r.take().is_empty(), "centered axis stops the cursor");
    assert!(r.engine.next_deadline().is_none());
}

#[test]
fn smart_toggle_latches_on_short_press() {
    let mut r = Rig::new();
    r.button(7, true, 0);
    r.button(7, false, 100);
    assert_eq!(r.take(), [Out::Button(1, true)], "short press latches");
    r.tick(600);
    r.button(7, true, 1000);
    assert!(r.take().is_empty());
    r.button(7, false, 1100);
    assert_eq!(r.take(), [Out::Button(1, false)], "next press releases");
    // Held past the delay: momentary.
    r.button(7, true, 2000);
    r.tick(2600);
    r.button(7, false, 3000);
    assert_eq!(r.take(), [Out::Button(1, true), Out::Button(1, false)]);
}

#[test]
fn pause_toggle_stops_other_bindings() {
    let mut r = Rig::new();
    r.button(8, true, 0);
    r.button(8, false, 10);
    assert!(r.engine.is_paused());
    r.button(1, true, 20);
    assert!(r.take().is_empty(), "paused");
    r.button(8, true, 30);
    assert!(
        !r.engine.is_paused(),
        "pause-resume still runs while paused"
    );
    r.button(1, true, 40);
    assert_eq!(r.take(), [Out::Button(1, true)]);
}

#[test]
fn split_axis_rescales_halves() {
    let mut r = Rig::new();
    r.axis(5, 0.5);
    r.axis(5, -1.0);
    r.axis(5, -0.5);
    // Upper half 0..1 -> -1..1; lower half -1..0 -> 1..-1.
    assert_eq!(
        r.take(),
        [Out::Axis(2, 0.0), Out::Axis(2, 1.0), Out::Axis(2, 0.0)]
    );
}

#[test]
fn run_command_on_press() {
    let mut r = Rig::new();
    r.button(9, true, 0);
    r.button(9, false, 10);
    assert_eq!(
        r.take(),
        [Out::Run(
            "notepad.exe".into(),
            vec!["a".into(), "b c".into()]
        )]
    );
}
