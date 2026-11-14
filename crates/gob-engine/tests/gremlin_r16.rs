//! Joystick Gremlin R16.1's mode change interaction tests (test/action_interaction in
//! ref/JoystickGremlin) run on the engine with Gremlin's own profiles: inputs and outputs are
//! the profile's logical device, addressed by label. Skipped when ref/ is not cloned.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gob_core::{InputType, LOGICAL_DEVICE_GUID};
use gob_engine::{Engine, InputKey, InputKind, Output, Value};

struct Null;
impl Output for Null {
    fn axis(&mut self, _: u32, _: u8, _: f32) {}
    fn button(&mut self, _: u32, _: u8, _: bool) {}
    fn hat(&mut self, _: u32, _: u8, _: Option<u16>) {}
}

struct Bot {
    engine: Engine,
    labels: HashMap<String, (InputKind, u32)>,
    t0: Instant,
    ms: u64,
}

const N: Option<u16> = Some(0);
const E: Option<u16> = Some(9000);

impl Bot {
    fn load(name: &str) -> Option<Self> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ref/JoystickGremlin/test/action_interaction/profiles")
            .join(name);
        if !path.exists() {
            return None;
        }
        let p = gob_gremlin::load(&path).unwrap();
        let labels = p
            .logical_inputs()
            .into_iter()
            .map(|i| {
                let kind = match i.input_type {
                    InputType::Axis => InputKind::Axis,
                    InputType::Hat => InputKind::Hat,
                    _ => InputKind::Button,
                };
                (i.label, (kind, i.id))
            })
            .collect();
        let mut engine = Engine::new(&p, &HashMap::new());
        let t0 = Instant::now();
        engine.tick(t0, &mut Null);
        Some(Self {
            engine,
            labels,
            t0,
            ms: 0,
        })
    }
    fn key(&self, label: &str) -> InputKey {
        let (kind, id) = *self.labels.get(label).unwrap_or_else(|| panic!("{label}"));
        InputKey::new(LOGICAL_DEVICE_GUID, kind, id)
    }
    fn step(&mut self) {
        self.ms += 10;
        self.engine
            .tick(self.t0 + Duration::from_millis(self.ms), &mut Null);
    }
    fn send(&mut self, label: &str, v: Value) {
        self.step();
        let k = self.key(label);
        let now = self.t0 + Duration::from_millis(self.ms);
        self.engine.process(&k, v, now, &mut Null);
        self.step();
    }
    fn press(&mut self, l: &str) {
        self.send(l, Value::Button(true));
    }
    fn release(&mut self, l: &str) {
        self.send(l, Value::Button(false));
    }
    fn tap(&mut self, l: &str) {
        self.press(l);
        self.release(l);
    }
    fn hat(&mut self, l: &str, d: Option<u16>) {
        self.send(l, Value::Hat(d));
    }
    fn set_axis(&mut self, l: &str, v: f32) {
        self.send(l, Value::Axis(v));
    }
    fn wait(&mut self, s: f32) {
        for _ in 0..(s * 100.0) as u32 {
            self.step();
        }
    }
    fn button(&self, l: &str) -> bool {
        let (_, id) = self.labels[l];
        self.engine
            .logical_values()
            .buttons
            .get(&id)
            .copied()
            .unwrap_or(false)
    }
    fn axis(&self, l: &str) -> f32 {
        let (_, id) = self.labels[l];
        self.engine
            .logical_values()
            .axes
            .get(&id)
            .copied()
            .unwrap_or(0.0)
    }
    fn hat_of(&self, l: &str) -> Option<u16> {
        let (_, id) = self.labels[l];
        self.engine
            .logical_values()
            .hats
            .get(&id)
            .copied()
            .flatten()
    }
    fn mode(&self) -> &str {
        self.engine.current_mode()
    }
}

const SWITCH: &str = "A: Switch to B, B: Switch to A";

macro_rules! bot {
    ($f:expr) => {
        match Bot::load($f) {
            Some(b) => b,
            None => return,
        }
    };
}

#[test]
fn smart_toggle_remains_latched_on_press_in_other_mode() {
    let mut b = bot!("mode_change_activation.xml");
    let button = "A: Smart Toggle 0.2: out_latched_mode_a, B: Map to out_button_mode_b";
    b.tap(button);
    assert!(b.button("out_latched_mode_a"));
    b.tap(SWITCH);
    assert_eq!(b.mode(), "B");
    b.press(button);
    assert!(b.button("out_button_mode_b"));
    b.release(button);
    assert!(!b.button("out_button_mode_b"));
    assert!(b.button("out_latched_mode_a"), "latched output lost");
    b.tap(SWITCH);
    b.tap(button);
    assert!(!b.button("out_latched_mode_a"));
}

