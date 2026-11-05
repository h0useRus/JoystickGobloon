//! Speed and allocations of the engine's hot path, without hardware:
//! `cargo run --release -p gob-engine --example engine_bench [-- <profile.xml>]`
//! Without a profile it builds one: 2 sticks, each with 8 axes (response curve + vJoy axis),
//! 92 buttons and 4 hats mapped to vJoy, in a mode and its child mode. Events go through
//! `Engine::process` + `Engine::tick` like the runner does; the output only counts calls.
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gob_engine::{Engine, InputKey, InputKind, Output, Value};

/// Counts allocations (and reallocations) of the whole process.
struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Output that only counts writes (each one is a driver call today).
#[derive(Default)]
struct Count(u64);

impl Output for Count {
    fn axis(&mut self, _device: u32, _axis: u8, _value: f32) {
        self.0 += 1;
    }
    fn button(&mut self, _device: u32, _button: u8, _pressed: bool) {
        self.0 += 1;
    }
    fn hat(&mut self, _device: u32, _hat: u8, _direction: Option<u16>) {
        self.0 += 1;
    }
}

const DEVICES: [&str; 2] = [
    "aaaaaaaa-0000-0000-0000-000000000001",
    "aaaaaaaa-0000-0000-0000-000000000002",
];
const AXES: u32 = 8;
const BUTTONS: u32 = 92;
const HATS: u32 = 4;
const EVENTS: u32 = 100_000;

