//! Runs the engine on a background thread against real devices.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gob_core::Profile;
use gob_input::{DeviceState, InputEvent, Timed};

use crate::platform::Injector;
use crate::{Engine, GamepadControl, InputKey, InputKind, LogicalValues, Output, Value};

/// Upper bound on how long the loop sleeps without events.
const IDLE: Duration = Duration::from_millis(20);
/// [`Latency`] covers this much time.
const LATENCY_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct RunnerStatus {
    pub mode: String,
    pub paused: bool,
    /// Profile file a load-profile action asked for; the GUI takes it.
    pub load_profile: Option<String>,
    /// Of the last window with input; `None` until the first.
    pub latency: Option<Latency>,
    /// Voice commands; `None` when the profile has no voice-input action.
    pub voice: Option<VoiceStatus>,
    /// Script errors the GUI has not taken yet ([`Runner::take_script_errors`]).
    pub script_errors: Vec<crate::ScriptError>,
}

/// Voice commands of the running profile.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoiceStatus {
    /// The speech model is loaded.
    pub ready: bool,
    /// No speech model was given ([`Runner::start`]'s `voice`): voice-input actions do nothing.
    pub no_model: bool,
    /// Why the speech model or library could not be loaded (English, for the log).
    pub error: Option<String>,
    /// A microphone is listening.
    pub listening: bool,
    /// The last utterance.
    pub heard: Option<VoiceHeard>,
    /// Phrases with words the speech model does not know (shared: cloned with every status).
    pub unknown_words: std::sync::Arc<[gob_voice::UnknownPhrase]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceHeard {
    pub text: String,
    pub confidence: f32,
    /// The command it triggered; `None` = not recognized.
    pub command: Option<u32>,
}

/// Time from reading a device to the end of the outputs its event caused (vJoy written,
/// keys sent), over the events of one [`LATENCY_WINDOW`]. The device's own delay is not in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Latency {
    pub average: Duration,
    pub max: Duration,
}

/// Collects [`Latency`] for the current window.
struct LatencyWindow {
    start: Instant,
    total: Duration,
    max: Duration,
    count: u32,
}

impl LatencyWindow {
    fn new(now: Instant) -> Self {
        Self {
            start: now,
            total: Duration::ZERO,
            max: Duration::ZERO,
            count: 0,
        }
    }

    fn add(&mut self, latency: Duration) {
        self.total += latency;
        self.max = self.max.max(latency);
        self.count += 1;
    }

    /// The finished window's result, once it is over and had events; then starts a new one.
    fn finish(&mut self, now: Instant) -> Option<Latency> {
        if now.duration_since(self.start) < LATENCY_WINDOW {
            return None;
        }
        let result = (self.count > 0).then(|| Latency {
            average: self.total / self.count,
            max: self.max,
        });
        *self = Self::new(now);
        result
    }
}

pub struct Runner {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    status: Arc<Mutex<RunnerStatus>>,
    logical: Arc<Mutex<LogicalValues>>,
    reload: Sender<Reload>,
}

/// A new engine for the running thread, with the device state to start it from; the thread
/// answers whether it took it.
struct Reload {
    engine: Engine,
    initial: Vec<DeviceState>,
    /// Another profile takes over (its start events run), not an edit of the running one.
    fresh: bool,
    reply: Sender<Result<(), String>>,
}

/// How long the actions of the stop and mode leave events may take before the profile's
/// outputs are let go.
const STOP_BUDGET: Duration = Duration::from_millis(500);

/// Engine threads of dropped runners that may still be running their stop actions; they hold
/// the virtual devices until they end.
static STOPPING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

/// Waits for the engine threads of stopped runners: their stop actions are done and the virtual
/// devices are let go. [`Runner::start`] does it first; call it before the program exits.
pub fn wait_stopped() {
    let threads = std::mem::take(&mut *STOPPING.lock().unwrap_or_else(|e| e.into_inner()));
    for t in threads {
        let _ = t.join();
    }
}

/// How long [`Runner::reload`] waits for the engine thread.
const RELOAD_TIMEOUT: Duration = Duration::from_secs(5);

struct SystemOutput {
    devices: HashMap<u32, Box<dyn gob_virtual::Device>>,
    gamepads: HashMap<u32, Box<dyn gob_virtual::Gamepad>>,
    /// Last values written, read back by vJoy conditions; a write of the same value again is
    /// not sent.
    written: HashMap<(u32, InputKind, u32), Value>,
    /// Writes of this pass of the loop, sent per device by [`SystemOutput::flush`]: one driver
    /// call per device instead of one per value. Buffers are kept between passes.
    pending: Vec<(u32, Vec<gob_virtual::Write>)>,
    pending_pads: Vec<(u32, Vec<gob_virtual::PadWrite>)>,
    /// Outputs whose last write the driver refused; logged once until one succeeds again.
    failing: HashSet<(u32, InputKind, u32)>,
    /// Writes the driver did not take (refused, or no device held): sent again even when the
    /// value is unchanged.
    unsent: HashSet<(u32, InputKind, u32)>,
    failing_pads: HashSet<(u32, Option<GamepadControl>)>,
    media: crate::media::Media,
    lighting: crate::lighting::Lighting,
    load_profile: Option<String>,
    /// Started for the first profile with voice-input actions; kept across reloads.
    voice: Option<gob_voice::VoiceService>,
    voice_config: Option<gob_voice::VoiceConfig>,
    voice_events: std::sync::mpsc::Sender<gob_voice::VoiceEvent>,
}

