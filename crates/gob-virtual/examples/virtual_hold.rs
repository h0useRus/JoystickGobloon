//! Holds virtual joystick 1 for a while, so other programs (jstest, sdl2-jstest, games) can be
//! checked against it. The layout is the backend's default unless `<data dir>/virtual.gob`
//! says otherwise.
//!   cargo run -p gob-virtual --example virtual_hold -- [seconds] [data dir]

fn main() {
    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(30);
    if let Some(dir) = args.next() {
        gob_virtual::set_data_dir(std::path::Path::new(&dir));
    }
    let backend = match gob_virtual::load() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let info = backend.devices().into_iter().find(|d| d.id == 1);
    println!("{info:?}");
    let device = match backend.acquire(1) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    println!("holding device 1 for {seconds} s");
    std::thread::sleep(std::time::Duration::from_secs(seconds));
    drop(device);
}