#[test]
fn inverted_mapping_remains_off_on_release_in_other_mode() {
    let mut b = bot!("mode_change_inverted.xml");
    let hb = "A: Hat Button N: Map inverted to out_shared_mode_a, B: Hat Button N: Map to out_shared_mode_b";
    b.hat(hb, N);
    assert!(!b.button("out_shared_mode_a"));
    b.tap(SWITCH);
    assert_eq!(b.mode(), "B");
    b.hat(hb, None);
    assert!(!b.button("out_shared_mode_a"));
}

#[test]
fn inverted_mapping_activates_on_press_in_other_mode() {
    let mut b = bot!("mode_change_inverted.xml");
    let hb = "A: Hat Button N: Map inverted to out_shared_mode_a, B: Hat Button N: Map to out_shared_mode_b";
    b.hat(hb, N);
    b.hat(hb, None);
    assert!(b.button("out_shared_mode_a"));
    b.tap(SWITCH);
    b.hat(hb, N);
    assert!(!b.button("out_shared_mode_a"), "inverted A output stays on");
    assert!(b.button("out_shared_mode_b"));
}

#[test]
fn hat_centers_first_on_direction_change_in_other_mode() {
    let mut b = bot!("mode_change_buttons.xml");
    let h = "A: Map to out_hat_mode_a, B: Map to out_hat_mode_b";
    b.hat(h, N);
    assert_eq!(b.hat_of("out_hat_mode_a"), N);
    b.tap(SWITCH);
    b.hat(h, E);
    assert_eq!(b.hat_of("out_hat_mode_a"), None, "A hat not centered");
    assert_eq!(b.hat_of("out_hat_mode_b"), E);
}

#[test]
fn relative_axis_stops_in_other_mode_and_restarts_on_return() {
    let mut b = bot!("mode_change_axes.xml");
    let a = "A: Map relative scale 5.0 to out_relative_mode_a";
    b.set_axis(a, 1.0);
    b.wait(0.1);
    b.tap(SWITCH);
    assert_eq!(b.mode(), "B");
    let stopped = b.axis("out_relative_mode_a");
    b.wait(0.1);
    assert!(stopped > 0.0);
    assert!(
        (b.axis("out_relative_mode_a") - stopped).abs() < 0.01,
        "kept moving in B"
    );
    b.tap(SWITCH);
    let restarted = b.axis("out_relative_mode_a");
    b.wait(0.1);
    assert!(
        b.axis("out_relative_mode_a") > restarted + 0.02,
        "did not restart"
    );
}

#[test]
fn virtual_button_activates_on_mode_entry_with_axis_in_range() {
    let mut b = bot!("mode_change_virtual_button.xml");
    let a = "B: Axis Button 0.5-1.0: Map to out_button_mode_b";
    b.set_axis(a, 0.75);
    b.tap(SWITCH);
    assert!(b.button("out_button_mode_b"), "not on at mode entry");
    b.set_axis(a, 0.0);
    b.tap(SWITCH);
    b.set_axis(a, 0.75);
    b.tap(SWITCH);
    assert!(b.button("out_button_mode_b"), "not on at second entry");
}

#[test]
fn directional_virtual_button_remains_off_on_entry_after_move_in_other_mode() {
    let mut b = bot!("mode_change_virtual_button.xml");
    let a = "B: Axis Button 0.5-1.0 from below: Map to out_directional_mode_b";
    b.set_axis(a, 0.0);
    b.set_axis(a, 0.75);
    b.tap(SWITCH);
    assert!(
        !b.button("out_directional_mode_b"),
        "directional on at entry"
    );
    b.set_axis(a, 0.8);
    assert!(b.button("out_directional_mode_b"));
}

#[test]
fn press_and_release_action_runs_on_release_in_other_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let m = "A: Macro on both [out_axis_macro_both]";
    b.press(m);
    b.wait(0.1);
    assert!((b.axis("out_axis_macro_both") - 0.1).abs() < 0.01);
    b.tap(SWITCH);
    b.release(m);
    b.wait(0.1);
    assert!(
        (b.axis("out_axis_macro_both") - 0.2).abs() < 0.01,
        "release macro not run: {}",
        b.axis("out_axis_macro_both")
    );
}