/// The joystick input a write changes.
fn write_key(device: u32, write: gob_virtual::Write) -> ((u32, InputKind, u32), Value) {
    use gob_virtual::Write;
    match write {
        Write::Axis(a, v) => ((device, InputKind::Axis, u32::from(a)), Value::Axis(v)),
        Write::Button(b, p) => ((device, InputKind::Button, u32::from(b)), Value::Button(p)),
        Write::Hat(h, d) => ((device, InputKind::Hat, u32::from(h)), Value::Hat(d)),
    }
}

/// The gamepad control a write changes; the d-pad's directions and the d-pad as a whole count
/// as one (`None`), since they change the same state.
fn pad_key(write: gob_virtual::PadWrite) -> Option<GamepadControl> {
    use gob_virtual::PadWrite;
    match write {
        PadWrite::Button(
            GamepadControl::DpadUp
            | GamepadControl::DpadDown
            | GamepadControl::DpadLeft
            | GamepadControl::DpadRight,
            _,
        )
        | PadWrite::Dpad(_) => None,
        PadWrite::Button(c, _) | PadWrite::Axis(c, _) => Some(c),
    }
}

impl SystemOutput {
    fn new(
        lighting: crate::lighting::Lighting,
        voice_config: Option<gob_voice::VoiceConfig>,
        voice_events: std::sync::mpsc::Sender<gob_voice::VoiceEvent>,
    ) -> Self {
        Self {
            devices: HashMap::new(),
            gamepads: HashMap::new(),
            written: HashMap::new(),
            pending: Vec::new(),
            pending_pads: Vec::new(),
            failing: HashSet::new(),
            unsent: HashSet::new(),
            failing_pads: HashSet::new(),
            media: Default::default(),
            lighting,
            load_profile: None,
            voice: None,
            voice_config,
            voice_events,
        }
    }

    /// Queues a joystick write for [`Self::flush`]. Unchanged values are dropped; a newer axis
    /// value replaces a queued one, but a button or hat changing again is sent in between, so
    /// a short press still reaches the game.
    fn queue(&mut self, device: u32, write: gob_virtual::Write) {
        let (key, value) = write_key(device, write);
        if self.written.get(&key) == Some(&value) && !self.unsent.contains(&key) {
            return;
        }
        self.written.insert(key, value);
        let i = match self.pending.iter().position(|(d, _)| *d == device) {
            Some(i) => i,
            None => {
                self.pending.push((device, Vec::new()));
                self.pending.len() - 1
            }
        };
        let queued = self.pending[i]
            .1
            .iter()
            .position(|&w| write_key(device, w).0 == key);
        match (queued, write) {
            (Some(q), gob_virtual::Write::Axis(..)) => self.pending[i].1[q] = write,
            (Some(_), _) => {
                self.send(i);
                self.pending[i].1.push(write);
            }
            (None, _) => self.pending[i].1.push(write),
        }
    }

    /// Queues a gamepad write like [`Self::queue`], without dropping unchanged values (the
    /// d-pad changes both as a whole and by direction).
    fn queue_pad(&mut self, gamepad: u32, write: gob_virtual::PadWrite) {
        let i = match self.pending_pads.iter().position(|(g, _)| *g == gamepad) {
            Some(i) => i,
            None => {
                self.pending_pads.push((gamepad, Vec::new()));
                self.pending_pads.len() - 1
            }
        };
        let key = pad_key(write);
        let queued = self.pending_pads[i]
            .1
            .iter()
            .position(|&w| pad_key(w) == key);
        match (queued, write) {
            (Some(q), gob_virtual::PadWrite::Axis(..)) => self.pending_pads[i].1[q] = write,
            (Some(_), _) => {
                self.send_pad(i);
                self.pending_pads[i].1.push(write);
            }
            (None, _) => self.pending_pads[i].1.push(write),
        }
    }

    /// Sends what this pass of the loop changed, one report per device.
    fn flush(&mut self) {
        for i in 0..self.pending.len() {
            self.send(i);
        }
        for i in 0..self.pending_pads.len() {
            self.send_pad(i);
        }
    }

