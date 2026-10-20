//! Creates a virtual joystick for a few seconds, moves its X axis and presses button 1, so
//! `evtest` or `jstest` can show it. Shows whether uinput is usable for this user.
//!   cargo run -p gob-uinput --example uinput_check

#[cfg(target_os = "linux")]
fn main() {
    use gob_uinput::{Axis, Spec, VirtualDevice, codes};

    let axes = [Axis {
        code: codes::ABS_X,
        min: -32768,
        max: 32767,
    }];
    let spec = Spec {
        name: "Joystick Gobloon uinput check",
        bus: codes::BUS_VIRTUAL,
        keys: &[codes::BTN_JOYSTICK],
        axes: &axes,
        ..Spec::default()
    };
    let dev = match VirtualDevice::create(&spec) {
        Ok(dev) => dev,
        Err(e) => {
            eprintln!("cannot create a uinput device: {e}");
            std::process::exit(1);
        }
    };
    println!("created {:?}", dev.event_node());
    for step in 0..50 {
        let x = ((f64::from(step) / 5.0).sin() * 32767.0) as i32;
        let pressed = i32::from(step % 10 < 5);
        if let Err(e) = dev.emit(&[
            (codes::EV_ABS, codes::ABS_X, x),
            (codes::EV_KEY, codes::BTN_JOYSTICK, pressed),
        ]) {
            eprintln!("write: {e}");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("uinput exists only on Linux");
}
