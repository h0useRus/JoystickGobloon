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

use gob_uinput::keys::scan_code;

use super::evdev::{EV_KEY, EV_REP, Node};

/// Reports (scan code, extended, pressed).
pub type KeySink = Arc<dyn Fn(u16, bool, bool) + Send + Sync>;

/// Name prefix of the virtual devices Joystick Gobloon creates (gob-virtual's joysticks and
/// gamepads, gob-engine's keyboard and mouse): their keys are not input, and they are our
/// output, not controllers (`DeviceInfo::own`).
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
