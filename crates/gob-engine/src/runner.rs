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
    reply: Sender<Result<(), String>>,
}

/// How long [`Runner::reload`] waits for the engine thread.
const RELOAD_TIMEOUT: Duration = Duration::from_secs(5);

struct SystemOutput {
    devices: HashMap<u32, Box<dyn gob_virtual::Device>>,
    gamepads: HashMap<u32, Box<dyn gob_virtual::Gamepad>>,
    /// Last values written, read back by vJoy conditions.
    written: HashMap<(u32, InputKind, u32), Value>,
    /// Outputs whose last write the driver refused; logged once until one succeeds again.
    failing: HashSet<(u32, InputKind, u32)>,
    failing_pads: HashSet<(u32, Option<GamepadControl>)>,
    media: crate::media::Media,
    lighting: crate::lighting::Lighting,
    load_profile: Option<String>,
}

impl SystemOutput {
    /// Records a write; a refused one is logged when it starts failing and when it works again.
    fn wrote(&mut self, key: (u32, InputKind, u32), value: Value, ok: Option<bool>) {
        self.written.insert(key, value);
        let (device, kind, id) = key;
        log_refused(&mut self.failing, key, ok, || {
            format!("virtual device {device}: writing {kind:?} {id} = {value:?}")
        });
    }

