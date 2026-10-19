//! Background polling thread with a shared snapshot and an event channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Calibration, DeviceState, InputEvent};

/// Calibration per (device GUID, axis slot); axes without one use DirectInput's range.
pub type Calibrations = std::collections::HashMap<(String, u8), Calibration>;

const POLL_INTERVAL: Duration = Duration::from_millis(4);
/// Re-enumerate devices every this many polls (~2 s) to catch hot-plugging.
const RESCAN_EVERY: u32 = 500;
const EVENT_BUFFER: usize = 4096;

/// Shared, read-only side of an [`InputService`]: see [`InputService::handle`].
#[derive(Clone)]
pub struct InputHandle {
    snapshot: Arc<RwLock<Vec<DeviceState>>>,
    broadcast: Broadcast,
}

impl InputHandle {
    /// Current state of every attached device.
    pub fn devices(&self) -> Vec<DeviceState> {
        self.snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Like [`InputService::subscribe`].
    pub fn subscribe(&self) -> Receiver<InputEvent> {
        self.broadcast.subscribe()
    }
}

/// Fan-out of input events to every subscriber; dead subscribers are dropped.
#[derive(Clone, Default)]
struct Broadcast(Arc<Mutex<Vec<SyncSender<InputEvent>>>>);

impl Broadcast {
    fn subscribe(&self) -> Receiver<InputEvent> {
        let (tx, rx) = mpsc::sync_channel(EVENT_BUFFER);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        rx
    }

    fn send(&self, event: InputEvent) {
        let mut subs = self.0.lock().unwrap_or_else(|e| e.into_inner());
        subs.retain(|tx| {
            !matches!(
                tx.try_send(event.clone()),
                Err(TrySendError::Disconnected(_))
            )
        });
    }
}

pub struct InputService {
    snapshot: Arc<RwLock<Vec<DeviceState>>>,
    broadcast: Broadcast,
    events: Receiver<InputEvent>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    calibrations: Arc<RwLock<Calibrations>>,
    /// Keyboard keys as events (low-level hook); `None` if it could not be installed.
    _keyboard: Option<crate::keyboard::Hook>,
}

impl InputService {
    /// Starts polling in a background thread. `on_change` is called after every
    /// change (e.g. to request a GUI repaint).
    pub fn start(on_change: impl Fn() + Send + Sync + 'static) -> Self {
        let on_change = Arc::new(on_change);
        let calibrations: Arc<RwLock<Calibrations>> = Arc::default();
        let snapshot = Arc::new(RwLock::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let broadcast = Broadcast::default();
        let events = broadcast.subscribe();
        let thread = {
            let (snapshot, stop, tx) = (snapshot.clone(), stop.clone(), broadcast.clone());
            let (on_change, cal) = (on_change.clone(), calibrations.clone());
            std::thread::Builder::new()
                .name("gob-input".into())
                .spawn(move || run(&snapshot, &stop, &tx, &cal, &*on_change))
                .ok()
        };
        let keyboard = {
            let tx = broadcast.clone();
            crate::keyboard::start(Arc::new(move |scan_code, extended, pressed| {
                tx.send(InputEvent::Key {
                    scan_code,
                    extended,
                    pressed,
                });
                on_change();
            }))
        };
        Self {
            snapshot,
            broadcast,
            events,
            stop,
            thread,
            calibrations,
            _keyboard: keyboard,
        }
    }

    /// Sets (or with `None` removes) the calibration of one axis; applies from the next poll.
    pub fn set_calibration(&self, device: &str, slot: u8, calibration: Option<Calibration>) {
        let mut map = self.calibrations.write().unwrap_or_else(|e| e.into_inner());
        let key = (device.to_ascii_lowercase(), slot);
        match calibration {
            Some(c) => map.insert(key, c),
            None => map.remove(&key),
        };
    }

    /// Current state of every attached device.
    pub fn devices(&self) -> RwLockReadGuard<'_, Vec<DeviceState>> {
        self.snapshot.read().unwrap_or_else(|e| e.into_inner())
    }

