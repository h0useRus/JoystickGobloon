//! A virtual Xbox 360 controller (ViGEmBus driver), read back through `InputService`: XInput,
//! our layout, buttons, triggers and stick. Needs ViGEmBus installed, so ignored by default:
//!   cargo test -p gob-input --test vigem -- --ignored
#![cfg(windows)]

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use gob_input::{DeviceState, InputEvent, InputService};
use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};

/// Our devices that look like an Xbox controller read through XInput.
fn xinput_pads(input: &InputService) -> Vec<DeviceState> {
    input
        .devices()
        .into_iter()
        .filter(|d| d.info.axes == [1, 2, 3, 4, 5, 6] && d.info.buttons == 11 && d.info.hats == 1)
        .collect()
}

/// The device's button, hat and (rounded) axis events during the next half second.
fn events_of(events: &Receiver<InputEvent>, guid: &str) -> Vec<String> {
    let end = Instant::now() + Duration::from_millis(500);
    let mut out = Vec::new();
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        let Ok(e) = events.recv_timeout(left) else {
            continue;
        };
        match e {
            InputEvent::Button {
                device,
                id,
                pressed,
            } if device == guid => out.push(format!("button {id} {pressed}")),
            InputEvent::Hat {
                device,
                id,
                direction,
            } if device == guid => out.push(format!("hat {id} {direction:?}")),
            InputEvent::Axis { device, id, value } if device == guid => {
                out.push(format!("axis {id} {:.0}", value))
            }
            _ => {}
        }
    }
    out
}

fn has(seen: &[String], want: &[&str]) {
    for w in want {
        assert!(seen.iter().any(|s| s == w), "{w} not in {seen:?}");
    }
}

#[test]
#[ignore = "needs the ViGEmBus driver"]
fn virtual_xbox_360_controller_reads_back() {
    let input = InputService::start(|| {});
    input.set_event_mode(true);
    let before = xinput_pads(&input).len();

    let client = Client::connect().expect("ViGEmBus is not installed");
    let mut pad = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
    pad.plugin().unwrap();
    pad.wait_ready().unwrap();
    pad.update(&XGamepad::default()).unwrap();

    // Found on the next device scan (every 2 s).
    let end = Instant::now() + Duration::from_secs(6);
    let state = loop {
        let pads = xinput_pads(&input);
        if pads.len() > before {
            break pads.into_iter().last().unwrap();
        }
        assert!(
            Instant::now() < end,
            "virtual controller not found in our layout"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let info = &state.info;
    println!("{info:?}");
    assert_eq!((info.vendor_id, info.product_id), (0x045e, 0x028e));
    assert!(
        info.hid_path.to_ascii_lowercase().contains("ig_"),
        "{}",
        info.hid_path
    );
    let guid = info.guid.clone();
    let events = input.subscribe();
    std::thread::sleep(Duration::from_millis(200));

    // A, Guide and d-pad up; LT pulled; left stick pushed up (negative, like DirectInput).
    let mut report = XGamepad {
        buttons: XButtons!(A | GUIDE | UP),
        left_trigger: 255,
        thumb_ly: 32767,
        ..Default::default()
    };
    pad.update(&report).unwrap();
    let seen = events_of(&events, &guid);
    has(
        &seen,
        &[
            "button 1 true",
            "button 11 true",
            "hat 1 Some(0)",
            "axis 3 1",
            "axis 2 -1",
        ],
    );
    assert!(
        !seen.iter().any(|s| s.starts_with("axis 6")),
        "RT untouched: {seen:?}"
    );

    // Everything released, RT pulled instead.
    report.buttons = XButtons(0);
    report.left_trigger = 0;
    report.right_trigger = 255;
    report.thumb_ly = 0;
    pad.update(&report).unwrap();
    has(
        &events_of(&events, &guid),
        &[
            "button 1 false",
            "button 11 false",
            "hat 1 None",
            "axis 3 -1",
            "axis 6 1",
        ],
    );

    pad.unplug().unwrap();
    let end = Instant::now() + Duration::from_secs(6);
    while input.devices().iter().any(|d| d.info.guid == guid) {
        assert!(
            Instant::now() < end,
            "virtual controller still listed after unplugging"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
