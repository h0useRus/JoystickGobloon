//! Holds a virtual joystick and a virtual gamepad in a known state, so what other programs read
//! (jstest, SDL, Wine's joy.cpl, games) can be compared control by control:
//!   joystick: axis 1..8 = -0.75, -0.5, -0.25, 0, 0.25, 0.5, 0.75, 1; buttons 1, 2, 17, 81, 95
//!             pressed, button 3 blinks every second; hat 1 east, hat 2 south
//!   gamepad:  A and Start pressed, B blinks every second; left stick x 0.5, y -0.5; RT full;
//!             d-pad right
//!   cargo run -p gob-virtual --example virtual_pattern -- [seconds] [joystick id] [gamepad id]
//! An id of 0 leaves that device out.

use std::time::{Duration, Instant};

use gob_virtual::GamepadControl;

fn main() {
    let mut args = std::env::args().skip(1).map(|a| a.parse::<u64>().ok());
    let seconds = args.next().flatten().unwrap_or(60);
    let stick_id = args.next().flatten().unwrap_or(1) as u32;
    let pad_id = args.next().flatten().unwrap_or(1) as u32;
    let backend = match gob_virtual::load() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let stick = (stick_id > 0).then(|| backend.acquire(stick_id));
    let stick = match stick.transpose() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("joystick {stick_id}: {e}");
            std::process::exit(1);
        }
    };
    let pad = (pad_id > 0).then(|| backend.acquire_gamepad(pad_id));
    let pad = match pad.transpose() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gamepad {pad_id}: {e}");
            std::process::exit(1);
        }
    };
    if let Some(s) = &stick {
        for slot in 1..=8u8 {
            s.set_axis(slot, -1.0 + f32::from(slot) * 0.25);
        }
        for n in [1, 2, 17, 81, 95] {
            s.set_button(n, true);
        }
        s.set_hat(1, Some(9000));
        s.set_hat(2, Some(18000));
    }
    if let Some(p) = &pad {
        p.set_button(GamepadControl::A, true);
        p.set_button(GamepadControl::Start, true);
        p.set_axis(GamepadControl::LeftX, 0.5);
        p.set_axis(GamepadControl::LeftY, -0.5);
        p.set_axis(GamepadControl::Rt, 1.0);
        p.set_dpad(Some(9000));
    }
    println!("joystick {stick_id}, gamepad {pad_id}: pattern for {seconds} s");
    let end = Instant::now() + Duration::from_secs(seconds);
    let mut on = false;
    while Instant::now() < end {
        on = !on;
        if let Some(s) = &stick {
            s.set_button(3, on);
        }
        if let Some(p) = &pad {
            p.set_button(GamepadControl::B, on);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
