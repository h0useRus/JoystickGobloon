//! Lists attached controllers and prints input changes:
//! `cargo run -p gob-input --example monitor [seconds] [--events]`
//! (`--events`: read on DirectInput change signals instead of polling every 1 ms)
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let events = args.iter().any(|a| a == "--events");
    let secs = args.iter().find_map(|s| s.parse().ok()).unwrap_or(10);
    let input = gob_input::InputService::start(|| {});
    input.set_event_mode(events);
    println!("mode: {}", if events { "events" } else { "polling" });
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        for event in input.events() {
            if event == gob_input::InputEvent::DevicesChanged {
                for d in input.devices().iter() {
                    let i = &d.info;
                    println!(
                        "{} {:?} vid={:04x} pid={:04x} axes={:?} buttons={} hats={}
    {}",
                        i.guid,
                        i.name,
                        i.vendor_id,
                        i.product_id,
                        i.axes,
                        i.buttons,
                        i.hats,
                        i.hid_path
                    );
                }
            } else {
                println!("{event:?}");
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
