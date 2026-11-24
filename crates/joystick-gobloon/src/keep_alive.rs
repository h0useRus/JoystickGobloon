//! Keep alive (Settings > General): a key combination (Win+Alt+Z unless changed) switches a
//! mouse nudge (a short move and back) every few seconds on and off, so a program that signs out an
//! idle user keeps seeing activity. The window in front when it was switched on (the game) is
//! brought forward for each nudge when another one has the focus by then, and the other one
//! gets it back afterwards (option `front`).
//!
//! The shortcut is watched on a thread of its own, like the overlay's: the game has the focus,
//! so the window gets no key events. While the option is off the thread only sleeps.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use gob_core::Key;

/// How often the shortcut is looked at while the option is on.
const POLL: Duration = Duration::from_millis(50);
/// How often the thread looks whether the option was switched on.
const IDLE: Duration = Duration::from_millis(250);
/// Win+Alt+Z as settings store keys (left Win with its E0 prefix, left Alt, Z).
pub const DEFAULT_KEYS: [u16; 3] = [0x15b, 0x38, 0x2c];
/// Time for the game to take the focus before the nudge, and to hand it back after.
const FRONT_WAIT: Duration = Duration::from_millis(500);
const BACK_WAIT: Duration = Duration::from_millis(150);
/// The range of the interval in the settings, in seconds.
pub const MIN_SECS: u32 = 10;
pub const MAX_SECS: u32 = 900;

#[derive(Default)]
struct Shared {
    /// The option is on: the shortcut works and the nudge may run.
    enabled: AtomicBool,
    /// No shortcut is being recorded: pressing the shortcut switches the nudge.
    listening: AtomicBool,
    /// The shortcut.
    keys: Mutex<Vec<Key>>,
    /// Seconds between nudges.
    every: AtomicU32,
    /// The nudge runs now.
    on: AtomicBool,
    /// Bring the window in front at switch-on forward for each nudge.
    front: AtomicBool,
}

pub struct KeepAlive {
    shared: Arc<Shared>,
    /// The shortcut as the settings last gave it.
    codes: Vec<u16>,
    /// What the status bar last said.
    shown: bool,
}

fn run(shared: &Shared, ctx: &egui::Context) {
    let (mut was_down, mut last, mut prepared) = (false, None::<Instant>, false);
    // The window in front when the nudge was switched on, unless it was one of ours.
    let mut target = None;
    // The nudge running now, if any.
    let mut nudging: Option<std::thread::JoinHandle<()>> = None;
    loop {
        if !shared.enabled.load(Ordering::Relaxed) {
            if shared.on.swap(false, Ordering::Relaxed) {
                log::info!("keep alive off (option switched off)");
                ctx.request_repaint();
            }
            // Switched on again: made ready again (Linux retries a virtual mouse that failed).
            (was_down, prepared) = (false, false);
            std::thread::sleep(IDLE);
            continue;
        }
        if !prepared {
            // Before the shortcut is first pressed, so the first nudge is not lost.
            gob_engine::prepare_mouse();
            prepared = true;
        }
        std::thread::sleep(POLL);
        let down = shared.listening.load(Ordering::Relaxed) && {
            let keys = shared
                .keys
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            crate::overlay::combo_down(&keys)
        };
        if down && !was_down {
            let on = !shared.on.fetch_xor(true, Ordering::Relaxed);
            log::info!("keep alive {}", if on { "on" } else { "off" });
            last = None;
            target = crate::platform::front_window().filter(|&w| !crate::platform::own_window(w));
            ctx.request_repaint();
        }
        was_down = down;
        // Taken every pass, so a changed interval counts from the last nudge at once.
        let every = Duration::from_secs(u64::from(
            shared
                .every
                .load(Ordering::Relaxed)
                .clamp(MIN_SECS, MAX_SECS),
        ));
        let idle = nudging
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished);
        if idle && shared.on.load(Ordering::Relaxed) && last.is_none_or(|t| t.elapsed() >= every) {
            let target = target.filter(|_| shared.front.load(Ordering::Relaxed));
            // On a thread of its own: a nudge takes up to a second, and the shortcut is
            // watched meanwhile (a quick press to stop it is not missed).
            nudging = std::thread::Builder::new()
                .name("keep-alive-nudge".into())
                .spawn(move || nudge(target))
                .inspect_err(|e| log::error!("keep alive nudge thread: {e}"))
                .ok();
            last = Some(Instant::now());
        }
    }
}

/// One nudge; with `target`, that window is brought forward first when another one is in front,
/// and the other one gets the focus back after.
fn nudge(target: Option<u64>) {
    let before = crate::platform::front_window();
    let switched = match target {
        Some(t) if before != Some(t) => {
            let done = crate::platform::bring_to_front(t);
            if done {
                std::thread::sleep(FRONT_WAIT);
            } else {
                log::debug!("keep alive: the window could not be brought forward");
            }
            done
        }
        _ => false,
    };
    gob_engine::nudge_mouse();
    if switched && let Some(before) = before {
        std::thread::sleep(BACK_WAIT);
        if !crate::platform::bring_to_front(before) {
            log::debug!("keep alive: the focus could not be given back");
        }
    }
}

impl KeepAlive {
    /// Starts the thread; `None` when it did not start (logged).
    pub fn start(ctx: egui::Context) -> Option<Self> {
        let shared = Arc::new(Shared::default());
        let thread_shared = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("keep-alive".into())
            .spawn(move || run(&thread_shared, &ctx));
        if let Err(e) = spawned {
            log::error!("keep alive thread: {e}");
            return None;
        }
        Some(Self {
            shared,
            codes: Vec::new(),
            shown: false,
        })
    }

    /// Takes the option from the settings; cheap, called every frame. `listening` is off while a
    /// shortcut or input is being recorded, so recording does not switch it (a running nudge
    /// keeps running).
    pub fn configure(
        &mut self,
        enabled: bool,
        listening: bool,
        every_secs: u32,
        front: bool,
        keys: &[u16],
    ) {
        self.shared.enabled.store(enabled, Ordering::Relaxed);
        self.shared.front.store(front, Ordering::Relaxed);
        self.shared.listening.store(listening, Ordering::Relaxed);
        self.shared.every.store(every_secs, Ordering::Relaxed);
        if self.codes != keys {
            self.codes = keys.to_vec();
            *self.shared.keys.lock().unwrap_or_else(|e| e.into_inner()) = keys
                .iter()
                .map(|&c| crate::overlay::key_from_code(c))
                .collect();
        }
    }

    /// The nudge runs now.
    pub fn is_on(&self) -> bool {
        self.shared.on.load(Ordering::Relaxed)
    }

    /// Switched on or off since the last call: the new state, for the status bar.
    pub fn changed(&mut self) -> Option<bool> {
        let on = self.is_on();
        (on != self.shown).then(|| {
            self.shown = on;
            on
        })
    }
}