fn prop(ty: &str, name: &str, value: &str) -> String {
    format!(r#"<property type="{ty}"><name>{name}</name><value>{value}</value></property>"#)
}

fn action(id: &str, kind: &str, body: &str) -> String {
    format!(
        r#"<action id="{id}" type="{kind}">{body}{}</action>"#,
        prop("activation-mode", "activation-mode", "both")
    )
}

fn vjoy(id: &str, device: usize, kind: &str, n: u32) -> String {
    action(
        id,
        "map-to-vjoy",
        &format!(
            "{}{}{}{}",
            prop("int", "vjoy-device-id", &(device + 1).to_string()),
            prop("int", "vjoy-input-id", &n.to_string()),
            prop("input_type", "vjoy-input-type", kind),
            prop("bool", "button-inverted", "False")
        ),
    )
}

fn curve(id: &str) -> String {
    action(
        id,
        "response-curve",
        &format!(
            "<deadzone>{}{}{}{}</deadzone><control-points>{}{}{}</control-points>{}",
            prop("float", "low", "-1.0"),
            prop("float", "center-low", "-0.05"),
            prop("float", "center-high", "0.05"),
            prop("float", "high", "1.0"),
            prop("point2d", "point", "-1.0,-1.0"),
            prop("point2d", "point", "0.0,0.0"),
            prop("point2d", "point", "1.0,1.0"),
            prop("string", "curve-type", "Cubic Spline")
        ),
    )
}

/// The built-in profile (see the module comment).
fn built_profile() -> String {
    let (mut inputs, mut library) = (String::new(), String::new());
    let mut bind = |device: usize, kind: &str, id: u32, children: &[String]| {
        let root = format!("r-{device}-{kind}-{id}");
        let refs: String = children
            .iter()
            .map(|c| format!("<action-id>{c}</action-id>"))
            .collect();
        library += &action(&root, "root", &format!("<actions>{refs}</actions>"));
        inputs += &format!(
            "<input><device-id>{}</device-id><input-type>{kind}</input-type><mode>Default</mode>\
             <input-id>{id}</input-id><action-configuration><root-action>{root}</root-action>\
             <behavior>{kind}</behavior></action-configuration></input>",
            DEVICES[device]
        );
    };
    let mut actions = String::new();
    for device in 0..DEVICES.len() {
        for id in 1..=AXES {
            let (c, v) = (format!("c-{device}-{id}"), format!("a-{device}-{id}"));
            actions += &curve(&c);
            actions += &vjoy(&v, device, "axis", id);
            bind(device, "axis", id, &[c, v]);
        }
        for id in 1..=BUTTONS {
            let v = format!("b-{device}-{id}");
            actions += &vjoy(&v, device, "button", id);
            bind(device, "button", id, &[v]);
        }
        for id in 1..=HATS {
            let v = format!("h-{device}-{id}");
            actions += &vjoy(&v, device, "hat", id);
            bind(device, "hat", id, &[v]);
        }
    }
    format!(
        r#"<profile version="14"><inputs>{inputs}</inputs>
        <settings><startup-mode>Combat</startup-mode></settings><library>{library}{actions}</library>
        <modes><mode>Default</mode><mode parent="Default">Combat</mode></modes></profile>"#
    )
}

/// Runs `events` like the runner: each one through `process`, then `tick`.
fn run(name: &str, engine: &mut Engine, events: &[(InputKey, Value)], start: Instant) -> Instant {
    let mut out = Count::default();
    let allocs = ALLOCS.load(Ordering::Relaxed);
    let began = Instant::now();
    let mut now = start;
    for (key, value) in events {
        now += Duration::from_micros(500);
        engine.process(key, *value, now, &mut out);
        engine.tick(now, &mut out);
    }
    let took = began.elapsed();
    let allocs = ALLOCS.load(Ordering::Relaxed) - allocs;
    let n = events.len() as f64;
    println!(
        "{name:<8} {:>8.0} ns/event  {:>6.2} allocations/event  {:>5.2} writes/event",
        took.as_nanos() as f64 / n,
        allocs as f64 / n,
        out.0 as f64 / n,
    );
    now
}

fn main() {
    let profile = match std::env::args().nth(1) {
        Some(path) => gob_gremlin::load(&path).unwrap_or_else(|e| panic!("{path}: {e}")),
        None => gob_gremlin::from_str(&built_profile()).expect("built-in profile"),
    };
    let devices: Vec<&str> = profile.devices.iter().map(|d| d.id.as_str()).collect();
    let devices = if devices.is_empty() {
        DEVICES.to_vec()
    } else {
        devices
    };
    let mut engine = Engine::new(&profile, &HashMap::new());
    println!(
        "{} devices, {} events per run, mode {}",
        devices.len(),
        EVENTS,
        engine.current_mode()
    );

    let key = |i: u32, kind: InputKind, id: u32| {
        InputKey::new(devices[i as usize % devices.len()], kind, id)
    };
    let axes: Vec<(InputKey, Value)> = (0..EVENTS)
        .map(|i| {
            let v = ((i as f32) * 0.013).sin();
            (key(i / AXES, InputKind::Axis, i % AXES + 1), Value::Axis(v))
        })
        .collect();
    let buttons: Vec<(InputKey, Value)> = (0..EVENTS)
        .map(|i| {
            let id = (i / 2) % BUTTONS + 1;
            (
                key(i / (2 * BUTTONS), InputKind::Button, id),
                Value::Button(i % 2 == 0),
            )
        })
        .collect();
    let hats: Vec<(InputKey, Value)> = (0..EVENTS)
        .map(|i| {
            let direction = (i % 9 != 8).then_some(((i % 9) * 4500) as u16);
            (
                key(i / 9, InputKind::Hat, i / 9 % HATS + 1),
                Value::Hat(direction),
            )
        })
        .collect();

    // A warm-up round fills the engine's maps, then each kind is measured.
    let mut now = run("warm-up", &mut engine, &axes[..1000], Instant::now());
    now = run("axes", &mut engine, &axes, now);
    now = run("buttons", &mut engine, &buttons, now);
    run("hats", &mut engine, &hats, now);
    println!(
        "idle: next deadline {:?}",
        engine
            .next_deadline()
            .map(|d| d.saturating_duration_since(now))
    );
}
