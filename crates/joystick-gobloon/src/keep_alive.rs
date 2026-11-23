//! Keep alive: Win+Alt+Z switches a mouse nudge (one pixel and back) every few seconds on and
//! off, so a program that signs out an idle user keeps seeing activity. Hidden: only with
//! `keep_alive_secs` in settings.gob, no switch in the UI and nothing on the site.
//!
//! The shortcut is watched on a thread of its own, like the overlay's: the game has the focus,
//! so the window gets no key events.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eframe::egui;
use gob_core::Key;

/// How often the shortcut is looked at.
const POLL: Duration = Duration::from_millis(50);

pub struct KeepAlive {
    on: Arc<AtomicBool>,
    /// What the status bar last said.
    shown: bool,
}

/// Win (either), Alt (either) and Z held together.
fn shortcut_down() -> bool {
    let held = |scan_code, extended| {
        crate::platform::key_held(Key {
            scan_code,
            extended,
        }) == Some(true)
    };
    (held(0x5b, true) || held(0x5c, true))
        && (held(0x38, false) || held(0x38, true))
        && held(0x2c, false)
}

impl KeepAlive {
    /// `None` when the hidden setting is off (0) or the thread did not start.
    pub fn start(every_secs: u32, ctx: egui::Context) -> Option<Self> {
        if every_secs == 0 {
            return None;
        }
        let every = Duration::from_secs(u64::from(every_secs));
        let on = Arc::new(AtomicBool::new(false));
        let shared = on.clone();
        let spawned = std::thread::Builder::new()
            .name("keep-alive".into())
            .spawn(move || {
                let (mut was_down, mut next, mut prepared) = (false, Instant::now(), false);
                loop {
                    std::thread::sleep(POLL);
                    let down = shortcut_down();
                    if down && !was_down {
                        let now_on = !shared.fetch_xor(true, Ordering::Relaxed);
                        log::info!("keep alive {}", if now_on { "on" } else { "off" });
                        next = Instant::now();
                        ctx.request_repaint();
                    }
                    was_down = down;
                    if shared.load(Ordering::Relaxed) && Instant::now() >= next {
                        gob_engine::nudge_mouse(!prepared);
                        prepared = true;
                        next = Instant::now() + every;
                    }
                }
            });
        if let Err(e) = spawned {
            log::error!("keep alive thread: {e}");
            return None;
        }
        Some(Self { on, shown: false })
    }

    /// The nudge runs now.
    pub fn is_on(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }

    /// Switched on or off since the last call: the new state, for the status bar.
    pub fn changed(&mut self) -> Option<bool> {
        let on = self.on.load(Ordering::Relaxed);
        (on != self.shown).then(|| {
            self.shown = on;
            on
        })
    }
}
