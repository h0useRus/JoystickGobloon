//! Background polling thread with a shared snapshot and an event channel.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};
use std::thread::JoinHandle;
#[cfg(windows)]
use std::time::{Duration, Instant};

use crate::{Calibration, DeviceState, InputEvent};

/// Calibration per (device GUID, axis slot); axes without one use DirectInput's range.
pub type Calibrations = std::collections::HashMap<(String, u8), Calibration>;

/// Polling mode: time between two reads of every device.
#[cfg(windows)]
const POLL_INTERVAL: Duration = Duration::from_millis(4);
/// Event mode: devices are also read this often without a change signal, as a fallback.
#[cfg(windows)]
const EVENT_TIMEOUT: Duration = Duration::from_millis(20);
/// Re-enumerate devices this often to catch hot-plugging.
#[cfg(windows)]
const RESCAN: Duration = Duration::from_secs(2);
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
    /// Bumped on every calibration change, so the polling thread copies the map only then.
    calibration_version: Arc<AtomicU64>,
    /// See [`InputService::set_event_mode`].
    event_mode: Arc<AtomicBool>,
    /// Keyboard keys as events (low-level hook); `None` if it could not be installed.
    _keyboard: Option<crate::keyboard::Hook>,
}

impl InputService {
    /// Starts polling in a background thread. `on_change` is called after every
    /// change (e.g. to request a GUI repaint).
    pub fn start(on_change: impl Fn() + Send + Sync + 'static) -> Self {
        let on_change = Arc::new(on_change);
        let calibrations: Arc<RwLock<Calibrations>> = Arc::default();
        let calibration_version = Arc::new(AtomicU64::new(0));
        let event_mode = Arc::new(AtomicBool::new(false));
        let snapshot = Arc::new(RwLock::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let broadcast = Broadcast::default();
        let events = broadcast.subscribe();
        let thread = {
            let (snapshot, stop, tx) = (snapshot.clone(), stop.clone(), broadcast.clone());
            let (on_change, cal) = (on_change.clone(), calibrations.clone());
            let (version, mode) = (calibration_version.clone(), event_mode.clone());
            std::thread::Builder::new()
                .name("gob-input".into())
                .spawn(move || {
                    let shared = Shared {
                        snapshot: &snapshot,
                        stop: &stop,
                        tx: &tx,
                        calibrations: &cal,
                        calibration_version: &version,
                        event_mode: &mode,
                    };
                    run(&shared, &*on_change)
                })
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
            calibration_version,
            event_mode,
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
        self.calibration_version.fetch_add(1, Ordering::Release);
    }

    /// Event mode (`true`): devices are read when DirectInput signals a change, and button and
    /// hat changes come from its buffer, so short presses between two reads are kept.
    /// Polling mode (`false`, the default): every device is read every 4 ms. Switching
    /// reopens the devices.
    pub fn set_event_mode(&self, on: bool) {
        self.event_mode.store(on, Ordering::Relaxed);
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

/// What the polling thread shares with [`InputService`].
// Linux has no polling thread yet (docs/LINUX_PLAN.md, phase 1).
#[cfg_attr(not(windows), allow(dead_code))]
struct Shared<'a> {
    snapshot: &'a RwLock<Vec<DeviceState>>,
    stop: &'a AtomicBool,
    tx: &'a Broadcast,
    calibrations: &'a RwLock<Calibrations>,
    calibration_version: &'a AtomicU64,
    event_mode: &'a AtomicBool,
}

#[cfg(not(windows))]
fn run(_: &Shared, _: &dyn Fn()) {}

#[cfg(windows)]
fn run(shared: &Shared, on_change: &dyn Fn()) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Threading::WaitForMultipleObjects;

    use crate::dinput::{Device, DirectInput};

    /// WaitForMultipleObjects takes at most this many handles.
    const MAX_WAIT: usize = 64;

    let tx = shared.tx;
    let Ok(di) = DirectInput::new() else { return };
    let mut devices: Vec<Device> = Vec::new();
    let mut next_scan = Instant::now();
    // The first scan, and the one after a device was lost, always reports the device list.
    let mut announce = true;
    let mut events = shared.event_mode.load(Ordering::Relaxed);
    let mut cal = Calibrations::new();
    let mut cal_version = None;
    let mut handles: Vec<HANDLE> = Vec::new();
    while !shared.stop.load(Ordering::Relaxed) {
        let mut changed = false;
        let wanted = shared.event_mode.load(Ordering::Relaxed);
        if wanted != events {
            // Buffer and event can only be set up while opening: reopen in the new mode.
            events = wanted;
            devices.clear();
            next_scan = Instant::now();
        }
        if Instant::now() >= next_scan {
            next_scan = Instant::now() + RESCAN;
            let attached = di.enumerate();
            let before = devices.len();
            devices.retain(|d| attached.iter().any(|a| guid_of(a) == d.state.info.guid));
            for inst in &attached {
                if !devices.iter().any(|d| d.state.info.guid == guid_of(inst))
                    && let Ok(mut dev) = di.open(inst, events)
                {
                    dev.set_calibrations(&cal);
                    devices.push(dev);
                }
            }
            if devices.len() != before || announce {
                announce = false;
                tx.send(InputEvent::DevicesChanged);
                changed = true;
            }
        }

        let version = shared.calibration_version.load(Ordering::Acquire);
        if cal_version != Some(version) {
            cal_version = Some(version);
            cal.clone_from(
                &shared
                    .calibrations
                    .read()
                    .unwrap_or_else(|e| e.into_inner()),
            );
            for dev in &mut devices {
                dev.set_calibrations(&cal);
            }
        }

        let mut lost = false;
        for dev in &mut devices {
            let read = if dev.event().is_some() {
                dev.read_buffered(&mut |event| {
                    tx.send(event);
                    changed = true;
                })
            } else {
                dev.poll()
            };
            if !read {
                lost = true;
                continue;
            }
            changed |= emit_changes(&dev.previous, &dev.state, tx);
            // Raw values matter to the calibration page even when the scaled value stays.
            changed |= dev.previous.raw != dev.state.raw;
        }
        if lost {
            next_scan = Instant::now(); // rescan right away
            announce = true;
        }
        if changed {
            let mut snapshot = shared.snapshot.write().unwrap_or_else(|e| e.into_inner());
            let same_devices = snapshot.len() == devices.len()
                && snapshot
                    .iter()
                    .zip(&devices)
                    .all(|(s, d)| s.info.guid == d.state.info.guid);
            if same_devices {
                // Usual case: only values changed, update them in place without allocating.
                for (s, d) in snapshot.iter_mut().zip(&devices) {
                    s.copy_readings(&d.state);
                }
            } else {
                *snapshot = devices.iter().map(|d| d.state.clone()).collect();
            }
            drop(snapshot);
            on_change();
        }

        handles.clear();
        handles.extend(devices.iter().filter_map(Device::event).take(MAX_WAIT));
        // A device without an event (polling mode, or it refused one) needs the short interval.
        let all_signal = !devices.is_empty() && handles.len() == devices.len();
        if events && all_signal {
            // SAFETY: the handles belong to `devices`, which stay open during the wait.
            unsafe {
                WaitForMultipleObjects(&handles, false, EVENT_TIMEOUT.as_millis() as u32);
            }
        } else if events && devices.is_empty() {
            std::thread::sleep(EVENT_TIMEOUT);
        } else {
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

#[cfg(windows)]
fn guid_of(inst: &windows::Win32::Devices::HumanInterfaceDevice::DIDEVICEINSTANCEW) -> String {
    let g = inst.guidInstance;
    crate::format_guid(g.data1, g.data2, g.data3, g.data4)
}

/// Sends one event per changed input; returns whether anything changed.
#[cfg(windows)]
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