    /// Sends the queued writes of `pending[i]`; a refused one is logged when it starts failing
    /// and when it works again.
    fn send(&mut self, i: usize) {
        let (device, writes) = &mut self.pending[i];
        if writes.is_empty() {
            return;
        }
        let device = *device;
        let target = self.devices.get(&device);
        let batch = target.map(|d| d.write(writes));
        for &w in writes.iter() {
            let key = write_key(device, w).0;
            // A refused batch: each write alone tells which ones the device took (those of the
            // batch it took are sent once more, which changes nothing).
            let ok = match batch {
                Some(false) => target.map(|d| d.write(std::slice::from_ref(&w))),
                ok => ok,
            };
            if ok != Some(true) {
                self.unsent.insert(key);
            } else if !self.unsent.is_empty() {
                self.unsent.remove(&key);
            }
            log_refused(&mut self.failing, key, ok, || {
                format!("virtual device {device}: writing {w:?}")
            });
        }
        writes.clear();
    }

    fn send_pad(&mut self, i: usize) {
        let (gamepad, writes) = &mut self.pending_pads[i];
        if writes.is_empty() {
            return;
        }
        let gamepad = *gamepad;
        let target = self.gamepads.get(&gamepad);
        let batch = target.map(|p| p.write(writes));
        for &w in writes.iter() {
            // As in `send`: a refused batch is told apart write by write.
            let ok = match batch {
                Some(false) => target.map(|p| p.write(std::slice::from_ref(&w))),
                ok => ok,
            };
            let control = match w {
                gob_virtual::PadWrite::Button(c, _) | gob_virtual::PadWrite::Axis(c, _) => Some(c),
                gob_virtual::PadWrite::Dpad(_) => None,
            };
            log_refused(&mut self.failing_pads, (gamepad, control), ok, || {
                let control = control.map_or("dpad", GamepadControl::name);
                format!("virtual gamepad {gamepad}: writing {control}")
            });
        }
        writes.clear();
    }
}

/// Logs a write the driver refused when it starts failing, and when it works again.
fn log_refused<K: Copy + Eq + std::hash::Hash>(
    failing: &mut HashSet<K>,
    key: K,
    ok: Option<bool>,
    what: impl FnOnce() -> String,
) {
    match ok {
        Some(false) if failing.insert(key) => log::warn!("{} failed", what()),
        Some(true) if failing.remove(&key) => log::info!("{} works again", what()),
        _ => {}
    }
}

impl Output for SystemOutput {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        self.queue(device, gob_virtual::Write::Axis(axis, value));
    }

    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        self.queue(device, gob_virtual::Write::Button(button, pressed));
    }

    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        self.queue(device, gob_virtual::Write::Hat(hat, direction));
    }

    fn gamepad_button(&mut self, gamepad: u32, control: GamepadControl, pressed: bool) {
        self.queue_pad(gamepad, gob_virtual::PadWrite::Button(control, pressed));
    }

    fn gamepad_axis(&mut self, gamepad: u32, control: GamepadControl, value: f32) {
        self.queue_pad(gamepad, gob_virtual::PadWrite::Axis(control, value));
    }

    fn gamepad_dpad(&mut self, gamepad: u32, direction: Option<u16>) {
        self.queue_pad(gamepad, gob_virtual::PadWrite::Dpad(direction));
    }

    fn play_sound(&mut self, file: &str, volume: u8) {
        self.media.play(file, volume);
    }

    fn light_led(&mut self, light: &crate::Light) {
        self.lighting.send(light);
    }

    fn speak(&mut self, request: crate::SpeechRequest) {
        self.media.speak(request);
    }

    fn load_profile(&mut self, file: &str) {
        self.load_profile = Some(file.to_string());
    }

    fn voice_listen(&mut self, microphone: &str, on: bool) {
        if let Some(voice) = &self.voice {
            voice.listen(microphone, on);
        }
    }

    fn vjoy_state(&self, device: u32, kind: InputKind, id: u32) -> Option<Value> {
        self.written.get(&(device, kind, id)).copied()
    }

    fn key_pressed(&self, scan_code: u16, extended: bool) -> bool {
        crate::platform::injector().key_pressed(scan_code, extended)
    }

    fn mouse_move(&mut self, dx: i32, dy: i32) {
        // Joystick writes queued before it go out first, in the profile's order.
        self.flush();
        crate::platform::injector().mouse_move(dx, dy);
    }

    fn mouse_button(&mut self, button: gob_core::MouseButton, pressed: bool) {
        // Joystick writes queued before it go out first, in the profile's order.
        self.flush();
        crate::platform::injector().mouse_button(button, pressed);
    }

    fn mouse_wheel(&mut self, notches: i32) {
        // Joystick writes queued before it go out first, in the profile's order.
        self.flush();
        crate::platform::injector().mouse_wheel(notches);
    }

    fn key(&mut self, scan_code: u16, extended: bool, pressed: bool) {
        // Joystick writes queued before it go out first, in the profile's order.
        self.flush();
        crate::platform::injector().key(scan_code, extended, pressed);
    }

    fn run_command(&mut self, executable: &str, arguments: &[String]) {
        // Detached: the program keeps running after Joystick Gobloon stops.
        let _ = std::process::Command::new(executable)
            .args(arguments)
            .spawn();
    }
}