    /// A gamepad write (`control` `None`: the d-pad as a whole), logged like [`Self::wrote`].
    fn wrote_pad(&mut self, gamepad: u32, control: Option<GamepadControl>, ok: Option<bool>) {
        log_refused(&mut self.failing_pads, (gamepad, control), ok, || {
            let control = control.map_or("dpad", GamepadControl::name);
            format!("virtual gamepad {gamepad}: writing {control}")
        });
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
        let ok = self.devices.get(&device).map(|d| d.set_axis(axis, value));
        let key = (device, InputKind::Axis, u32::from(axis));
        self.wrote(key, Value::Axis(value), ok);
    }

    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        let ok = self
            .devices
            .get(&device)
            .map(|d| d.set_button(button, pressed));
        let key = (device, InputKind::Button, u32::from(button));
        self.wrote(key, Value::Button(pressed), ok);
    }

    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        let ok = self.devices.get(&device).map(|d| d.set_hat(hat, direction));
        let key = (device, InputKind::Hat, u32::from(hat));
        self.wrote(key, Value::Hat(direction), ok);
    }

    fn gamepad_button(&mut self, gamepad: u32, control: GamepadControl, pressed: bool) {
        let ok = self
            .gamepads
            .get(&gamepad)
            .map(|p| p.set_button(control, pressed));
        self.wrote_pad(gamepad, Some(control), ok);
    }

    fn gamepad_axis(&mut self, gamepad: u32, control: GamepadControl, value: f32) {
        let ok = self
            .gamepads
            .get(&gamepad)
            .map(|p| p.set_axis(control, value));
        self.wrote_pad(gamepad, Some(control), ok);
    }

    fn gamepad_dpad(&mut self, gamepad: u32, direction: Option<u16>) {
        let ok = self.gamepads.get(&gamepad).map(|p| p.set_dpad(direction));
        self.wrote_pad(gamepad, None, ok);
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

    fn vjoy_state(&self, device: u32, kind: InputKind, id: u32) -> Option<Value> {
        self.written.get(&(device, kind, id)).copied()
    }

    fn key_pressed(&self, scan_code: u16, extended: bool) -> bool {
        crate::platform::injector().key_pressed(scan_code, extended)
    }

    fn mouse_move(&mut self, dx: i32, dy: i32) {
        crate::platform::injector().mouse_move(dx, dy);
    }

    fn mouse_button(&mut self, button: gob_core::MouseButton, pressed: bool) {
        crate::platform::injector().mouse_button(button, pressed);
    }

    fn mouse_wheel(&mut self, notches: i32) {
        crate::platform::injector().mouse_wheel(notches);
    }

    fn key(&mut self, scan_code: u16, extended: bool, pressed: bool) {
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
    /// `None`: LED control is off and light-led actions do nothing.
    pub fn start(
        profile: &Profile,
        aliases: &HashMap<String, String>,
        events: Receiver<Timed>,
        initial: &[DeviceState],
        led_defaults: Option<&[crate::Light]>,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let mut engine = Engine::new(profile, aliases);
        let virtual_devices = gob_virtual::load().map_err(|e| e.to_string())?;
        let mut out = SystemOutput {
            devices: HashMap::new(),
            gamepads: HashMap::new(),
            written: HashMap::new(),
            failing: HashSet::new(),
            failing_pads: HashSet::new(),
            media: Default::default(),
            lighting: crate::lighting::Lighting::new(
                initial
                    .iter()
                    .filter(|d| !d.info.hid_path.is_empty())
                    .map(|d| (d.info.guid.to_ascii_lowercase(), d.info.hid_path.clone()))
                    .collect(),
                led_defaults.map(<[crate::Light]>::to_vec),
            ),
            load_profile: None,
        };
        prepare(&mut engine, &mut out, virtual_devices.as_ref(), initial)?;
        if led_defaults.is_none() && engine.uses_lights() {
            log::info!("light-led actions ignored: experimental features are off");
        }
        crate::platform::Injector::prepare(crate::platform::injector());
        engine.activate(&mut out);

        let status = Arc::new(Mutex::new(RunnerStatus {
            mode: engine.current_mode().to_string(),
            paused: false,
            load_profile: None,
            latency: None,
        }));
        engine.take_logical_changed();
        let logical = Arc::new(Mutex::new(engine.logical_values()));
        let stop = Arc::new(AtomicBool::new(false));
        let (reload, reloads) = std::sync::mpsc::channel();
        let thread = {
            let (status, logical, stop) = (status.clone(), logical.clone(), stop.clone());
            let threads = Threads {
                events,
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
    ) -> Result<(), String> {
        let (reply, answer) = std::sync::mpsc::channel();
        let request = Reload {
            engine: Engine::new(profile, aliases),
            initial: initial.to_vec(),
            reply,
        };
        self.reload
            .send(request)
            .map_err(|_| "the engine has stopped".to_string())?;
        answer
            .recv_timeout(RELOAD_TIMEOUT)
            .map_err(|_| "the engine did not answer".to_string())?
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
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Acquires the virtual devices `engine` writes to that `out` does not hold yet, then seeds
/// the engine with `initial` and sends the axis positions through it.
fn prepare(
    engine: &mut Engine,
    out: &mut SystemOutput,
    virtual_devices: &dyn gob_virtual::Backend,
    initial: &[DeviceState],
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
    Ok(())
}

/// What the engine thread shares with the [`Runner`].
struct Threads {
    events: Receiver<Timed>,
    reloads: Receiver<Reload>,
    virtual_devices: Arc<dyn gob_virtual::Backend>,
    status: Arc<Mutex<RunnerStatus>>,
    logical: Arc<Mutex<LogicalValues>>,
    stop: Arc<AtomicBool>,
}

fn run(mut engine: Engine, mut out: SystemOutput, shared: &Threads, on_change: &dyn Fn()) {
    let Threads {
        events,
        reloads,
        virtual_devices,
        status,
        logical,
        stop,
    } = shared;
    let mut window = LatencyWindow::new(Instant::now());
    while !stop.load(Ordering::Relaxed) {
        for mut r in reloads.try_iter() {
            let result = prepare(
                &mut r.engine,
                &mut out,
                virtual_devices.as_ref(),
                &r.initial,
            );
            if result.is_ok() {
                engine = r.engine;
                engine.activate(&mut out);
                engine.take_logical_changed();
                *logical.lock().unwrap_or_else(|e| e.into_inner()) = engine.logical_values();
                on_change();
            }
            let _ = r.reply.send(result);
        }
        let wait = engine.next_deadline().map_or(IDLE, |d| {
            d.saturating_duration_since(Instant::now()).min(IDLE)
        });
        let first = match events.recv_timeout(wait) {
            Ok(e) => Some(e),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        for timed in first.into_iter().chain(events.try_iter()) {
            if let Some((key, value)) = convert(&timed.event) {
                engine.process(&key, value, Instant::now(), &mut out);
                window.add(timed.read.elapsed());
            }
        }
        let now = Instant::now();
        engine.tick(now, &mut out);

        if engine.take_logical_changed() {
            *logical.lock().unwrap_or_else(|e| e.into_inner()) = engine.logical_values();
            on_change();
        }
        let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
        let load = out.load_profile.take();
        let latency = window.finish(now);
        if s.mode != engine.current_mode()
            || s.paused != engine.is_paused()
            || load.is_some()
            || latency.is_some_and(|l| s.latency != Some(l))
        {
            s.mode = engine.current_mode().to_string();
            s.paused = engine.is_paused();
            if load.is_some() {
                s.load_profile = load;
            }
            if latency.is_some() {
                s.latency = latency;
            }
            on_change();
        }
    }
    // Dropping `out` resets and releases the vJoy devices.
}

fn convert(event: &InputEvent) -> Option<(InputKey, Value)> {
    Some(match event {
        InputEvent::Axis { device, id, value } => (
            InputKey::new(device, InputKind::Axis, u32::from(*id)),
            Value::Axis(*value),
        ),
        InputEvent::Button {
            device,
            id,
            pressed,
        } => (
            InputKey::new(device, InputKind::Button, u32::from(*id)),
            Value::Button(*pressed),
        ),
        InputEvent::Hat {
            device,
            id,
            direction,
        } => (
            InputKey::new(device, InputKind::Hat, u32::from(*id)),
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
                InputKey::new(
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