#[test]
fn held_smart_toggle_turns_off_on_release_in_other_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let t = "A: Smart Toggle 0.3: out_toggle_mode_a";
    b.press(t);
    b.wait(0.35);
    assert!(b.button("out_toggle_mode_a"));
    b.tap(SWITCH);
    b.release(t);
    assert!(!b.button("out_toggle_mode_a"), "held toggle stays on");
}

#[test]
fn tempo_runs_short_action_on_release_in_other_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let t = "A: [Tempo 0.3 on release - short: out_short, long: out_long]";
    b.press(t);
    b.tap(SWITCH);
    b.release(t);
    let mut seen = b.button("out_short");
    for _ in 0..20 {
        b.step();
        seen |= b.button("out_short");
    }
    assert!(seen, "short action not run");
    assert!(!b.button("out_long"));
}

#[test]
fn condition_action_turns_off_on_release_in_other_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let t = "A: Condition condition_button pressed - true: out_condition";
    b.press("condition_button");
    b.press(t);
    assert!(b.button("out_condition"));
    b.release("condition_button");
    b.tap(SWITCH);
    b.release(t);
    assert!(!b.button("out_condition"), "condition output stays on");
}

#[test]
fn chain_advances_on_release_in_other_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let c = "A: Chain 1: out_chain_1 2: out_chain_2";
    b.press(c);
    assert!(b.button("out_chain_1"));
    b.tap(SWITCH);
    b.release(c);
    assert!(!b.button("out_chain_1"), "chain output stays on");
    b.tap(SWITCH);
    b.press(c);
    assert!(b.button("out_chain_2"), "chain did not advance");
}

#[test]
fn macro_hold_stops_on_release_in_other_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let m = "A: Macro hold [out_axis_macro]";
    b.press(m);
    b.wait(0.2);
    b.tap(SWITCH);
    b.release(m);
    b.wait(0.1);
    let v = b.axis("out_axis_macro");
    b.wait(0.5);
    assert_eq!(b.axis("out_axis_macro"), v, "macro keeps running");
}

#[test]
fn temporary_mode_change_ignores_release_in_non_temporary_other_mode() {
    let mut b = bot!("mode_change_temporary.xml");
    b.press("A: Temporary to T1");
    b.press("T1: Temporary to T2");
    assert_eq!(b.mode(), "T2");
    b.release("A: Temporary to T1");
    assert_eq!(b.mode(), "T2");
    b.release("T1: Temporary to T2");
    assert_eq!(b.mode(), "T1");
}

#[test]
fn release_reaches_press_mode_only() {
    let mut b = bot!("mode_change_buttons.xml");
    let x = "A: Map to out_button_mode_a, B: Map to out_button_mode_b";
    b.press(x);
    assert!(b.button("out_button_mode_a"));
    b.tap(SWITCH);
    b.release(x);
    assert!(!b.button("out_button_mode_a"));
    assert!(!b.button("out_button_mode_b"));
}

#[test]
fn control_tempo_short_same_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let t = "A: [Tempo 0.3 on release - short: out_short, long: out_long]";
    b.press(t);
    b.step();
    b.release(t);
    let mut seen = b.button("out_short");
    for _ in 0..20 {
        b.step();
        seen |= b.button("out_short");
    }
    assert!(seen, "short action not seen even in the same mode");
}

#[test]
fn control_held_smart_toggle_same_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let t = "A: Smart Toggle 0.3: out_toggle_mode_a";
    b.press(t);
    b.wait(0.35);
    b.release(t);
    assert!(!b.button("out_toggle_mode_a"));
}

#[test]
fn control_chain_same_mode() {
    let mut b = bot!("mode_change_containers.xml");
    let c = "A: Chain 1: out_chain_1 2: out_chain_2";
    b.tap(c);
    b.press(c);
    assert!(b.button("out_chain_2"));
}

#[test]
fn control_virtual_button_same_mode() {
    let mut b = bot!("mode_change_virtual_button.xml");
    let a = "B: Axis Button 0.5-1.0: Map to out_button_mode_b";
    b.tap(SWITCH);
    assert_eq!(b.mode(), "B");
    b.set_axis(a, 0.75);
    let first = b.button("out_button_mode_b");
    b.set_axis(a, 0.0);
    b.set_axis(a, 0.75);
    assert!(b.button("out_button_mode_b"), "never on");
    assert!(first, "first value in range ignored");
}
