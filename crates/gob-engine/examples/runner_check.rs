//! Starts the real Runner with a synthetic device state (axis 3 of the profile's
//! first device at 1.0) and keeps it running for a few seconds:
//! `cargo run -p gob-engine --example runner_check -- <profile.xml>`
use std::collections::HashMap;
use std::time::Duration;

use gob_input::{DeviceInfo, DeviceState};

fn main() {
    let path = std::env::args().nth(1).expect("profile path");
    let profile = gob_gremlin::load(&path).expect("profile");
    let dev = &profile.devices[0];
    let mut state = DeviceState::new(DeviceInfo {
        guid: dev.id.clone(),
        name: dev.name.trim().into(),
        vendor_id: 0,
        product_id: 0,
        axes: (1..=8).collect(),
        buttons: 0,
        hats: 0,
        ..Default::default()
    });
    state.axes[2] = 1.0;
    let (_tx, rx) = std::sync::mpsc::sync_channel(16);
    match gob_engine::Runner::start(&profile, &HashMap::new(), rx, &[state], None, || {}) {
        Ok(runner) => {
            println!("running in mode {}", runner.status().mode);
            std::thread::sleep(Duration::from_secs(4));
        }
        Err(e) => eprintln!("start failed: {e}"),
    }
}
