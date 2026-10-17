//! Prints vJoy devices: `cargo run -p gob-vjoy --example vjoy_info [--sweep <id>]`.
//! Lists them through vJoyInterface.dll and straight from the driver (read-only) to compare.
//! With `--sweep`, moves X, blinks button 1 and spins hat 1 for 3 seconds.
use std::time::{Duration, Instant};

fn main() {
    let vjoy = match gob_vjoy::VJoy::load() {
        Ok(v) => v,
        Err(e) => return eprintln!("{e}"),
    };
    println!(
        "{} enabled={} version={}",
        vjoy.dll_path.display(),
        vjoy.enabled(),
        vjoy.version()
    );
    let dll = vjoy.devices();
    for d in &dll {
        println!("dll    {d:?}");
    }
    match gob_vjoy::direct::Direct::find() {
        None => println!("driver: not found"),
        Some(direct) => {
            println!(
                "driver version={:?} {:?}",
                direct.version(),
                direct.driver_info()
            );
            for c in direct.collections() {
                println!("driver {c:?} {:?}", direct.device_stat(c.report_id));
            }
            let devices = direct.devices();
            println!(
                "same as the DLL: {}",
                if devices == dll { "yes" } else { "NO" }
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
