//! Profile activation shared by the main window and the tray menu. eframe does not run the
//! app's `update` while the window is hidden in the tray, so the tray menu starts and stops
//! the engine through this object instead of asking the app.
//!
//! Starting and reloading run on a worker thread of their own (building the engine runs the
//! profile's scripts, taking the virtual devices talks to drivers or System Gobloon), so the
//! window never waits for them; each result comes back as a [`Done`]. Stopping is immediate.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui;
use gob_core::Profile;
use gob_engine::{LogicalValues, Runner, RunnerStatus};
use gob_input::InputHandle;

#[derive(Clone)]
pub struct Activation {
    inner: Arc<Mutex<Inner>>,
    worker: Sender<Request>,
}

/// What a start or reload was asked for, so the window knows what its result means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    /// Activate in the window.
    Start,
    /// The edited profile or a changed script file handed to the running engine.
    Edit,
    /// Another profile because another program came to the front.
    Switch,
    /// Activate from the tray menu.
    Tray,
}

/// A finished start or reload.
pub struct Done {
    pub job: Job,
    pub result: Result<(), String>,
}

enum Request {
    Start(Job),
    /// `fresh`: another profile takes over (stop and start events run), not an edit.
    Reload(Job, bool),
}

struct Inner {
    runner: Option<Runner>,
    /// The profile "Activate" starts, as the window last saw it, with its file name.
    profile: Option<(Profile, String)>,
    input: InputHandle,
    ctx: egui::Context,
    /// Finished starts and reloads, for the window (a tray start's error shows once the window
    /// is visible).
    done: Vec<Done>,
    /// A start is on its way to the worker or running there.
    starting: bool,
    /// Counts stops: a start that finishes after a stop is dropped.
    generation: u64,
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
        let inner = Arc::new(Mutex::new(Inner {
            runner: None,
            profile: None,
            input,
            ctx,
            done: Vec::new(),
            starting: false,
            generation: 0,
            led_defaults: None,
            voice: None,
            start_mode: None,
            folder: None,
            file_errors: Vec::new(),
            script_files: Vec::new(),
        }));
        let (worker, requests) = std::sync::mpsc::channel();
        {
            let inner = inner.clone();
            let spawned = std::thread::Builder::new()
                .name("activation".into())
                .spawn(move || work(&inner, &requests));
            if let Err(e) = spawned {
                log::error!("activation thread: {e}");
            }
        }
        Self { inner, worker }
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
        lock(&self.inner)
    }

    /// A profile runs, or is starting.
    pub fn is_active(&self) -> bool {
        let inner = self.lock();
        inner.runner.is_some() || inner.starting
    }

    /// What the running profile sends to the virtual devices and the keyboard.
    pub fn outputs(&self) -> Option<gob_engine::OutputTap> {
        self.lock().runner.as_ref().map(gob_engine::Runner::outputs)
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

    /// Stops the engine (or a start on its way); true if it was running or starting.
    pub fn stop(&self) -> bool {
        let (runner, starting) = {
            let mut inner = self.lock();
            inner.generation += 1;
            let starting = std::mem::take(&mut inner.starting);
            (inner.runner.take(), starting)
        };
        let stopped = runner.is_some() || starting;
        if stopped {
            log::info!("profile stopped");
        }
        stopped
    }

    /// Starts the engine on the current profile, or hands it to the running one (its stop
    /// actions, then the start of this one). The result comes as a [`Done`].
    pub fn start(&self, job: Job) {
        {
            let mut inner = self.lock();
            if inner.runner.is_none() {
                inner.starting = true;
            }
        }
        self.send(Request::Start(job));
    }

    /// Hands the current profile to the running engine, which keeps its virtual devices (a game
    /// keeps them too); not running: nothing happens. On an error the old profile keeps running.
    /// `fresh`: another profile takes over (stop and start events run), not an edit.
    pub fn reload(&self, job: Job, fresh: bool) {
        self.send(Request::Reload(job, fresh));
    }

    fn send(&self, request: Request) {
        let job = match &request {
            Request::Start(job) | Request::Reload(job, _) => *job,
        };
        if self.worker.send(request).is_err() {
            finish(
                &self.inner,
                job,
                Some(Err("the activation thread has stopped".into())),
            );
        }
    }

    /// The next finished start or reload.
    pub fn take_done(&self) -> Option<Done> {
        let mut inner = self.lock();
        (!inner.done.is_empty()).then(|| inner.done.remove(0))
    }

    /// Start or stop, for the tray menu: a failed start waits as a [`Done`] for the window.
    pub fn toggle_from_tray(&self) {
        if !self.stop() {
            self.start(Job::Tray);
        }
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

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|e| e.into_inner())
}

