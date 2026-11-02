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

/// The program of the Applications page whose profile the program in front asks for.
/// `own`: Joystick Gobloon's file name, which never gets a profile.
pub fn choose<'a>(
    front: Option<&Program>,
    apps: &'a [AppEntry],
    own: &str,
) -> Option<&'a AppEntry> {
    let key = program_key(&front?.name);
    if key == program_key(own) {
        return None;
    }
    apps.iter().find(|a| a.profile.is_some() && a.key() == key)
}

/// Remembers which program chose the running profile, so it is applied once: coming back to
/// that program (alt-tab) keeps the profile as it runs, mode changes included.
#[derive(Default)]
pub struct Switcher {
    /// [`AppEntry::key`] of the program whose profile was applied last.
    applied: Option<String>,
}

impl Switcher {
    /// Forgets the last program: the next check applies the program in front (Activate).
    pub fn reset(&mut self) {
        self.applied = None;
    }

    /// The program whose profile should be applied now; `None`: keep the running one.
    pub fn check<'a>(
        &mut self,
        front: Option<&Program>,
        apps: &'a [AppEntry],
        own: &str,
    ) -> Option<&'a AppEntry> {
        let app = choose(front, apps, own)?;
        let key = app.key();
        if self.applied.as_ref() == Some(&key) {
            return None;
        }
        self.applied = Some(key);
        Some(app)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn app(path: &str, profile: Option<&str>) -> AppEntry {
        let mut a = AppEntry::new(PathBuf::from(path));
        a.profile = profile.map(PathBuf::from);
        a
    }

    fn program(name: &str) -> Program {
        Program::new(1, PathBuf::from(format!("C:/Other/{name}")))
    }

    const OWN: &str = "JoystickGobloon.exe";

    fn apps() -> Vec<AppEntry> {
        vec![
            app("C:/Games/One/One.exe", Some("one.xml")),
            app("C:/Games/Two/Two.exe", Some("two.xml")),
            app("C:/Tools/radio.exe", None),
            app("C:/Tools/JoystickGobloon.exe", Some("own.xml")),
        ]
    }

    #[test]
    fn the_program_in_front_chooses_by_file_name() {
        let apps = apps();
        let chosen = choose(Some(&program("ONE.exe")), &apps, OWN).unwrap();
        assert_eq!(chosen.profile, Some(PathBuf::from("one.xml")));
        assert!(
            choose(Some(&program("radio.exe")), &apps, OWN).is_none(),
            "no profile"
        );
        assert!(
            choose(Some(&program("browser.exe")), &apps, OWN).is_none(),
            "not listed"
        );
        assert!(
            choose(Some(&program(OWN)), &apps, OWN).is_none(),
            "never itself"
        );
        assert!(choose(None, &apps, OWN).is_none(), "nothing in front");
    }

    #[test]
    fn a_profile_is_applied_once_per_program() {
        let apps = apps();
        let mut switcher = Switcher::default();
        let (one, two) = (program("One.exe"), program("Two.exe"));
        assert!(switcher.check(Some(&one), &apps, OWN).is_some());
        // Alt-tab to a browser and back: the running profile stays as it is.
        assert!(
            switcher
                .check(Some(&program("browser.exe")), &apps, OWN)
                .is_none()
        );
        assert!(switcher.check(Some(&one), &apps, OWN).is_none());
        // Another game in front switches; back to the first switches again.
        assert!(switcher.check(Some(&two), &apps, OWN).is_some());
        assert!(switcher.check(Some(&one), &apps, OWN).is_some());
        // Activate applies the program in front even when it chose last time.
        switcher.reset();
        assert!(switcher.check(Some(&one), &apps, OWN).is_some());
    }
}
