//! Switching profiles by program: which program's profile should run, from the program in
//! front. Only a program with a profile in front switches; anything else in front (another
//! program, the desktop, Joystick Gobloon itself) keeps the running profile.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use eframe::egui;

use crate::platform::Program;
use crate::settings::{AppEntry, program_key};

/// How often the program in front is looked up.
const INTERVAL: Duration = Duration::from_secs(1);

/// Looks up the program in front on its own thread while switching is on and wakes the app
/// when it changes (also with the window in the tray).
pub struct Watcher {
    on: Arc<AtomicBool>,
    changed: Receiver<Option<Program>>,
}

impl Watcher {
    pub fn start(ctx: egui::Context) -> Self {
        let on = Arc::new(AtomicBool::new(false));
        let (tx, changed) = channel();
        let watching = on.clone();
        std::thread::Builder::new()
            .name("program-watcher".into())
            .spawn(move || {
                // Process of the window in front as last sent; `None`: send the next one.
                let mut last: Option<Option<u32>> = None;
                loop {
                    std::thread::sleep(INTERVAL);
                    if !watching.load(Ordering::Relaxed) {
                        last = None;
                        continue;
                    }
                    let front = crate::platform::foreground_program();
                    let pid = front.as_ref().map(|p| p.pid);
                    if last != Some(pid) {
                        last = Some(pid);
                        // What switching saw, for reports of a game that did not switch.
                        match &front {
                            Some(p) => log::debug!(
                                "program in front: {} (pid {}, {})",
                                p.name,
                                p.pid,
                                p.path.display()
                            ),
                            None => log::debug!(
                                "program in front: none readable (desktop, or a window this \
                                 system does not name)"
                            ),
                        }
                        if tx.send(front).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                }
            })
            .expect("program watcher thread");
        Self { on, changed }
    }

    pub fn set_enabled(&self, on: bool) {
        self.on.store(on, Ordering::Relaxed);
    }

    /// The program in front, when it changed since the last call.
    pub fn changed(&self) -> Option<Option<Program>> {
        self.changed.try_iter().last()
    }
}

/// The program of the Applications page whose profile the program in front asks for; for any
/// other program (or the desktop) the "any other program" entry when it has a profile.
/// `own`: Joystick Gobloon's file name, which never gets a profile and keeps the running one.
pub fn choose<'a>(
    front: Option<&Program>,
    apps: &'a [AppEntry],
    own: &str,
) -> Option<&'a AppEntry> {
    let other = || apps.iter().find(|a| a.is_other() && a.profile.is_some());
    let Some(front) = front else {
        return other();
    };
    let key = program_key(&front.name);
    if key == program_key(own) {
        return None;
    }
    apps.iter()
        .find(|a| !a.is_other() && a.profile.is_some() && a.key() == key)
        .or_else(other)
}

/// Remembers which program chose the running profile, so it is applied once: coming back to
/// that program (alt-tab) keeps the profile as it runs, mode changes included.
#[derive(Default)]
pub struct Switcher {
    /// [`AppEntry::key`] of the program whose profile was applied last.
    applied: Option<String>,
    /// The program whose profile could not be applied; tried again when it comes to the front
    /// again.
    failed: Option<String>,
}

impl Switcher {
    /// Forgets the last program: the next check applies the program in front (Activate).
    pub fn reset(&mut self) {
        self.applied = None;
        self.failed = None;
    }

    /// Another program came to the front: a failed one may be tried again.
    pub fn front_changed(&mut self) {
        self.failed = None;
    }

    /// `app`'s profile runs now.
    pub fn applied(&mut self, app: &AppEntry) {
        self.applied = Some(app.key());
        self.failed = None;
    }

    /// `app`'s profile could not be applied.
    pub fn failed(&mut self, app: &AppEntry) {
        self.failed = Some(app.key());
    }

    /// The program whose profile [`Self::check`] would apply now, without applying it.
    pub fn due<'a>(
        &self,
        front: Option<&Program>,
        apps: &'a [AppEntry],
        own: &str,
    ) -> Option<&'a AppEntry> {
        choose(front, apps, own).filter(|app| {
            let key = Some(app.key());
            self.applied != key && self.failed != key
        })
    }
}

/// A controller of the profile and the attached device standing in for it (itself, or one of
/// the same name for a profile from another PC); `None`: not attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Served {
    pub id: String,
    pub name: String,
    pub by: Option<String>,
}

/// Which attached device serves each controller of the profile.
pub fn served(profile: &gob_core::Profile, devices: &[gob_input::DeviceState]) -> Vec<Served> {
    let aliases = crate::substitute_view::auto_aliases(profile, devices);
    profile
        .devices
        .iter()
        .filter(|d| !gob_core::is_builtin_device(&d.id))
        .map(|d| {
            let own = devices
                .iter()
                .find(|s| s.info.guid.eq_ignore_ascii_case(&d.id))
                .map(|s| s.info.guid.clone());
            Served {
                id: d.id.clone(),
                name: d.name.trim().to_string(),
                by: own.or_else(|| aliases.get(&d.id.to_ascii_lowercase()).cloned()),
            }
        })
        .collect()
}

/// How a controller of the running profile changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceChange {
    Disconnected(String),
    Connected(String),
    /// Attached again as another device (a new id).
    Replaced(String),
}

/// The changes from `before` to `after`; controllers only in one of them (the profile was
/// edited) do not count.
pub fn device_changes(before: &[Served], after: &[Served]) -> Vec<DeviceChange> {
    after
        .iter()
        .filter_map(|a| {
            let b = before.iter().find(|b| b.id == a.id)?;
            let name = a.name.clone();
            match (&b.by, &a.by) {
                (Some(_), None) => Some(DeviceChange::Disconnected(name)),
                (None, Some(_)) => Some(DeviceChange::Connected(name)),
                (Some(x), Some(y)) if !x.eq_ignore_ascii_case(y) => {
                    Some(DeviceChange::Replaced(name))
                }
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
