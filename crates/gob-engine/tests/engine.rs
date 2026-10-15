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
    /// vJoy button on a device other than 1.
    DevButton(u32, u8, bool),
    Key(u16, bool),
}

#[derive(Default)]
struct Recorder(Vec<Out>);

impl Output for Recorder {
    fn axis(&mut self, _device: u32, axis: u8, value: f32) {
        self.0
            .push(Out::Axis(axis, (value * 1000.0).round() / 1000.0));
    }
    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        self.0.push(match device {
            1 => Out::Button(button, pressed),
            d => Out::DevButton(d, button, pressed),
        });
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
    fn key(&mut self, scan_code: u16, _extended: bool, pressed: bool) {
        self.0.push(Out::Key(scan_code, pressed));
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

fn macro_act(id: &str, repeat: &str, activation: &str, steps: &[String]) -> String {
    act_on(
        id,
        "macro",
        &format!(
            "{}{}{}{}",
            prop("string", "repeat-mode", repeat),
            prop("int", "repeat-count", "3"),
            prop("float", "repeat-delay", "0.1"),
            steps.concat()
        ),
        activation,
    )
}

fn step_vjoy(device: u32, button: u32, pressed: bool) -> String {
    format!(
        r#"<macro-action type="vjoy">{}{}{}{}</macro-action>"#,
        prop("int", "vjoy-id", &device.to_string()),
        prop("input_type", "input-type", "button"),
        prop("int", "input-id", &button.to_string()),
        prop("bool", "value", if pressed { "True" } else { "False" })
    )
}

fn step_key(scan_code: u16, pressed: bool) -> String {
    format!(
        r#"<macro-action type="key">{}{}{}</macro-action>"#,
        prop("int", "scan-code", &scan_code.to_string()),
        prop("bool", "is-extended", "False"),
        prop("bool", "is-pressed", if pressed { "True" } else { "False" })
    )
}

fn step_pause(seconds: &str) -> String {
    format!(
        r#"<macro-action type="pause">{}</macro-action>"#,
        prop("float", "duration", seconds)
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
        root("r-b10", &["mtempo"]),
        root("r-b11", &["mkeys"]),
        root("r-b12", &["mhold"]),
        root("r-b13", &["mtoggle"]),
        root("r-b14", &["mcount"]),
        root("r-b15", &["keys"]),
        root("r-b16", &["chain"]),
        root("r-b17", &["dtx"]),
        root("r-b18", &["dtc"]),
        root("r-h1", &["hb4"]),
        root("r-a6", &["merge"]),
        root("r-a8", &["delta"]),
        act(
            "delta",
            "axis-delta",
            &format!(
                "{}<positive-actions><action-id>v38</action-id></positive-actions>\
                 <negative-actions><action-id>v39</action-id></negative-actions>",
                prop("float", "threshold", "0.25")
            ),
        ),
        vjoy("v38", "button", 38),
        vjoy("v39", "button", 39),
        act(
            "merge",
            "merge-axis",
            &format!(
                "{}{}{}{}{}{}<actions><action-id>va2</action-id></actions>",
                prop("string", "label", "Pedals"),
                prop("uuid", "axis1-guid", &DEV.to_uppercase()),
                prop("int", "axis1-axis", "6"),
                prop("uuid", "axis2-guid", DEV),
                prop("int", "axis2-axis", "7"),
                prop("string", "operation", "bidirectional"),
            ),
        ),
        vjoy("va2", "axis", 2),
        root("r-h2", &["hb8"]),
        act(
            "hb4",
            "hat-buttons",
            &format!(
                "{}<North><action-id>v34</action-id></North><East><action-id>v35</action-id></East>                 <South/><West/><Center><action-id>v36</action-id></Center>",
                prop("int", "button-count", "4")
            ),
        ),
        act(
            "hb8",
            "hat-buttons",
            &format!(
                "{}<North><action-id>v34</action-id></North>                 <North-East><action-id>v37</action-id></North-East>",
                prop("int", "button-count", "8")
            ),
        ),
        vjoy("v34", "button", 34),
        vjoy("v35", "button", 35),
        vjoy("v36", "button", 36),
        vjoy("v37", "button", 37),
        act(
            "chain",
            "chain",
            &format!(
                "<chain-0><action-id>v30</action-id></chain-0>\
                 <chain-1><action-id>v31</action-id></chain-1>{}",
                prop("float", "timeout", "1.0")
            ),
        ),
        vjoy("v30", "button", 30),
        vjoy("v31", "button", 31),
        vjoy("v32", "button", 32),
        vjoy("v33", "button", 33),
        act(
            "dtx",
            "double-tap",
            &format!(
                "<single-actions><action-id>v32</action-id></single-actions>\
                 <double-actions><action-id>v33</action-id></double-actions>{}{}",
                prop("float", "threshold", "0.3"),
                prop("string", "activate-on", "exclusive")
            ),
        ),
        act(
            "dtc",
            "double-tap",
            &format!(
                "<single-actions><action-id>v32</action-id></single-actions>\
                 <double-actions><action-id>v33</action-id></double-actions>{}{}",
                prop("float", "threshold", "0.3"),
                prop("string", "activate-on", "combined")
            ),
        ),
        act(
            "keys",
            "map-to-keyboard",
            &format!(
                "<input>{}{}</input><input>{}{}</input>",
                prop("int", "scan-code", "29"),
                prop("bool", "is-extended", "False"),
                prop("int", "scan-code", "75"),
                prop("bool", "is-extended", "True")
            ),
        ),
        // Tap: vJoy button 26; hold: macro pulsing vJoy 2 button 46 on release
        // (Star Citizen scan / light amplification bind).
        act(
            "mtempo",
            "tempo",
            &format!(
                "<short-actions><action-id>v26</action-id></short-actions>\
                 <long-actions><action-id>mnight</action-id></long-actions>{}{}",
                prop("float", "threshold", "0.5"),
                prop("string", "activate-on", "release")
            ),
        ),
        vjoy("v26", "button", 26),
        macro_act(
            "mnight",
            "Single",
            "release",
            &[
                step_vjoy(2, 46, true),
                step_pause("0.1"),
                step_vjoy(2, 46, false),
            ],
        ),
        macro_act(
            "mkeys",
            "Single",
            "press",
            &[step_key(30, true), step_key(30, false)],
        ),
        macro_act(
            "mhold",
            "Hold",
            "press",
            &[step_vjoy(1, 40, true), step_vjoy(1, 40, false)],
        ),
        macro_act(
            "mtoggle",
            "Toggle",
            "press",
            &[step_vjoy(1, 41, true), step_vjoy(1, 41, false)],
        ),
        macro_act(
            "mcount",
            "Count",
            "press",
            &[
                step_vjoy(1, 42, true),
                step_pause("0.02"),
                step_vjoy(1, 42, false),
            ],
        ),
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
        input("button", 10, "Default", "r-b10", ""),
        input("button", 11, "Default", "r-b11", ""),
        input("button", 12, "Default", "r-b12", ""),
        input("button", 13, "Default", "r-b13", ""),
        input("button", 14, "Default", "r-b14", ""),
        input("button", 15, "Default", "r-b15", ""),
        input("button", 16, "Default", "r-b16", ""),
        input("button", 17, "Default", "r-b17", ""),
        input("button", 18, "Default", "r-b18", ""),
        input("hat", 1, "Default", "r-h1", ""),
        input("axis", 6, "Default", "r-a6", ""),
        input("axis", 7, "Default", "r-a6", ""),
        input("axis", 8, "Default", "r-a8", ""),
        input("hat", 2, "Default", "r-h2", ""),
        input("hat", 2, "Combat", "r-b1c", ""),
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

    fn hat(&mut self, id: u32, direction: Option<u16>) {
        let now = self.t0;
        self.engine.process(
            &InputKey::new(DEV, InputKind::Hat, id),
            Value::Hat(direction),
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

#[test]
fn tempo_long_press_runs_macro_on_release() {
    let mut r = Rig::new();
    assert!(
        r.engine.vjoy_devices().contains(&2),
        "macro-only vJoy device"
    );
    r.button(10, true, 0);
    r.tick(600);
    assert!(
        r.take().is_empty(),
        "release-activated macro waits for release"
    );
    r.button(10, false, 900);
    assert_eq!(r.take(), [Out::DevButton(2, 46, true)]);
    r.tick(950);
    assert!(r.take().is_empty());
    r.tick(1000);
    assert_eq!(r.take(), [Out::DevButton(2, 46, false)]);

    r.button(10, true, 2000);
    r.button(10, false, 2200);
    assert_eq!(r.take(), [Out::Button(26, true)]);
    r.tick(2300);
    assert_eq!(
        r.take(),
        [Out::Button(26, false)],
        "tap stays a short pulse"
    );
}

#[test]
fn macro_default_delay_and_queueing() {
    let mut r = Rig::new();
    r.button(11, true, 0);
    r.button(11, false, 5);
    assert_eq!(r.take(), [Out::Key(30, true)]);
    r.button(11, true, 10);
    r.button(11, false, 15);
    assert!(
        r.take().is_empty(),
        "second activation waits for the first run"
    );
    r.tick(49);
    assert!(r.take().is_empty());
    r.tick(50);
    assert_eq!(r.take(), [Out::Key(30, false), Out::Key(30, true)]);
    r.tick(100);
    assert_eq!(r.take(), [Out::Key(30, false)]);
    r.tick(1000);
    assert!(r.take().is_empty());
}

#[test]
fn macro_hold_repeats_until_release() {
    let mut r = Rig::new();
    r.button(12, true, 0);
    r.tick(50);
    r.tick(150);
    assert_eq!(
        r.take(),
        [
            Out::Button(40, true),
            Out::Button(40, false),
            Out::Button(40, true)
        ]
    );
    r.button(12, false, 160);
    r.tick(200);
    r.tick(400);
    assert_eq!(
        r.take(),
        [Out::Button(40, false)],
        "current round completes"
    );
}

#[test]
fn macro_toggle_stops_on_second_activation() {
    let mut r = Rig::new();
    r.button(13, true, 0);
    r.button(13, false, 10);
    r.tick(50);
    r.tick(150);
    assert_eq!(
        r.take(),
        [
            Out::Button(41, true),
            Out::Button(41, false),
            Out::Button(41, true)
        ]
    );
    r.button(13, true, 160);
    r.button(13, false, 170);
    r.tick(200);
    r.tick(400);
    assert_eq!(r.take(), [Out::Button(41, false)]);
    r.button(13, true, 500);
    assert_eq!(r.take(), [Out::Button(41, true)], "restarts afterwards");
}

#[test]
fn macro_count_runs_n_times() {
    let mut r = Rig::new();
    r.button(14, true, 0);
    r.button(14, false, 5);
    for ms in (0..1000).step_by(5) {
        r.tick(ms);
    }
    let presses = r
        .take()
        .iter()
        .filter(|o| **o == Out::Button(42, true))
        .count();
    assert_eq!(presses, 3);
}

#[test]
fn map_to_keyboard_presses_in_order_releases_in_reverse() {
    let mut r = Rig::new();
    r.button(15, true, 0);
    assert_eq!(r.take(), [Out::Key(29, true), Out::Key(75, true)]);
    r.button(15, false, 10);
    assert_eq!(r.take(), [Out::Key(75, false), Out::Key(29, false)]);

    // Let go in another mode: keys are released once.
    r.button(15, true, 20);
    r.button(2, true, 30);
    r.take();
    r.button(15, false, 40);
    assert_eq!(r.take(), [Out::Key(75, false), Out::Key(29, false)]);
}

#[test]
fn chain_cycles_and_restarts_after_timeout() {
    let mut r = Rig::new();
    let tap = |r: &mut Rig, ms: u64| {
        r.button(16, true, ms);
        r.button(16, false, ms + 10);
        r.take()
    };
    assert_eq!(
        tap(&mut r, 0),
        [Out::Button(30, true), Out::Button(30, false)]
    );
    assert_eq!(
        tap(&mut r, 100),
        [Out::Button(31, true), Out::Button(31, false)]
    );
    assert_eq!(
        tap(&mut r, 200),
        [Out::Button(30, true), Out::Button(30, false)]
    );
    // Next would be 31, but more than a second passed.
    assert_eq!(
        tap(&mut r, 2000),
        [Out::Button(30, true), Out::Button(30, false)]
    );
}

#[test]
fn double_tap_exclusive() {
    let mut r = Rig::new();
    // Single tap: pulse after the threshold.
    r.button(17, true, 0);
    r.button(17, false, 50);
    r.tick(299);
    assert!(r.take().is_empty());
    r.tick(300);
    assert_eq!(r.take(), [Out::Button(32, true)]);
    r.tick(350);
    assert_eq!(r.take(), [Out::Button(32, false)]);
    // Single hold: press after the threshold, release with the button.
    r.button(17, true, 1000);
    r.tick(1300);
    assert_eq!(r.take(), [Out::Button(32, true)]);
    r.button(17, false, 1500);
    assert_eq!(r.take(), [Out::Button(32, false)]);
    // Double tap: only the double action.
    r.button(17, true, 2000);
    r.button(17, false, 2050);
    r.button(17, true, 2100);
    assert_eq!(r.take(), [Out::Button(33, true)]);
    r.button(17, false, 2400);
    r.tick(3000);
    assert_eq!(r.take(), [Out::Button(33, false)]);
}

#[test]
fn double_tap_combined() {
    let mut r = Rig::new();
    r.button(18, true, 0);
    r.button(18, false, 50);
    r.button(18, true, 100);
    r.button(18, false, 150);
    r.tick(1000);
    assert_eq!(
        r.take(),
        [
            Out::Button(32, true),
            Out::Button(32, false),
            Out::Button(32, true),
            Out::Button(33, true),
            Out::Button(32, false),
            Out::Button(33, false),
        ]
    );
}

#[test]
fn hat_buttons_four_and_eight_directions() {
    let mut r = Rig::new();
    r.hat(1, None);
    assert_eq!(r.take(), [Out::Button(36, true)]);
    r.hat(1, Some(0));
    assert_eq!(r.take(), [Out::Button(34, true), Out::Button(36, false)]);
    // Four buttons: a diagonal is none of them.
    r.hat(1, Some(4500));
    assert_eq!(r.take(), [Out::Button(34, false)]);
    r.hat(1, Some(9000));
    assert_eq!(r.take(), [Out::Button(35, true)]);
    r.hat(1, None);
    assert_eq!(r.take(), [Out::Button(35, false), Out::Button(36, true)]);

    r.hat(2, Some(4500));
    assert_eq!(r.take(), [Out::Button(37, true)]);
    r.hat(2, Some(0));
    assert_eq!(r.take(), [Out::Button(34, true), Out::Button(37, false)]);
}

#[test]
fn hat_buttons_release_when_centered_in_another_mode() {
    let mut r = Rig::new();
    r.hat(2, Some(0));
    assert_eq!(r.take(), [Out::Button(34, true)]);
    // Hat 2 maps to vJoy button 5 in Combat; centering there still releases button 34.
    r.button(2, true, 10);
    r.hat(2, None);
    assert_eq!(r.take(), [Out::Button(5, false), Out::Button(34, false)]);
}

#[test]
fn merge_axis_combines_both_inputs() {
    let mut r = Rig::new();
    r.axis(6, 1.0);
    assert_eq!(r.take(), [Out::Axis(2, -0.5)]);
    r.axis(7, 1.0);
    assert_eq!(r.take(), [Out::Axis(2, 0.0)]);
    r.axis(6, -1.0);
    assert_eq!(r.take(), [Out::Axis(2, 1.0)]);
}

#[test]
fn merge_operations() {
    use gob_core::MergeOp::*;
    assert_eq!(Average.apply(0.2, 0.6), 0.4);
    assert_eq!(Sum.apply(0.8, 0.6), 1.0);
    assert_eq!(MaximumDeflection.apply(-0.9, 0.5), -0.9);
    assert_eq!(ClosestToCenter.apply(0.5, -0.5), -0.5);
    assert_eq!(
        gob_core::MergeOp::parse("Maximum-Deflection"),
        Some(MaximumDeflection)
    );
}

#[test]
fn axis_delta_pulses_per_threshold() {
    let mut r = Rig::new();
    r.axis_at(8, 0.1, 0); // first value only sets the reference
    r.axis_at(8, 0.2, 10);
    assert!(r.take().is_empty());
    r.axis_at(8, 0.4, 20); // +0.3 accumulated
    assert_eq!(r.take(), [Out::Button(38, true)]);
    r.tick(70);
    assert_eq!(r.take(), [Out::Button(38, false)]);
    r.axis_at(8, 0.3, 100);
    r.axis_at(8, 0.1, 110); // -0.3
    assert_eq!(r.take(), [Out::Button(39, true)]);
}