    /// A separate event stream, e.g. for the mapping engine. Drop the receiver to unsubscribe.
    pub fn subscribe(&self) -> Receiver<InputEvent> {
        self.broadcast.subscribe()
    }

    /// The device states and the event stream for other owners (e.g. a tray menu that starts
    /// the engine while the window is hidden). Polling stops with the service, not the handle.
    pub fn handle(&self) -> InputHandle {
        InputHandle {
            snapshot: self.snapshot.clone(),
            broadcast: self.broadcast.clone(),
        }
    }

    /// Changes since the last call. Events are dropped when nobody drains them.
    pub fn events(&self) -> impl Iterator<Item = InputEvent> + '_ {
        self.events.try_iter()
    }
}

impl Drop for InputService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(not(windows))]
fn run(
    _: &RwLock<Vec<DeviceState>>,
    _: &AtomicBool,
    _: &Broadcast,
    _: &RwLock<Calibrations>,
    _: &dyn Fn(),
) {
}

#[cfg(windows)]
fn run(
    snapshot: &RwLock<Vec<DeviceState>>,
    stop: &AtomicBool,
    tx: &Broadcast,
    calibrations: &RwLock<Calibrations>,
    on_change: &dyn Fn(),
) {
    use crate::dinput::{Device, DirectInput};

    let Ok(di) = DirectInput::new() else { return };
    let mut devices: Vec<Device> = Vec::new();
    let mut tick = 0u32;
    while !stop.load(Ordering::Relaxed) {
        let mut changed = false;
        if tick.is_multiple_of(RESCAN_EVERY) {
            let attached = di.enumerate();
            let before = devices.len();
            devices.retain(|d| attached.iter().any(|a| guid_of(a) == d.state.info.guid));
            for inst in &attached {
                if !devices.iter().any(|d| d.state.info.guid == guid_of(inst))
                    && let Ok(dev) = di.open(inst)
                {
                    devices.push(dev);
                }
            }
            if devices.len() != before || tick == 0 {
                tx.send(InputEvent::DevicesChanged);
                changed = true;
            }
        }
        tick = tick.wrapping_add(1);

        let mut lost = false;
        let cal = calibrations
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        for dev in &mut devices {
            let previous = dev.state.clone();
            if !dev.poll(&cal) {
                lost = true;
                continue;
            }
            changed |= emit_changes(&previous, &dev.state, tx);
            // Raw values matter to the calibration page even when the scaled value stays.
            changed |= previous.raw != dev.state.raw;
        }
        if lost {
            tick = 0; // rescan right away
        }
        if changed {
            *snapshot.write().unwrap_or_else(|e| e.into_inner()) =
                devices.iter().map(|d| d.state.clone()).collect();
            on_change();
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(windows)]
fn guid_of(inst: &windows::Win32::Devices::HumanInterfaceDevice::DIDEVICEINSTANCEW) -> String {
    let g = inst.guidInstance;
    crate::format_guid(g.data1, g.data2, g.data3, g.data4)
}

/// Sends one event per changed input; returns whether anything changed.
fn emit_changes(old: &DeviceState, new: &DeviceState, tx: &Broadcast) -> bool {
    let device = || new.info.guid.clone();
    let mut changed = false;
    for &id in &new.info.axes {
        let i = usize::from(id) - 1;
        // Ignore sensor noise below ~0.05%.
        if (old.axes[i] - new.axes[i]).abs() > 0.0005 {
            changed = true;
            tx.send(InputEvent::Axis {
                device: device(),
                id,
                value: new.axes[i],
            });
        }
    }
    for (i, (&a, &b)) in old.buttons.iter().zip(&new.buttons).enumerate() {
        if a != b {
            changed = true;
            tx.send(InputEvent::Button {
                device: device(),
                id: i as u16 + 1,
                pressed: b,
            });
        }
    }
    for (i, (&a, &b)) in old.hats.iter().zip(&new.hats).enumerate() {
        if a != b {
            changed = true;
            tx.send(InputEvent::Hat {
                device: device(),
                id: i as u8 + 1,
                direction: b,
            });
        }
    }
    changed
}
