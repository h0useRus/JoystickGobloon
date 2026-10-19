//! Changes the vJoy configuration without vJoyConfig (UAC prompt), e.g.
//! `cargo run -p gob-vjoy --example vjoy_config -- 2 -f -a X Y Z Rx Ry Rz Sl0 Sl1 -b 127 -p 4`,
//! `... -- -d 3`, `... -- enable on`. Runs itself elevated with `--vjoy-config` like Joystick Gobloon's exe.
use gob_vjoy::configure::{self, Change};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some((first, rest)) = args.split_first()
        && first == configure::ARG
    {
        std::process::exit(configure::run_admin(rest));
    }
    let Some(change) = Change::from_args(&args) else {
        eprintln!(
            "usage: vjoy_config <id> -f [-a X Y ...] -b N (-p|-s) N [-e ...] | -d <id> | enable on|off"
        );
        std::process::exit(2);
    };
    println!("{}", change.args().join(" "));
    let exe = std::env::current_exe().expect("exe path");
    match configure::run_elevated(&exe, &change) {
        Ok(outcome) => println!("{outcome:?}"),
        Err(e) => println!("failed: {e}"),
    }
}
