//! Runs the engine on a background thread against real devices.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gob_core::Profile;
use gob_input::{DeviceState, InputEvent};
use gob_vjoy::VJoy;

use crate::{Engine, InputKey, InputKind, Output, Value};

/// Upper bound on how long the loop sleeps without events.
const IDLE: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, Default)]
pub struct RunnerStatus {
    pub mode: String,
    pub paused: bool,
    /// Profile file a load-profile action asked for; the GUI takes it.
    pub load_profile: Option<String>,
}

pub struct Runner {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    status: Arc<Mutex<RunnerStatus>>,
}

struct SystemOutput {
    devices: HashMap<u32, gob_vjoy::Device>,
    /// Last values written, read back by vJoy conditions.
    written: HashMap<(u32, InputKind, u32), Value>,
    media: crate::media::Media,
    lighting: crate::lighting::Lighting,
    load_profile: Option<String>,
}

impl Output for SystemOutput {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        if let Some(d) = self.devices.get(&device) {
            d.set_axis(axis, value);
        }
        let key = (device, InputKind::Axis, u32::from(axis));
        self.written.insert(key, Value::Axis(value));
    }

    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        if let Some(d) = self.devices.get(&device) {
            d.set_button(button, pressed);
        }
        let key = (device, InputKind::Button, u32::from(button));
        self.written.insert(key, Value::Button(pressed));
    }

    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        if let Some(d) = self.devices.get(&device) {
            d.set_hat(hat, direction);
        }
        let key = (device, InputKind::Hat, u32::from(hat));
        self.written.insert(key, Value::Hat(direction));
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
        crate::sendinput::key_pressed(scan_code, extended)
    }

    fn mouse_move(&mut self, dx: i32, dy: i32) {
        crate::sendinput::mouse_move(dx, dy);
    }

    fn mouse_button(&mut self, button: gob_core::MouseButton, pressed: bool) {
        crate::sendinput::mouse_button(button, pressed);
    }

    fn mouse_wheel(&mut self, notches: i32) {
        crate::sendinput::mouse_wheel(notches);
    }

    fn key(&mut self, scan_code: u16, extended: bool, pressed: bool) {
        crate::sendinput::key(scan_code, extended, pressed);
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
    pub fn start(
        profile: &Profile,
        aliases: &HashMap<String, String>,
        events: Receiver<InputEvent>,
        initial: &[DeviceState],
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let mut engine = Engine::new(profile, aliases);
        let vjoy = VJoy::load().map_err(|e| e.to_string())?;
        let mut out = SystemOutput {
            devices: HashMap::new(),
            written: HashMap::new(),
            media: Default::default(),
            lighting: crate::lighting::Lighting::new(
                initial
                    .iter()
                    .filter(|d| !d.info.hid_path.is_empty())
                    .map(|d| (d.info.guid.to_ascii_lowercase(), d.info.hid_path.clone()))
                    .collect(),
            ),
            load_profile: None,
        };
        for id in engine.vjoy_devices() {
            out.devices
                .insert(id, vjoy.acquire(id).map_err(|e| e.to_string())?);
        }

        let now = Instant::now();
        for d in initial.iter().filter(|d| !d.info.is_vjoy()) {
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
                engine.process(
                    &key,
                    Value::Axis(d.axes[usize::from(axis) - 1]),
                    now,
                    &mut out,
                );
            }
        }

        engine.activate(&mut out);

        let status = Arc::new(Mutex::new(RunnerStatus {
            mode: engine.current_mode().to_string(),
            paused: false,
            load_profile: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (status, stop) = (status.clone(), stop.clone());
            std::thread::Builder::new()
                .name("gob-engine".into())
                .spawn(move || run(engine, out, &events, &status, &stop, &on_change))
                .map_err(|e| e.to_string())?
        };
        Ok(Self {
            stop,
            thread: Some(thread),
            status,
        })
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
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    mut engine: Engine,
    mut out: SystemOutput,
    events: &Receiver<InputEvent>,
    status: &Mutex<RunnerStatus>,
    stop: &AtomicBool,
    on_change: &dyn Fn(),
) {
    while !stop.load(Ordering::Relaxed) {
        let wait = engine.next_deadline().map_or(IDLE, |d| {
            d.saturating_duration_since(Instant::now()).min(IDLE)
        });
        let first = match events.recv_timeout(wait) {
            Ok(e) => Some(e),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        for event in first.into_iter().chain(events.try_iter()) {
            if let Some((key, value)) = convert(&event) {
                engine.process(&key, value, Instant::now(), &mut out);
            }
        }
        engine.tick(Instant::now(), &mut out);

        let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
        let load = out.load_profile.take();
        if s.mode != engine.current_mode() || s.paused != engine.is_paused() || load.is_some() {
            s.mode = engine.current_mode().to_string();
            s.paused = engine.is_paused();
            if load.is_some() {
                s.load_profile = load;
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
