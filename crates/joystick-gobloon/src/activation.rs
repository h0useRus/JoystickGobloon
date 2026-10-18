//! Profile activation shared by the main window and the tray menu. eframe does not run the
//! app's `update` while the window is hidden in the tray, so the tray menu starts and stops
//! the engine through this object instead of asking the app.

use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui;
use gob_core::Profile;
use gob_engine::{Runner, RunnerStatus};
use gob_input::InputHandle;

#[derive(Clone)]
pub struct Activation(Arc<Mutex<Inner>>);

struct Inner {
    runner: Option<Runner>,
    /// The profile "Activate" starts, as the window last saw it, with its file name.
    profile: Option<(Profile, String)>,
    input: InputHandle,
    ctx: egui::Context,
    /// A start from the tray failed; the app shows it once the window is visible.
    error: Option<String>,
}

impl Activation {
    pub fn new(input: InputHandle, ctx: egui::Context) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            runner: None,
            profile: None,
            input,
            ctx,
            error: None,
        })))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_active(&self) -> bool {
        self.lock().runner.is_some()
    }

    /// The profile to start next; cloned only when it changed.
    pub fn set_profile(&self, profile: Option<(&Profile, String)>) {
        let mut inner = self.lock();
        let same = match (&inner.profile, &profile) {
            (Some((a, an)), Some((b, bn))) => a == *b && an == bn,
            (None, None) => true,
            _ => false,
        };
        if !same {
            inner.profile = profile.map(|(p, n)| (p.clone(), n));
        }
    }

    /// File name of the profile "Activate" starts.
    pub fn profile_name(&self) -> Option<String> {
        self.lock().profile.as_ref().map(|(_, n)| n.clone())
    }

    /// Stops the engine; true if it was running.
    pub fn stop(&self) -> bool {
        let stopped = self.lock().runner.take().is_some();
        if stopped {
            log::info!("profile stopped");
        }
        stopped
    }

    /// Starts the engine on the current profile (stopping a running one first).
    pub fn start(&self) -> Result<(), String> {
        let mut inner = self.lock();
        inner.runner = None;
        let Some((profile, name)) = &inner.profile else {
            return Err("no profile loaded".into());
        };
        let devices = inner.input.devices();
        let aliases = crate::substitute_view::auto_aliases(profile, &devices);
        let repaint = {
            let ctx = inner.ctx.clone();
            move || ctx.request_repaint()
        };
        let runner = Runner::start(
            profile,
            &aliases,
            inner.input.subscribe(),
            &devices,
            repaint,
        )?;
        log::info!(
            "profile {name} active, {} device substitution(s)",
            aliases.len()
        );
        inner.runner = Some(runner);
        Ok(())
    }

    /// Start or stop, for the tray menu: a failed start is kept for [`Self::take_error`].
    pub fn toggle_from_tray(&self) {
        if self.stop() {
            return;
        }
        if let Err(e) = self.start() {
            log::error!("tray: activating the profile: {e}");
            self.lock().error = Some(e);
        }
    }

    pub fn take_error(&self) -> Option<String> {
        self.lock().error.take()
    }

    pub fn status(&self) -> Option<RunnerStatus> {
        self.lock().runner.as_ref().map(|r| r.status())
    }

    /// A "load profile" action asked for this file.
    pub fn take_load_request(&self) -> Option<String> {
        self.lock()
            .runner
            .as_ref()
            .and_then(|r| r.take_load_request())
    }
}