impl Runner {
    /// Takes the vJoy devices the profile needs and starts processing `events`.
    /// `initial` is the current device state, used to send axis positions right away.
    /// `led_defaults` are set on the devices whose LEDs the profile changed when it stops;
    /// `None`: LED control is off and light-led actions do nothing. `voice` is how voice
    /// commands run; `None`: voice-input actions do nothing.
    pub fn start(
        profile: &Profile,
        aliases: &HashMap<String, String>,
        events: Receiver<Timed>,
        initial: &[DeviceState],
        led_defaults: Option<&[crate::Light]>,
        voice: Option<crate::VoiceConfig>,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        // A profile that just stopped may still hold the devices for its stop actions.
        wait_stopped();
        let mut engine = Engine::new(profile, aliases);
        let virtual_devices = gob_virtual::load().map_err(|e| e.to_string())?;
        let (voice_events, voice_inbox) = std::sync::mpsc::channel();
        let lighting = crate::lighting::Lighting::new(
            initial
                .iter()
                .filter(|d| !d.info.hid_path.is_empty())
                .map(|d| (d.info.guid.to_ascii_lowercase(), d.info.hid_path.clone()))
                .collect(),
            led_defaults.map(<[crate::Light]>::to_vec),
        );
        let mut out = SystemOutput::new(lighting, voice, voice_events);
        acquire(&engine, &mut out, virtual_devices.as_ref())?;
        feed_initial(&mut engine, &mut out, initial);
        set_up_voice(&engine, &mut out);
        if led_defaults.is_none() && engine.uses_lights() {
            log::info!("light-led actions ignored: experimental features are off");
        }
        crate::platform::Injector::prepare(crate::platform::injector());
        engine.activate(&mut out);
        engine.profile_started(Instant::now(), &mut out);
        out.flush();

        let status = Arc::new(Mutex::new(RunnerStatus {
            mode: engine.current_mode().to_string(),
            paused: false,
            load_profile: None,
            latency: None,
            voice: initial_voice_status(&engine, &out),
            script_errors: engine.take_script_errors(),
        }));
        engine.take_logical_changed();
        let logical = Arc::new(Mutex::new(engine.logical_values()));
        let stop = Arc::new(AtomicBool::new(false));
        let (reload, reloads) = std::sync::mpsc::channel();
        let thread = {
            let (status, logical, stop) = (status.clone(), logical.clone(), stop.clone());
            let threads = Threads {
                events,
                voice: voice_inbox,
                reloads,
                virtual_devices,
                status,
                logical,
                stop,
            };
            std::thread::Builder::new()
                .name("gob-engine".into())
                .spawn(move || run(engine, out, &threads, &on_change))
                .map_err(|e| e.to_string())?
        };
        Ok(Self {
            stop,
            thread: Some(thread),
            status,
            logical,
            reload,
        })
    }

    /// Replaces the running profile with `profile`, keeping the virtual devices: a game
    /// using them does not see them go. Devices the new profile needs are acquired, the
    /// others stay held until the runner stops. On an error the old profile keeps running.
    pub fn reload(
        &self,
        profile: &Profile,
        aliases: &HashMap<String, String>,
        initial: &[DeviceState],
        fresh: bool,
    ) -> Result<(), String> {
        let (reply, answer) = std::sync::mpsc::channel();
        let request = Reload {
            engine: Engine::new(profile, aliases),
            initial: initial.to_vec(),
            fresh,
            reply,
        };
        self.reload
            .send(request)
            .map_err(|_| "the engine has stopped".to_string())?;
        answer
            .recv_timeout(RELOAD_TIMEOUT)
            .map_err(|_| "the engine did not answer".to_string())?
    }

    /// Script errors since the last call: a script that did not load, or the first error of a
    /// running one.
    pub fn take_script_errors(&self) -> Vec<crate::ScriptError> {
        std::mem::take(
            &mut self
                .status
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .script_errors,
        )
    }

    /// Profile file requested by a load-profile action, once.
    pub fn take_load_request(&self) -> Option<String> {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .load_profile
            .take()
    }

    pub fn status(&self) -> RunnerStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Current values of the logical device's inputs.
    pub fn logical(&self) -> LogicalValues {
        self.logical
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Drop for Runner {
    /// Stops the engine thread without waiting for it: it runs the profile's stop actions and
    /// lets go of the devices on its own ([`wait_stopped`]).
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            STOPPING.lock().unwrap_or_else(|e| e.into_inner()).push(t);
        }
    }
}

/// The voice status a new engine starts with: `None` without voice-input actions.
fn initial_voice_status(engine: &Engine, out: &SystemOutput) -> Option<VoiceStatus> {
    engine.uses_voice().then(|| VoiceStatus {
        no_model: out.voice_config.is_none(),
        ..Default::default()
    })
}

