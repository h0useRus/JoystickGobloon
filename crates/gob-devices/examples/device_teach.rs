//! Learns a device's numbering: asks for each control of a plugin in turn and records the input
//! the connected device sends.
//! `cargo run -p gob-devices --example device_teach <plugin dir> [--only id,id] [--write]`
//! Press the control, or Enter to skip it. Prints the result as `inputs.gob`; `--write` saves it
//! into the plugin (only the controls taught this run change).
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gob_devices::{DevicePlugin, Direction, Input, Inputs, Kind, plugin::INPUTS};
use gob_input::{InputEvent, InputService};

/// How far an axis has to move from where it rested to count.
const AXIS_TRAVEL: f32 = 0.6;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.iter().any(|a| a == "--write");
    let only: Option<Vec<String>> = args
        .iter()
        .position(|a| a == "--only")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(str::to_string).collect());
    let Some(dir) = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || args[i - 1] != "--only"))
        .map(|(_, a)| PathBuf::from(a))
    else {
        eprintln!("usage: device_teach <plugin dir> [--only id,id] [--write]");
        std::process::exit(2);
    };
    let plugin = DevicePlugin::load(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));

    let input = InputService::start(|| {});
    let started = Instant::now();
    while !input.scanned() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let Some(device) = input.devices().into_iter().find(|d| {
        let i = &d.info;
        !i.is_own() && plugin.score(i.vendor_id, i.product_id, &i.name).is_some()
    }) else {
        eprintln!(
            "no connected device fits {} ({:?})",
            plugin.id(),
            plugin.manifest.matches
        );
        std::process::exit(1);
    };
    let guid = device.info.guid.clone();
    println!(
        "{}: {} ({} buttons, {} hats, axes {:?})",
        plugin.manifest.name,
        device.info.name,
        device.info.buttons,
        device.info.hats,
        device.info.axes
    );
    println!("Press each control when asked; Enter skips it.\n");

    let (enter_tx, enter) = mpsc::channel();
    std::thread::spawn(move || {
        for _ in std::io::stdin().lock().lines() {
            if enter_tx.send(()).is_err() {
                break;
            }
        }
    });

    let mut taught = Inputs::default();
    for control in &plugin.manifest.controls {
        if only.as_ref().is_some_and(|ids| !ids.contains(&control.id)) {
            continue;
        }
        for part in control.parts() {
            let label = if part.is_empty() {
                control.name.clone()
            } else {
                format!("{} - {part}", control.name)
            };
            let axis = control.kind.is_axis_part(part);
            println!(
                "{label}: {}",
                if axis {
                    "move it all the way and back"
                } else {
                    "press it"
                }
            );
            let got = wait(&input, &guid, axis, &enter);
            match got {
                Some(i) => {
                    println!("  = {i}");
                    taught.set(&control.id, part, i);
                }
                None => println!("  skipped"),
            }
        }
        if control.kind == Kind::Hat {
            taught.compact_hat(&control.id);
        }
    }

    println!("\n# {INPUTS}\n{}", taught.to_text());
    if write {
        let path = dir.join(INPUTS);
        let mut all = plugin.defaults.clone();
        all.overlay(&taught);
        all.save(&path).expect("write inputs");
        println!("written to {}", path.display());
    }
}

/// The next input of the device: a button press, a hat direction or an axis moved far. `None`
/// when Enter was pressed. Waits for the control to be let go before returning.
fn wait(input: &InputService, guid: &str, axis: bool, enter: &mpsc::Receiver<()>) -> Option<Input> {
    while enter.try_recv().is_ok() {}
    let _ = input.events().count();
    let rest = input
        .devices()
        .into_iter()
        .find(|d| d.info.guid == guid)
        .map(|d| d.axes)
        .unwrap_or_default();
    let mut moved: Option<u8> = None;
    loop {
        if enter.try_recv().is_ok() {
            return None;
        }
        for event in input.events() {
            match event {
                InputEvent::Button {
                    device,
                    id,
                    pressed: true,
                } if !axis && &*device == guid => {
                    settle(input, guid, Input::Button(id));
                    return Some(Input::Button(id));
                }
                InputEvent::Hat {
                    device,
                    id,
                    direction: Some(angle),
                } if !axis && &*device == guid => {
                    settle(input, guid, Input::Hat(id));
                    return Some(Input::HatDirection(id, Direction::of_angle(angle)));
                }
                InputEvent::Axis { device, id, value } if axis && &*device == guid => {
                    let travel = (value - rest[usize::from(id - 1)]).abs();
                    if travel > AXIS_TRAVEL {
                        moved = Some(id);
                    }
                    if travel < 0.2 && moved == Some(id) {
                        return Some(Input::Axis(id));
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Waits until the pressed button or hat is let go (others may stay held, e.g. a switch).
fn settle(input: &InputService, guid: &str, pressed: Input) {
    loop {
        let held = input
            .devices()
            .into_iter()
            .find(|d| d.info.guid == guid)
            .is_some_and(|d| match pressed {
                Input::Button(n) => d.buttons.get(usize::from(n - 1)).copied().unwrap_or(false),
                Input::Hat(n) => d.hats.get(usize::from(n - 1)).copied().flatten().is_some(),
                _ => false,
            });
        if !held {
            let _ = input.events().count();
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
