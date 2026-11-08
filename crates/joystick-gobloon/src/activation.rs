//! Profile activation shared by the main window and the tray menu. eframe does not run the
//! app's `update` while the window is hidden in the tray, so the tray menu starts and stops
//! the engine through this object instead of asking the app.

use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui;
use gob_core::Profile;
use gob_engine::{LogicalValues, Runner, RunnerStatus};
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
    /// Default LED colors, set when a profile that changed them stops; `None`: LED control
    /// is off (experimental features off).
    led_defaults: Option<Vec<gob_engine::Light>>,
    /// How voice commands run; `None`: voice-input actions do nothing.
    voice: Option<gob_engine::VoiceConfig>,
    /// Mode the engine starts in instead of the profile's starting mode (the mode a program
    /// of the Applications page chose for its profile).
    start_mode: Option<String>,
    /// The profile's folder: relative script file paths start there.
    folder: Option<std::path::PathBuf>,
    /// Script files that could not be read on the last start or reload.
    file_errors: Vec<String>,
    /// The running profile's script files as they were read.
    script_files: ScriptFiles,
}

impl Activation {
    pub fn new(input: InputHandle, ctx: egui::Context) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            runner: None,
            profile: None,
            input,
            ctx,
            error: None,
            led_defaults: None,
            voice: None,
            start_mode: None,
            folder: None,
            file_errors: Vec::new(),
            script_files: Vec::new(),
        })))
    }

    /// Default LED colors for the next start (a running profile keeps its own); `None`
    /// turns LED control off.
    pub fn set_led_defaults(&self, defaults: Option<Vec<gob_engine::Light>>) {
        self.lock().led_defaults = defaults;
    }

    /// Voice commands for the next start (a running profile keeps its own).
    pub fn set_voice(&self, voice: Option<gob_engine::VoiceConfig>) {
        self.lock().voice = voice;
    }

    /// Mode the next start or reload begins in; `None`: the profile's starting mode.
    pub fn set_start_mode(&self, mode: Option<String>) {
        self.lock().start_mode = mode;
    }

    pub fn start_mode(&self) -> Option<String> {
        self.lock().start_mode.clone()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_active(&self) -> bool {
        self.lock().runner.is_some()
    }

    /// The profile's folder, for its external script files.
    pub fn set_profile_folder(&self, folder: Option<std::path::PathBuf>) {
        self.lock().folder = folder;
    }

    /// A script file of the running profile changed since it was read.
    pub fn script_files_changed(&self) -> bool {
        let inner = self.lock();
        inner.runner.is_some() && inner.script_files.iter().any(|(p, m)| modified(p) != *m)
    }

    /// Script problems since the last call: files that could not be read and the scripts' own
    /// errors (a script that did not load, a running script's first error).
    pub fn take_script_errors(&self) -> Vec<String> {
        let mut inner = self.lock();
        let mut out = std::mem::take(&mut inner.file_errors);
        if let Some(runner) = &inner.runner {
            out.extend(runner.take_script_errors().iter().map(ToString::to_string));
        }
        out
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

    /// Starts the engine on the current profile. A running one hands over in its own thread
    /// (its stop actions, then the start of this one), so the window never waits for it.
    pub fn start(&self) -> Result<(), String> {
        if self.lock().runner.is_some() {
            return self.reload(true);
        }
        let mut inner = self.lock();
        let Some((profile, name)) = &inner.profile else {
            return Err("no profile loaded".into());
        };
        let profile = starting_in(profile, inner.start_mode.as_deref());
        let (profile, file_errors, files) = with_script_files(profile, inner.folder.as_deref());
        let profile = &*profile;
        let devices = inner.input.devices();
        let aliases = crate::substitute_view::auto_aliases(profile, &devices);
        let repaint = {
            let ctx = inner.ctx.clone();
            move || ctx.request_repaint()
        };
        let runner = Runner::start(
            profile,
            &aliases,
            inner.input.subscribe_timed(),
            &devices,
            inner.led_defaults.as_deref(),
            inner.voice.clone(),
            repaint,
        )?;
        log::info!(
            "profile {name} active, {} device substitution(s)",
            aliases.len()
        );
        inner.runner = Some(runner);
        inner.file_errors = file_errors;
        inner.script_files = files;
        Ok(())
    }

    /// Hands the current profile to the running engine, which keeps its virtual devices (a game
    /// keeps them too); not running: nothing to do. On an error the old profile keeps running.
    /// `fresh`: another profile takes over (stop and start events run), not an edit.
    pub fn reload(&self, fresh: bool) -> Result<(), String> {
        let mut inner = self.lock();
        let (Some(runner), Some((profile, name))) = (&inner.runner, &inner.profile) else {
            return Ok(());
        };
        let profile = starting_in(profile, inner.start_mode.as_deref());
        let (profile, file_errors, files) = with_script_files(profile, inner.folder.as_deref());
        let profile = &*profile;
        let devices = inner.input.devices();
        let aliases = crate::substitute_view::auto_aliases(profile, &devices);
        runner.reload(profile, &aliases, &devices, fresh)?;
        let name = name.clone();
        inner.file_errors = file_errors;
        inner.script_files = files;
        log::info!(
            "profile {name} reloaded, {} device substitution(s)",
            aliases.len()
        );
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

    /// Values of the logical device while a profile runs.
    pub fn logical(&self) -> Option<LogicalValues> {
        self.lock().runner.as_ref().map(|r| r.logical())
    }

    /// A "load profile" action asked for this file.
    pub fn take_load_request(&self) -> Option<String> {
        self.lock()
            .runner
            .as_ref()
            .and_then(|r| r.take_load_request())
    }
}

/// The running profile's external script files with their modification time when read.
type ScriptFiles = Vec<(std::path::PathBuf, Option<std::time::SystemTime>)>;

fn modified(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The profile with the code of its external Lua script files read in (the engine reads no
/// files), the files that could not be read, and the files with their modification time.
fn with_script_files<'a>(
    profile: std::borrow::Cow<'a, Profile>,
    folder: Option<&std::path::Path>,
) -> (std::borrow::Cow<'a, Profile>, Vec<String>, ScriptFiles) {
    let files: ScriptFiles = profile
        .script_instances()
        .into_iter()
        .filter(|s| s.language == gob_core::ScriptLanguage::Lua)
        .filter_map(|s| match s.source {
            gob_core::ScriptSource::File(p) => Some(gob_core::script_path(&p, folder)),
            gob_core::ScriptSource::Code(_) => None,
        })
        .map(|p| {
            let m = modified(&p);
            (p, m)
        })
        .collect();
    if files.is_empty() {
        return (profile, Vec::new(), files);
    }
    let mut profile = profile.into_owned();
    let failed = profile
        .read_script_files(folder)
        .into_iter()
        .map(|(name, e)| {
            log::warn!("script {name}: {e}");
            format!("{name}: {e}")
        })
        .collect();
    (std::borrow::Cow::Owned(profile), failed, files)
}

/// `profile`, starting in `mode` when it has that mode.
fn starting_in<'a>(profile: &'a Profile, mode: Option<&str>) -> std::borrow::Cow<'a, Profile> {
    match mode {
        Some(mode)
            if profile.startup_mode().as_deref() != Some(mode)
                && profile.modes.iter().any(|m| m.name == mode) =>
        {
            let mut profile = profile.clone();
            profile.set_startup_mode(mode);
            std::borrow::Cow::Owned(profile)
        }
        _ => std::borrow::Cow::Borrowed(profile),
    }
}
