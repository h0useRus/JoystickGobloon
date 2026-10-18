//! Prints vJoy devices: `cargo run -p gob-vjoy --example vjoy_info [--sweep <id>]`.
//! Lists the driver's devices (read-only) and the configuration in the registry; the devices
//! the driver reports must match the registry.
//! With `--sweep`, moves X, blinks button 1 and spins hat 1 for 3 seconds.
use std::time::{Duration, Instant};

fn main() {
    let vjoy = match gob_vjoy::VJoy::load() {
        Ok(v) => v,
        Err(e) => return eprintln!("{e}"),
    };
    println!("vJoy enabled={} version={}", vjoy.enabled(), vjoy.version());
    let devices = vjoy.devices();
    if let Some(direct) = gob_vjoy::direct::Direct::find() {
        println!("driver {:?}", direct.driver_info());
        for c in direct.collections() {
            println!("driver {c:?} {:?}", direct.device_stat(c.report_id));
        }
    }
    match gob_vjoy::descriptor::read_registry() {
        Err(e) => println!("registry: {e}"),
        Ok(reg) => {
            for c in &reg {
                println!("reg    {}", c.create_args().join(" "));
            }
            let same = reg.len() == devices.len()
                && reg.iter().zip(&devices).all(|(c, d)| {
                    let axes: Vec<u8> = (1..=8u8).filter(|s| c.axes[usize::from(*s) - 1]).collect();
                    let hats = if c.discrete_hats {
                        d.disc_hats
                    } else {
                        d.cont_hats
                    };
                    c.id == d.id && axes == d.axes && c.buttons == d.buttons && c.hats == hats
                });
            println!(
                "driver matches the registry: {}",
                if same { "yes" } else { "NO" }
            );
        }
    }
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, id] = args.as_slice() {
        if flag == "--sweep" {
            let dev = vjoy
                .acquire(id.parse().expect("device id"))
                .unwrap_or_else(|e| panic!("{e}"));
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(3) {
                let t = start.elapsed().as_secs_f32();
                dev.set_axis(1, (t * 3.0).sin());
                dev.set_button(1, ((t * 4.0) as u32).is_multiple_of(2));
                dev.set_hat(1, Some(((t * 9000.0) as u32 % 36000) as u16));
                std::thread::sleep(Duration::from_millis(10));
            }
            println!("sweep done");
        }
    }
}