/// Acquires the virtual devices `engine` writes to that `out` does not hold yet, then seeds
/// the engine with `initial` and sends the axis positions through it.
/// Takes the virtual devices `engine` writes that are not taken yet.
fn acquire(
    engine: &Engine,
    out: &mut SystemOutput,
    virtual_devices: &dyn gob_virtual::Backend,
) -> Result<(), String> {
    use std::collections::hash_map::Entry;
    for id in engine.vjoy_devices() {
        if let Entry::Vacant(e) = out.devices.entry(id) {
            e.insert(virtual_devices.acquire(id).map_err(|e| e.to_string())?);
        }
    }
    for id in engine.gamepads() {
        if let Entry::Vacant(e) = out.gamepads.entry(id) {
            let pad = virtual_devices
                .acquire_gamepad(id)
                .map_err(|e| format!("gamepad {id}: {e}"))?;
            e.insert(pad);
        }
    }
    Ok(())
}

/// Runs the profile's mode leave and stop events and gives their actions (a key press, a
/// macro, speech) up to [`STOP_BUDGET`] to finish.
fn finish_stop(engine: &mut Engine, out: &mut SystemOutput) {
    let start = Instant::now();
    if !engine.profile_stopping(start, out) {
        return;
    }
    let end = start + STOP_BUDGET;
    loop {
        out.flush();
        let now = Instant::now();
        if !engine.stop_pending() || now >= end {
            break;
        }
        let next = engine.next_deadline().unwrap_or(end).min(end);
        let wait = next.saturating_duration_since(now);
        std::thread::sleep(wait.clamp(Duration::from_millis(1), Duration::from_millis(10)));
        engine.tick(Instant::now(), out);
    }
}

/// Gives `engine` the inputs as they are: buttons and hats as its starting state, axes as
/// events (they write their outputs at once).
fn feed_initial(engine: &mut Engine, out: &mut SystemOutput, initial: &[DeviceState]) {
    let now = Instant::now();
    for d in initial.iter().filter(|d| !d.info.is_own()) {
        for (i, &b) in d.buttons.iter().enumerate() {
            let key = InputKey::new(&d.info.guid, InputKind::Button, i as u32 + 1);
            engine.seed(&key, Value::Button(b));
        }
        for (i, &h) in d.hats.iter().enumerate() {
            let key = InputKey::new(&d.info.guid, InputKind::Hat, i as u32 + 1);
            engine.seed(&key, Value::Hat(h));
        }
        for &axis in &d.info.axes {
            let key = InputKey::new(&d.info.guid, InputKind::Axis, u32::from(axis));
            engine.process(&key, Value::Axis(d.axes[usize::from(axis) - 1]), now, out);
        }
    }
}

/// Gives the voice service `engine`'s phrases and microphones, starting it for the first
/// profile with voice-input actions. Only for the engine that runs from now on: the service
/// listens for one profile.
fn set_up_voice(engine: &Engine, out: &mut SystemOutput) {
    if engine.uses_voice() {
        let phrases = engine.voice_phrases().to_vec();
        match (&out.voice, &out.voice_config) {
            (Some(voice), _) => voice.set_phrases(phrases),
            (None, Some(config)) => {
                out.voice = Some(gob_voice::VoiceService::start(
                    config.clone(),
                    phrases,
                    out.voice_events.clone(),
                ))
            }
            (None, None) => log::warn!("voice-input actions ignored: no speech model"),
        }
    }
    // Microphones kept open follow the profile's voice-input actions (none: all close).
    if let Some(voice) = &out.voice {
        voice.set_microphones(engine.voice_microphones());
    }
}

/// What the voice service reported, whichever profile ran: it outlives reloads.
#[derive(Default)]
struct ServiceState {
    ready: bool,
    error: Option<String>,
    /// Microphones listening now.
    listening: HashSet<String>,
    unknown_words: std::sync::Arc<[gob_voice::UnknownPhrase]>,
}

impl ServiceState {
    /// Takes in an event; the utterance it reports, if any.
    fn handle(&mut self, event: gob_voice::VoiceEvent) -> Option<VoiceHeard> {
        use gob_voice::VoiceEvent;
        match event {
            VoiceEvent::Ready => {
                self.ready = true;
                self.error = None;
            }
            VoiceEvent::Failed(e) => self.error = Some(e),
            VoiceEvent::Listening {
                microphone,
                on: true,
            } => {
                self.listening.insert(microphone);
            }
            VoiceEvent::Listening {
                microphone,
                on: false,
            } => {
                self.listening.remove(&microphone);
            }
            VoiceEvent::UnknownWords(list) => self.unknown_words = list,
            VoiceEvent::Heard {
                command,
                text,
                confidence,
            } => {
                return Some(VoiceHeard {
                    text,
                    confidence,
                    command,
                });
            }
        }
        None
    }

    /// `status` with the service's state.
    fn apply(&self, status: VoiceStatus) -> VoiceStatus {
        VoiceStatus {
            ready: self.ready,
            error: self.error.clone(),
            listening: !self.listening.is_empty(),
            unknown_words: self.unknown_words.clone(),
            ..status
        }
    }
}