/// The worker: starts and reloads in the order they were asked for.
fn work(inner: &Arc<Mutex<Inner>>, requests: &Receiver<Request>) {
    for request in requests {
        let (job, result) = match request {
            Request::Start(job) => (job, start_now(inner)),
            Request::Reload(job, fresh) => (job, reload_now(inner, fresh)),
        };
        finish(inner, job, result);
    }
}

/// Hands a result to the window; `None` (stopped meanwhile, nothing running) is no result.
fn finish(inner: &Mutex<Inner>, job: Job, result: Option<Result<(), String>>) {
    let Some(result) = result else {
        return;
    };
    if let Err(e) = &result {
        log::error!("activating the profile ({job:?}): {e}");
    }
    let mut inner = lock(inner);
    inner.done.push(Done { job, result });
    inner.ctx.request_repaint();
}

/// What a start or reload takes from the shared state, so the slow part runs unlocked.
struct Prepared {
    profile: Profile,
    name: String,
    folder: Option<std::path::PathBuf>,
    devices: Vec<gob_input::DeviceState>,
    generation: u64,
}

fn prepare(inner: &Inner) -> Option<Prepared> {
    let (profile, name) = inner.profile.as_ref()?;
    Some(Prepared {
        profile: starting_in(profile, inner.start_mode.as_deref()).into_owned(),
        name: name.clone(),
        folder: inner.folder.clone(),
        devices: inner.input.devices(),
        generation: inner.generation,
    })
}

/// Longest a start waits for the input thread's first look for devices (a profile activated
/// as the program starts): the profile's devices, their positions and substitutions need it.
const SCAN_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

fn start_now(shared: &Mutex<Inner>) -> Option<Result<(), String>> {
    let input = lock(shared).input.clone();
    let waited = std::time::Instant::now();
    while !input.scanned() && waited.elapsed() < SCAN_WAIT {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (prepared, events, led_defaults, voice, ctx) = {
        let mut inner = lock(shared);
        if inner.runner.is_some() {
            drop(inner);
            return reload_now(shared, true);
        }
        if !inner.starting {
            return None; // stopped before it began
        }
        let Some(prepared) = prepare(&inner) else {
            inner.starting = false;
            return Some(Err("no profile loaded".into()));
        };
        let events = inner.input.subscribe_timed();
        (
            prepared,
            events,
            inner.led_defaults.clone(),
            inner.voice.clone(),
            inner.ctx.clone(),
        )
    };
    let (profile, file_errors, files) = with_script_files(
        std::borrow::Cow::Owned(prepared.profile),
        prepared.folder.as_deref(),
    );
    let aliases = crate::substitute_view::auto_aliases(&profile, &prepared.devices);
    let started = Runner::start(
        &profile,
        &aliases,
        events,
        &prepared.devices,
        led_defaults.as_deref(),
        voice,
        move || ctx.request_repaint(),
    );
    let mut inner = lock(shared);
    if inner.generation != prepared.generation {
        return None; // stopped while starting: the new runner stops as it is dropped
    }
    inner.starting = false;
    let runner = match started {
        Ok(runner) => runner,
        Err(e) => return Some(Err(e)),
    };
    log::info!(
        "profile {} active, {} device substitution(s)",
        prepared.name,
        aliases.len()
    );
    inner.runner = Some(runner);
    inner.file_errors = file_errors;
    inner.script_files = files;
    Some(Ok(()))
}

fn reload_now(shared: &Mutex<Inner>, fresh: bool) -> Option<Result<(), String>> {
    // Nothing running or no profile: nothing to hand over, which is fine.
    let (prepared, reloader) = {
        let inner = lock(shared);
        let (Some(runner), Some(prepared)) = (inner.runner.as_ref(), prepare(&inner)) else {
            return Some(Ok(()));
        };
        (prepared, runner.reloader())
    };
    let (profile, file_errors, files) = with_script_files(
        std::borrow::Cow::Owned(prepared.profile),
        prepared.folder.as_deref(),
    );
    let aliases = crate::substitute_view::auto_aliases(&profile, &prepared.devices);
    if let Err(e) = reloader.reload(&profile, &aliases, &prepared.devices, fresh) {
        return Some(Err(e));
    }
    let mut inner = lock(shared);
    if inner.generation == prepared.generation {
        inner.file_errors = file_errors;
        inner.script_files = files;
    }
    log::info!(
        "profile {} reloaded, {} device substitution(s)",
        prepared.name,
        aliases.len()
    );
    Some(Ok(()))
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
