//! WRITES to the device (temporary, until it is reconnected) - run only on purpose.
//! `led_set <device index> <led> <#RRGGBB> [off|on|slow|fast|fastest]` sets one LED,
//! `led_set <device index> clear [led]` drops one or every LED from the list.
//! Device indexes as listed when run without arguments.
use gob_lighting::{Led, Mode, Vendor};

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
        let mode = match args.get(3).map(String::as_str) {
            Some("off") => Mode::Off,
            Some("slow") => Mode::SlowBlink,
            Some("fast") => Mode::FastBlink,
            Some("fastest") => Mode::FastestBlink,
            _ => Mode::On,
        };
        device.set(&[Led {
            id: led,
            color,
            mode,
        }])
    };
    if let Err(e) = result {
        return eprintln!("{e}");
    }
    match device.vendor {
        Vendor::Vkb => match gob_lighting::vkb::read(&device.hid) {
            Ok(list) => {
                println!("set LEDs now:");
                for c in list {
                    println!("    {c:?}");
                }
            }
            Err(e) => eprintln!("{e}"),
        },
    }
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}
