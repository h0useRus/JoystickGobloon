//! WRITES to the device (temporary, until it is reconnected) - run only on purpose.
//! `led_set <device index> <led> <#RRGGBB> [off|on|slow|fast|ultra]` sets one LED,
//! `led_set <device index> clear [led]` returns one or every LED to the device profile.
//! Device indexes as listed when run without arguments.
use gob_lighting::{Color, LedConfig, LedMode};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let devices = gob_lighting::devices();
    if args.is_empty() {
        for (i, d) in devices.iter().enumerate() {
            println!("{i}: {} {}", d.name(), d.path());
        }
        return;
    }
    let Some(device) = args[0].parse::<usize>().ok().and_then(|i| devices.get(i)) else {
        return eprintln!("no device {}", args[0]);
    };
    let result = if args.get(1).map(String::as_str) == Some("clear") {
        match args.get(2).and_then(|l| l.parse().ok()) {
            Some(led) => device.release(led),
            None => device.release_all(),
        }
    } else {
        let (Some(led), Some(color)) = (
            args.get(1).and_then(|l| l.parse::<u8>().ok()),
            args.get(2).and_then(|c| parse_color(c)),
        ) else {
            return eprintln!(
                "usage: led_set <device> <led> <#RRGGBB> [mode] | <device> clear [led]"
            );
        };
        let led_mode = match args.get(3).map(String::as_str) {
            Some("off") => LedMode::Off,
            Some("slow") => LedMode::SlowBlink,
            Some("fast") => LedMode::FastBlink,
            Some("ultra") => LedMode::UltraBlink,
            _ => LedMode::Constant,
        };
        device.set(LedConfig {
            led,
            led_mode,
            color1: color,
            ..Default::default()
        })
    };
    match result.and_then(|_| device.read()) {
        Ok(list) => {
            println!("set LEDs now:");
            for c in list {
                println!("    {c:?}");
            }
        }
        Err(e) => eprintln!("{e}"),
    }
}

fn parse_color(s: &str) -> Option<Color> {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).ok()?;
    Some(Color::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}
