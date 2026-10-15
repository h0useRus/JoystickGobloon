//! Feeds one input into a profile and prints the vJoy output, without hardware:
//! `cargo run -p gob-engine --example dry_run -- <profile.xml> <device#> <axis|button|hat> <id> <value>`
//! `device#` is the 1-based index in the profile's device list.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_engine::{Engine, InputKey, InputKind, Output, Value};

struct Print;

impl Output for Print {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        println!("vJoy {device} axis {axis} = {value:.3}");
    }
    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        println!("vJoy {device} button {button} = {pressed}");
    }
    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        println!("vJoy {device} hat {hat} = {direction:?}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path, device, kind, id, value] = args.as_slice() else {
        eprintln!("usage: dry_run <profile.xml> <device#> <axis|button|hat> <id> <value>");
        std::process::exit(2);
    };
    let profile = gob_gremlin::load(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let dev = &profile.devices[device.parse::<usize>().expect("device#") - 1];
    println!(
        "{} ({}) in mode {:?}",
        dev.name.trim(),
        dev.id,
        profile.startup_mode()
    );
    let mut engine = Engine::new(&profile, &HashMap::new());
    let id: u32 = id.parse().expect("id");
    let (kind, value) = match kind.as_str() {
        "axis" => (
            InputKind::Axis,
            Value::Axis(value.parse().expect("axis value")),
        ),
        "button" => (
            InputKind::Button,
            Value::Button(value == "1" || value == "true"),
        ),
        _ => (InputKind::Hat, Value::Hat(value.parse().ok())),
    };
    let now = Instant::now();
    engine.process(&InputKey::new(&dev.id, kind, id), value, now, &mut Print);
    engine.tick(now + Duration::from_secs(2), &mut Print);
    println!("mode now: {}", engine.current_mode());
}
