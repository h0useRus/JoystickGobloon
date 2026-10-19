//! Lists attached controllers and prints input changes:
//! `cargo run -p gob-input --example monitor [seconds]`
use std::time::{Duration, Instant};

fn main() {
    let secs = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let input = gob_input::InputService::start(|| {});
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
