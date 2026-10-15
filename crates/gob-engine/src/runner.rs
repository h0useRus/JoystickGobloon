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
}

pub struct Runner {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    status: Arc<Mutex<RunnerStatus>>,
}

struct SystemOutput {
    devices: HashMap<u32, gob_vjoy::Device>,
}

impl Output for SystemOutput {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        if let Some(d) = self.devices.get(&device) {
            d.set_axis(axis, value);
        }
    }

    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        if let Some(d) = self.devices.get(&device) {
            d.set_button(button, pressed);
        }
    }

    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        if let Some(d) = self.devices.get(&device) {
            d.set_hat(hat, direction);
        }
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

    fn run_command(&mut self, executable: &str, arguments: &[String]) {
        // Detached: the program keeps running after JoyGob stops.
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
        };
        for id in engine.vjoy_devices() {
            out.devices
                .insert(id, vjoy.acquire(id).map_err(|e| e.to_string())?);
        }

        let now = Instant::now();
        for d in initial.iter().filter(|d| !d.info.is_vjoy()) {
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

        let status = Arc::new(Mutex::new(RunnerStatus {
            mode: engine.current_mode().to_string(),
            paused: false,
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
        if s.mode != engine.current_mode() || s.paused != engine.is_paused() {
            s.mode = engine.current_mode().to_string();
            s.paused = engine.is_paused();
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
        InputEvent::DevicesChanged => return None,
    })
}