/// What the engine thread shares with the [`Runner`].
struct Threads {
    events: Receiver<Timed>,
    voice: Receiver<gob_voice::VoiceEvent>,
    reloads: Receiver<Reload>,
    virtual_devices: Arc<dyn gob_virtual::Backend>,
    status: Arc<Mutex<RunnerStatus>>,
    logical: Arc<Mutex<LogicalValues>>,
    stop: Arc<AtomicBool>,
}

fn run(mut engine: Engine, mut out: SystemOutput, shared: &Threads, on_change: &dyn Fn()) {
    let Threads {
        events,
        voice,
        reloads,
        virtual_devices,
        status,
        logical,
        stop,
    } = shared;
    let mut window = LatencyWindow::new(Instant::now());
    // When this pass's events were read; kept between passes.
    let mut reads: Vec<Instant> = Vec::new();
    let mut ids = DeviceIds::default();
    // A fine timer is on: outputs move on their own.
    let mut fine = false;
    let mut service = ServiceState::default();
    let mut voice_status = status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .voice
        .clone();
    while !stop.load(Ordering::Relaxed) {
        for mut r in reloads.try_iter() {
            let result = acquire(&r.engine, &mut out, virtual_devices.as_ref());
            // Answered at once: the old profile's stop actions below must not hold up the
            // caller (the GUI).
            let _ = r.reply.send(result.clone());
            if result.is_ok() {
                // A held push-to-talk input would keep the microphone open otherwise, and a
                // command's button would stay down. The utterance it ends is heard with the old
                // phrases: they change only after it.
                engine.stop_voice(Instant::now(), &mut out);
                if r.fresh {
                    finish_stop(&mut engine, &mut out);
                }
                // Keys, buttons and latched outputs of the old profile go up: the devices stay.
                // Before the new profile writes, so what it presses stays down.
                engine.release_all(&mut out);
                feed_initial(&mut r.engine, &mut out, &r.initial);
                engine = r.engine;
                set_up_voice(&engine, &mut out);
                voice_status = initial_voice_status(&engine, &out).map(|v| service.apply(v));
                engine.activate(&mut out);
                if r.fresh {
                    engine.profile_started(Instant::now(), &mut out);
                }
                engine.take_logical_changed();
                *logical.lock().unwrap_or_else(|e| e.into_inner()) = engine.logical_values();
                on_change();
            }
        }
        // Reloads wrote to the devices above.
        out.flush();
        if engine.animating() != fine {
            fine = !fine;
            crate::platform::fine_timer(fine);
        }
        let wait = engine.next_deadline().map_or(IDLE, |d| {
            d.saturating_duration_since(Instant::now()).min(IDLE)
        });
        let first = match events.recv_timeout(wait) {
            Ok(e) => Some(e),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        reads.clear();
        for timed in first.into_iter().chain(events.try_iter()) {
            if let Some((key, value)) = convert(&mut ids, &timed.event) {
                engine.process(&key, value, Instant::now(), &mut out);
                reads.push(timed.read);
            }
        }
        let mut voice_changed = false;
        for event in voice.try_iter() {
            voice_changed |= on_voice(
                &mut engine,
                &mut out,
                &mut service,
                &mut voice_status,
                event,
            );
        }
        let now = Instant::now();
        engine.tick(now, &mut out);
        out.flush();
        // Latency ends when the outputs are written.
        for read in &reads {
            window.add(read.elapsed());
        }

        if engine.take_logical_changed() {
            *logical.lock().unwrap_or_else(|e| e.into_inner()) = engine.logical_values();
            on_change();
        }
        let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
        let load = out.load_profile.take();
        let latency = window.finish(now);
        let script_errors = engine.take_script_errors();
        if !script_errors.is_empty() {
            s.script_errors.extend(script_errors);
            on_change();
        }
        if s.mode != engine.current_mode()
            || s.paused != engine.is_paused()
            || load.is_some()
            || latency.is_some_and(|l| s.latency != Some(l))
            || voice_changed
            || s.voice != voice_status
        {
            s.mode = engine.current_mode().to_string();
            s.paused = engine.is_paused();
            s.voice = voice_status.clone();
            if load.is_some() {
                s.load_profile = load;
            }
            if latency.is_some() {
                s.latency = latency;
            }
            on_change();
        }
    }
    finish_stop(&mut engine, &mut out);
    // Held keys and mouse buttons go up; dropping `out` resets and releases the vJoy devices.
    engine.release_all(&mut out);
    out.flush();
    if fine {
        crate::platform::fine_timer(false);
    }
}

/// Hands a voice event to the engine, the service's state and the status; true when the status
/// changed.
fn on_voice(
    engine: &mut Engine,
    out: &mut SystemOutput,
    service: &mut ServiceState,
    status: &mut Option<VoiceStatus>,
    event: gob_voice::VoiceEvent,
) -> bool {
    let heard = service.handle(event);
    if let Some(id) = heard.as_ref().and_then(|h| h.command) {
        engine.voice_command(id, Instant::now(), out);
    }
    let Some(s) = status else {
        return false;
    };
    if heard.is_some() {
        s.heard = heard;
    }
    *s = service.apply(s.clone());
    true
}

/// Device ids of input events as the engine's keys take them, made once per device.
#[derive(Default)]
struct DeviceIds(HashMap<String, Arc<str>>);

impl DeviceIds {
    fn key(&mut self, device: &str, kind: InputKind, id: u32) -> InputKey {
        let device = match self.0.get(device) {
            Some(d) => d.clone(),
            None => {
                let d: Arc<str> = device.to_ascii_lowercase().into();
                self.0.insert(device.to_string(), d.clone());
                d
            }
        };
        InputKey { device, kind, id }
    }
}

fn convert(ids: &mut DeviceIds, event: &InputEvent) -> Option<(InputKey, Value)> {
    Some(match event {
        InputEvent::Axis { device, id, value } => (
            ids.key(device, InputKind::Axis, u32::from(*id)),
            Value::Axis(*value),
        ),
        InputEvent::Button {
            device,
            id,
            pressed,
        } => (
            ids.key(device, InputKind::Button, u32::from(*id)),
            Value::Button(*pressed),
        ),
        InputEvent::Hat {
            device,
            id,
            direction,
        } => (
            ids.key(device, InputKind::Hat, u32::from(*id)),
            Value::Hat(*direction),
        ),
        InputEvent::Key {
            scan_code,
            extended,
            pressed,
        } => {
            let key = gob_core::Key {
                scan_code: *scan_code,
                extended: *extended,
            };
            (
                ids.key(
                    gob_core::KEYBOARD_GUID,
                    InputKind::Button,
                    gob_core::key_input_id(key),
                ),
                Value::Button(*pressed),
            )
        }
        InputEvent::DevicesChanged => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use gob_virtual::Write;

    /// A virtual joystick that records each driver call.
    struct Calls(Arc<Mutex<Vec<Vec<Write>>>>);

    impl gob_virtual::Device for Calls {
        fn set_axis(&self, slot: u8, value: f32) -> bool {
            self.write(&[Write::Axis(slot, value)])
        }
        fn set_button(&self, n: u8, pressed: bool) -> bool {
            self.write(&[Write::Button(n, pressed)])
        }
        fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
            self.write(&[Write::Hat(n, direction)])
        }
        fn reset(&self) -> bool {
            true
        }
        fn write(&self, changes: &[Write]) -> bool {
            self.0.lock().unwrap().push(changes.to_vec());
            true
        }
    }

    fn output() -> (SystemOutput, Arc<Mutex<Vec<Vec<Write>>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut out = SystemOutput::new(
            crate::lighting::Lighting::new(HashMap::new(), None),
            None,
            std::sync::mpsc::channel().0,
        );
        out.devices.insert(1, Box::new(Calls(calls.clone())));
        (out, calls)
    }

    #[test]
    fn device_ids_are_made_once_per_device() {
        let mut ids = DeviceIds::default();
        let a = ids.key("AAAA-01", InputKind::Axis, 1);
        let b = ids.key("AAAA-01", InputKind::Button, 3);
        assert_eq!(&*a.device, "aaaa-01");
        assert!(Arc::ptr_eq(&a.device, &b.device));
        assert_eq!(a, InputKey::new("aaaa-01", InputKind::Axis, 1));
    }

    #[test]
    fn one_driver_call_per_device_and_pass() {
        let (mut out, calls) = output();
        out.axis(1, 1, 0.5);
        out.axis(1, 2, -0.5);
        out.button(1, 3, true);
        out.axis(1, 1, 0.25);
        out.flush();
        assert_eq!(
            *calls.lock().unwrap(),
            [vec![
                Write::Axis(1, 0.25),
                Write::Axis(2, -0.5),
                Write::Button(3, true)
            ]],
            "the newer axis value replaces the queued one"
        );
        out.flush();
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "nothing queued, nothing sent"
        );
    }

    #[test]
    fn unchanged_values_are_not_sent() {
        let (mut out, calls) = output();
        out.axis(1, 1, 0.5);
        out.flush();
        out.axis(1, 1, 0.5);
        out.button(1, 1, false);
        out.button(1, 1, false);
        out.flush();
        assert_eq!(
            *calls.lock().unwrap(),
            [vec![Write::Axis(1, 0.5)], vec![Write::Button(1, false)]]
        );
        assert_eq!(
            out.vjoy_state(1, InputKind::Axis, 1),
            Some(Value::Axis(0.5)),
            "conditions still read the value"
        );
    }

    #[test]
    fn a_press_and_release_in_one_pass_both_reach_the_driver() {
        let (mut out, calls) = output();
        out.button(1, 4, true);
        out.button(1, 4, false);
        out.hat(1, 1, Some(9000));
        out.flush();
        assert_eq!(
            *calls.lock().unwrap(),
            [
                vec![Write::Button(4, true)],
                vec![Write::Button(4, false), Write::Hat(1, Some(9000))]
            ]
        );
    }

    #[test]
    fn a_write_that_did_not_arrive_is_sent_again() {
        let (mut out, calls) = output();
        out.button(2, 1, true);
        out.flush();
        // Device 2 is held now: the same value goes out although it is unchanged.
        out.devices.insert(2, Box::new(Calls(calls.clone())));
        out.button(2, 1, true);
        out.flush();
        out.button(2, 1, true);
        out.flush();
        assert_eq!(
            *calls.lock().unwrap(),
            [vec![Write::Button(1, true)]],
            "sent once, then unchanged again"
        );
    }

    /// A virtual joystick without axis 7: a call with it is refused, the rest of it is taken.
    struct NoAxis7(Arc<Mutex<Vec<Vec<Write>>>>);

    impl gob_virtual::Device for NoAxis7 {
        fn set_axis(&self, slot: u8, value: f32) -> bool {
            self.write(&[Write::Axis(slot, value)])
        }
        fn set_button(&self, n: u8, pressed: bool) -> bool {
            self.write(&[Write::Button(n, pressed)])
        }
        fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
            self.write(&[Write::Hat(n, direction)])
        }
        fn reset(&self) -> bool {
            true
        }
        fn write(&self, changes: &[Write]) -> bool {
            self.0.lock().unwrap().push(changes.to_vec());
            !changes.iter().any(|w| matches!(w, Write::Axis(7, _)))
        }
    }

    #[test]
    fn one_refused_write_does_not_make_the_others_unsent() {
        let (mut out, calls) = output();
        out.devices.insert(1, Box::new(NoAxis7(calls.clone())));
        out.axis(1, 1, 0.5);
        out.axis(1, 7, 0.5);
        out.button(1, 2, true);
        out.flush();
        calls.lock().unwrap().clear();
        // The same values again: only the refused one is tried again.
        out.axis(1, 1, 0.5);
        out.axis(1, 7, 0.5);
        out.button(1, 2, true);
        out.flush();
        let calls = calls.lock().unwrap();
        assert!(
            calls
                .iter()
                .flatten()
                .all(|w| matches!(w, Write::Axis(7, _))),
            "only axis 7 goes out again: {calls:?}"
        );
        assert!(out.failing.contains(&write_key(1, Write::Axis(7, 0.5)).0));
        assert!(!out.failing.contains(&write_key(1, Write::Axis(1, 0.5)).0));
    }

    fn listening(microphone: &str, on: bool) -> gob_voice::VoiceEvent {
        gob_voice::VoiceEvent::Listening {
            microphone: microphone.into(),
            on,
        }
    }

    #[test]
    fn listening_while_any_microphone_listens() {
        let mut service = ServiceState::default();
        service.handle(listening("left", true));
        service.handle(listening("right", true));
        service.handle(listening("left", false));
        assert!(service.apply(VoiceStatus::default()).listening);
        service.handle(listening("right", false));
        assert!(!service.apply(VoiceStatus::default()).listening);
    }

    #[test]
    fn unknown_words_reach_the_status() {
        let mut service = ServiceState::default();
        let list: std::sync::Arc<[_]> = std::sync::Arc::new([gob_voice::UnknownPhrase {
            command: 1,
            phrase: "zz me".into(),
            words: vec!["zz".into()],
        }]);
        service.handle(gob_voice::VoiceEvent::UnknownWords(list.clone()));
        assert_eq!(service.apply(VoiceStatus::default()).unknown_words, list);
        service.handle(gob_voice::VoiceEvent::UnknownWords(std::sync::Arc::new([])));
        assert!(
            service
                .apply(VoiceStatus::default())
                .unknown_words
                .is_empty()
        );
    }

    #[test]
    fn the_service_state_outlives_profiles() {
        let mut service = ServiceState::default();
        service.handle(gob_voice::VoiceEvent::Failed("no library".into()));
        // A profile without voice in between does not lose it: the next one with voice shows it.
        let next = service.apply(VoiceStatus::default());
        assert_eq!(next.error.as_deref(), Some("no library"));
        assert!(!next.ready);
        service.handle(gob_voice::VoiceEvent::Ready);
        let next = service.apply(VoiceStatus::default());
        assert!(next.ready && next.error.is_none());
    }

    #[test]
    fn latency_window() {
        let t0 = Instant::now();
        let mut w = LatencyWindow::new(t0);
        w.add(Duration::from_micros(100));
        w.add(Duration::from_micros(300));
        assert_eq!(
            w.finish(t0 + Duration::from_millis(500)),
            None,
            "not over yet"
        );
        assert_eq!(
            w.finish(t0 + LATENCY_WINDOW),
            Some(Latency {
                average: Duration::from_micros(200),
                max: Duration::from_micros(300),
            })
        );
        assert_eq!(
            w.finish(t0 + 2 * LATENCY_WINDOW),
            None,
            "a window without events"
        );
    }
}
