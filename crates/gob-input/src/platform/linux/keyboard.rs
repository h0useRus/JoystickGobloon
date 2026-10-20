//! Keyboard input from every evdev keyboard (works under X11 and Wayland alike).
//!
//! Keys still reach other programs (the devices are not grabbed). Auto-repeat is filtered, and
//! our own virtual devices are skipped, so keys sent by map-to-keyboard or macro actions never
//! come back as input. Key codes become set 1 scan codes, as the Windows hook reports them.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::evdev::{EV_KEY, EV_REP, Node};

/// Reports (scan code, extended, pressed).
pub type KeySink = Arc<dyn Fn(u16, bool, bool) + Send + Sync>;

/// Name prefix of the virtual devices Joystick Gobloon creates; their keys are not input.
pub(crate) const OWN_DEVICE_PREFIX: &str = "Joystick Gobloon";
/// Look for new keyboards this often.
const RESCAN: Duration = Duration::from_secs(2);
/// Longest wait for keys, so stopping is noticed.
const WAIT_MS: i32 = 200;
const KEY_ESC: usize = 1;
const KEY_A: usize = 30;
const KEY_Z: usize = 44;
const KEY_SPACE: usize = 57;

pub struct Hook {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

pub fn start(sink: KeySink) -> Option<Hook> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let stop = stop.clone();
        std::thread::Builder::new()
            .name("gob-keyboard".into())
            .spawn(move || run(&sink, &stop))
            .ok()?
    };
    Some(Hook {
        stop,
        thread: Some(thread),
    })
}

impl Drop for Hook {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(sink: &KeySink, stop: &AtomicBool) {
    let mut keyboards: Vec<Node> = Vec::new();
    let mut down: HashSet<(u16, bool)> = HashSet::new();
    let mut next_scan = Instant::now();
    // SAFETY: `input_event` is plain data; all zeroes is a valid value.
    let mut buf: [super::evdev::Event; 64] = unsafe { std::mem::zeroed() };
    let mut fds = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        if Instant::now() >= next_scan {
            next_scan = Instant::now() + RESCAN;
            let open: Vec<PathBuf> = keyboards.iter().map(|k| k.path.clone()).collect();
            keyboards.extend(
                super::evdev::event_nodes()
                    .into_iter()
                    .filter(|p| !open.contains(p))
                    .filter_map(|p| Node::open(&p).ok())
                    .filter(is_keyboard),
            );
        }
        fds.clear();
        fds.extend(keyboards.iter().map(|k| libc::pollfd {
            fd: k.fd(),
            events: libc::POLLIN,
            revents: 0,
        }));
        if fds.is_empty() {
            std::thread::sleep(Duration::from_millis(WAIT_MS as u64));
            continue;
        }
        super::evdev::wait_readable(&mut fds, WAIT_MS);
        keyboards.retain(|k| {
            loop {
                let Ok(count) = k.read(&mut buf) else {
                    return false; // unplugged
                };
                for e in buf[..count].iter().filter(|e| e.type_ == EV_KEY) {
                    // 2 = auto-repeat.
                    if e.value == 2 {
                        continue;
                    }
                    let Some(key) = scan_code(e.code) else {
                        continue;
                    };
                    let pressed = e.value == 1;
                    let changed = if pressed {
                        down.insert(key)
                    } else {
                        down.remove(&key)
                    };
                    if changed {
                        sink(key.0, key.1, pressed);
                    }
                }
                if count < buf.len() {
                    return true;
                }
            }
        });
    }
}

/// A keyboard: letter keys, space and Esc, and key repeat; not one of our virtual devices.
fn is_keyboard(node: &Node) -> bool {
    let caps = node.caps();
    caps.events.has(EV_REP.into())
        && [KEY_ESC, KEY_A, KEY_Z, KEY_SPACE]
            .iter()
            .all(|&k| caps.keys.has(k))
        && !node.name().starts_with(OWN_DEVICE_PREFIX)
}

/// evdev key code -> (set 1 scan code, E0 prefix). Codes 1..=88 are the scan codes themselves
/// (the kernel's table started as set 1), except NumLock, which Windows (and Gremlin) report
/// with E0 to tell it from Pause.
pub(crate) fn scan_code(code: u16) -> Option<(u16, bool)> {
    Some(match code {
        69 => (0x45, true), // KEY_NUMLOCK
        1..=88 => (code, false),
        89 => (0x73, false),                       // KEY_RO
        92 => (0x79, false),                       // KEY_HENKAN
        93 => (0x70, false),                       // KEY_KATAKANAHIRAGANA
        94 => (0x7b, false),                       // KEY_MUHENKAN
        96 => (0x1c, true),                        // KEY_KPENTER
        97 => (0x1d, true),                        // KEY_RIGHTCTRL
        98 => (0x35, true),                        // KEY_KPSLASH
        99 => (0x37, true),                        // KEY_SYSRQ (Print Screen)
        100 => (0x38, true),                       // KEY_RIGHTALT
        102 => (0x47, true),                       // KEY_HOME
        103 => (0x48, true),                       // KEY_UP
        104 => (0x49, true),                       // KEY_PAGEUP
        105 => (0x4b, true),                       // KEY_LEFT
        106 => (0x4d, true),                       // KEY_RIGHT
        107 => (0x4f, true),                       // KEY_END
        108 => (0x50, true),                       // KEY_DOWN
        109 => (0x51, true),                       // KEY_PAGEDOWN
        110 => (0x52, true),                       // KEY_INSERT
        111 => (0x53, true),                       // KEY_DELETE
        113 => (0x20, true),                       // KEY_MUTE
        114 => (0x2e, true),                       // KEY_VOLUMEDOWN
        115 => (0x30, true),                       // KEY_VOLUMEUP
        117 => (0x59, false),                      // KEY_KPEQUAL
        119 => (0x45, false),                      // KEY_PAUSE
        121 => (0x7e, false),                      // KEY_KPCOMMA
        122 => (0x72, false),                      // KEY_HANGEUL
        123 => (0x71, false),                      // KEY_HANJA
        124 => (0x7d, false),                      // KEY_YEN
        125 => (0x5b, true),                       // KEY_LEFTMETA
        126 => (0x5c, true),                       // KEY_RIGHTMETA
        127 => (0x5d, true),                       // KEY_COMPOSE (Menu)
        163 => (0x19, true),                       // KEY_NEXTSONG
        164 => (0x22, true),                       // KEY_PLAYPAUSE
        165 => (0x10, true),                       // KEY_PREVIOUSSONG
        166 => (0x24, true),                       // KEY_STOPCD
        183..=193 => (0x64 + (code - 183), false), // KEY_F13..KEY_F23
        194 => (0x76, false),                      // KEY_F24
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::scan_code;

    #[test]
    fn scan_codes_like_windows() {
        assert_eq!(scan_code(1), Some((0x01, false)), "Esc");
        assert_eq!(scan_code(30), Some((0x1e, false)), "A");
        assert_eq!(scan_code(88), Some((0x58, false)), "F12");
        assert_eq!(scan_code(97), Some((0x1d, true)), "right Ctrl");
        assert_eq!(scan_code(103), Some((0x48, true)), "up");
        assert_eq!(scan_code(119), Some((0x45, false)), "Pause");
        assert_eq!(scan_code(69), Some((0x45, true)), "NumLock");
        assert_eq!(scan_code(183), Some((0x64, false)), "F13");
        assert_eq!(scan_code(194), Some((0x76, false)), "F24");
        assert_eq!(scan_code(0x120), None, "joystick button");
    }
}
