//! Changes the vJoy configuration without vJoyConfig (UAC prompt), e.g.
//! `cargo run -p gob-vjoy --example vjoy_config -- 2 -f -a X Y Z Rx Ry Rz Sl0 Sl1 -b 127 -p 4`,
//! `... -- -d 3`, `... -- enable on`. Runs itself elevated with `--admin` first.
use gob_vjoy::configure::{self, Change, Outcome};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (admin, rest) = match args.split_first() {
        Some((a, rest)) if a == "--admin" => (true, rest),
        _ => (false, args.as_slice()),
    };
    let Some(change) = Change::from_args(rest) else {
        eprintln!("usage: vjoy_config <id> -f [-a X Y ...] -b N (-p|-s) N [-e ...] | -d <id> | enable on|off");
        std::process::exit(2);
    };
    if admin {
        let code = match configure::apply(&change) {
            Ok(Outcome::Done) => 0,
            Ok(Outcome::RebootNeeded) => configure::EXIT_REBOOT,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
        std::process::exit(code as i32);
    }
    let exe = std::env::current_exe().expect("exe path");
    let mut elevated = vec!["--admin".to_string()];
    elevated.extend(change.args());
    println!("{}", change.args().join(" "));
    match gob_vjoy::config::run_elevated(&exe, &elevated) {
        Ok(()) => println!("done"),
        Err(e) => println!("failed: {e}"),
    }
}
