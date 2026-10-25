//! Background polling thread with a shared snapshot and an event channel.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::{Calibration, DeviceState, InputEvent, Timed};

/// Calibration per (device GUID, axis slot); axes without one use DirectInput's range.
pub type Calibrations = std::collections::HashMap<(String, u8), Calibration>;

/// Re-enumerate devices this often to catch hot-plugging.
const RESCAN: Duration = Duration::from_secs(2);
const EVENT_BUFFER: usize = 4096;
/// Range and default of [`InputService::set_poll_interval`].
pub const MIN_POLL_INTERVAL: Duration = Duration::from_millis(1);
pub const MAX_POLL_INTERVAL: Duration = Duration::from_millis(5);
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(1);

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

    /// Like [`InputService::subscribe_timed`].
    pub fn subscribe_timed(&self) -> Receiver<Timed> {
        self.broadcast.subscribe_timed()
    }
}

enum Subscriber {
    /// The GUI: its queue fills while the window is hidden, which is expected.
    Plain(SyncSender<InputEvent>),
    /// The mapping engine, with the number of events lost since its queue filled up.
    Timed(SyncSender<Timed>, u64),
}

/// Fan-out of input events to every subscriber; dead subscribers are dropped.
#[derive(Clone, Default)]
struct Broadcast(Arc<Mutex<Vec<Subscriber>>>);

impl Broadcast {
    fn subscribe(&self) -> Receiver<InputEvent> {
        let (tx, rx) = mpsc::sync_channel(EVENT_BUFFER);
        self.add(Subscriber::Plain(tx));
        rx
    }

    fn subscribe_timed(&self) -> Receiver<Timed> {
        let (tx, rx) = mpsc::sync_channel(EVENT_BUFFER);
        self.add(Subscriber::Timed(tx, 0));
        rx
    }

    fn add(&self, subscriber: Subscriber) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(subscriber);
    }

    /// `read`: when the device was read. A full queue drops the event: the input thread
    /// never waits for a subscriber. Losses of the engine's events are logged.
    fn send(&self, event: InputEvent, read: Instant) {
        let mut subs = self.0.lock().unwrap_or_else(|e| e.into_inner());
        subs.retain_mut(|sub| match sub {
            Subscriber::Plain(tx) => !matches!(
                tx.try_send(event.clone()),
                Err(TrySendError::Disconnected(_))
            ),
            Subscriber::Timed(tx, lost) => match tx.try_send(Timed {
                event: event.clone(),
                read,
            }) {
                Ok(()) => {
                    if *lost > 0 {
                        log::warn!("input: {lost} event(s) lost while the engine fell behind");
                        *lost = 0;
                    }
                    true
                }
                Err(TrySendError::Full(_)) => {
                    if *lost == 0 {
                        log::warn!("input: the engine's event queue is full, events are lost");
                    }
                    *lost += 1;
                    true
                }
                Err(TrySendError::Disconnected(_)) => false,
            },
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
    /// See [`InputService::set_poll_interval`], in microseconds.
    poll_interval: Arc<AtomicU64>,
    /// See [`InputService::polls_devices`].
    polling: Arc<AtomicBool>,
    /// Keyboard keys as events (low-level hook); `None` if it could not be installed.
    _keyboard: Option<crate::platform::keyboard::Hook>,
}

impl InputService {
    /// Starts polling in a background thread. `on_change` is called after every
    /// change (e.g. to request a GUI repaint).
    pub fn start(on_change: impl Fn() + Send + Sync + 'static) -> Self {
        let on_change = Arc::new(on_change);
        let calibrations: Arc<RwLock<Calibrations>> = Arc::default();
        let calibration_version = Arc::new(AtomicU64::new(0));
        let event_mode = Arc::new(AtomicBool::new(false));
        let poll_interval = Arc::new(AtomicU64::new(DEFAULT_POLL_INTERVAL.as_micros() as u64));
        let polling = Arc::new(AtomicBool::new(true));
        let snapshot = Arc::new(RwLock::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let broadcast = Broadcast::default();
        let events = broadcast.subscribe();
        let thread = {
            let (snapshot, stop, tx) = (snapshot.clone(), stop.clone(), broadcast.clone());
            let (on_change, cal) = (on_change.clone(), calibrations.clone());
            let (version, mode) = (calibration_version.clone(), event_mode.clone());
            let (poll, polling) = (poll_interval.clone(), polling.clone());
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
                        poll_interval: &poll,
                        polling: &polling,
                    };
                    run(&shared, &*on_change)
                })
                .ok()
        };
        let keyboard = {
            let tx = broadcast.clone();
            crate::platform::keyboard::start(Arc::new(move |scan_code, extended, pressed| {
                let event = InputEvent::Key {
                    scan_code,
                    extended,
                    pressed,
                };
                tx.send(event, Instant::now());
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
            poll_interval,
            polling,
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
    /// Polling mode (`false`, the default): every device is read every poll interval.
    /// Switching reopens the devices.
    pub fn set_event_mode(&self, on: bool) {
        self.event_mode.store(on, Ordering::Relaxed);
    }

    /// Time between two reads of a polled device: every device in polling mode, the ones
    /// that cannot signal a change in event mode. Clamped to
    /// [`MIN_POLL_INTERVAL`]..=[`MAX_POLL_INTERVAL`].
    pub fn set_poll_interval(&self, interval: Duration) {
        let interval = interval.clamp(MIN_POLL_INTERVAL, MAX_POLL_INTERVAL);
        self.poll_interval
            .store(interval.as_micros() as u64, Ordering::Relaxed);
    }

    /// Whether the poll interval matters now: polling mode, or in event mode a device that
    /// cannot signal its changes.
    pub fn polls_devices(&self) -> bool {
        self.polling.load(Ordering::Relaxed)
    }

    /// Current state of every attached device.
    pub fn devices(&self) -> Vec<DeviceState> {
        self.snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// A separate event stream, e.g. for the mapping engine. Drop the receiver to unsubscribe.
    pub fn subscribe(&self) -> Receiver<InputEvent> {
        self.broadcast.subscribe()
    }

    /// Like [`Self::subscribe`], each event with the time its device was read.
    pub fn subscribe_timed(&self) -> Receiver<Timed> {
        self.broadcast.subscribe_timed()
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
struct Shared<'a> {
    snapshot: &'a RwLock<Vec<DeviceState>>,
    stop: &'a AtomicBool,
    tx: &'a Broadcast,
    calibrations: &'a RwLock<Calibrations>,
    calibration_version: &'a AtomicU64,
    event_mode: &'a AtomicBool,
    poll_interval: &'a AtomicU64,
    polling: &'a AtomicBool,
}

/// The input thread: finds, opens and reads controllers through this platform's backend.
fn run(shared: &Shared, on_change: &dyn Fn()) {
    use crate::platform::{Backend, Device, Platform};

    let tx = shared.tx;
    let Some(platform) = Platform::new() else {
        return;
    };
    let mut devices: Vec<<Platform as Backend>::Device> = Vec::new();
    let mut next_scan = Instant::now();
    // The first scan, and the one after a device was lost, always reports the device list.
    let mut announce = true;
    let mut events = shared.event_mode.load(Ordering::Relaxed);
    let mut cal = Calibrations::new();
    let mut cal_version = None;
    let mut exclusive_version = None;
    while !shared.stop.load(Ordering::Relaxed) {
        let mut changed = false;
        let wanted = shared.event_mode.load(Ordering::Relaxed);
        if wanted != events {
            // The read mode is set up when a device is opened: reopen them in the new mode.
            events = wanted;
            devices.clear();
            next_scan = Instant::now();
        }
        if Instant::now() >= next_scan {
            next_scan = Instant::now() + RESCAN;
            let attached = platform.attached();
            let before = devices.len();
            devices.retain(|d| attached.iter().any(|(id, _)| *id == d.state().info.guid));
            for (id, found) in &attached {
                if !devices.iter().any(|d| d.state().info.guid == *id)
                    && let Some(mut dev) = platform.open(found, events)
                {
                    dev.set_calibrations(&cal);
                    if Platform::EXCLUSIVE {
                        dev.set_exclusive(crate::exclusive::wanted(id));
                    }
                    devices.push(dev);
                }
            }
            if devices.len() != before || announce {
                announce = false;
                tx.send(InputEvent::DevicesChanged, Instant::now());
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

        let version = crate::exclusive::version();
        if Platform::EXCLUSIVE && exclusive_version != Some(version) {
            exclusive_version = Some(version);
            for dev in &mut devices {
                let on = crate::exclusive::wanted(&dev.state().info.guid);
                dev.set_exclusive(on);
            }
        }

        let mut lost = false;
        for dev in &mut devices {
            let at = Instant::now();
            let read = dev.read(&mut |event| {
                tx.send(event, at);
                changed = true;
            });
            if !read {
                lost = true;
                continue;
            }
            changed |= emit_changes(dev.previous(), dev.state(), tx, at);
            // Raw values matter to the calibration page even when the scaled value stays.
            changed |= dev.previous().raw != dev.state().raw;
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
                    .all(|(s, d)| s.info.guid == d.state().info.guid);
            if same_devices {
                // Usual case: only values changed, update them in place without allocating.
                for (s, d) in snapshot.iter_mut().zip(&devices) {
                    s.copy_readings(d.state());
                }
            } else {
                *snapshot = devices.iter().map(|d| d.state().clone()).collect();
            }
            drop(snapshot);
            on_change();
        }
        let polled = !events || devices.iter().any(|d| !d.signals());
        shared.polling.store(polled, Ordering::Relaxed);
        let poll = Duration::from_micros(shared.poll_interval.load(Ordering::Relaxed));
        platform.wait(&devices, events, poll);
    }
}

/// Sends one event per changed input, read at `at`; returns whether anything changed.
fn emit_changes(old: &DeviceState, new: &DeviceState, tx: &Broadcast, at: Instant) -> bool {
    let device = || new.info.guid.clone();
    let mut changed = false;
    for &id in &new.info.axes {
        let i = usize::from(id) - 1;
        // Ignore sensor noise below ~0.05%.
        if (old.axes[i] - new.axes[i]).abs() > 0.0005 {
            changed = true;
            tx.send(
                InputEvent::Axis {
                    device: device(),
                    id,
                    value: new.axes[i],
                },
                at,
            );
        }
    }
    for (i, (&a, &b)) in old.buttons.iter().zip(&new.buttons).enumerate() {
        if a != b {
            changed = true;
            tx.send(
                InputEvent::Button {
                    device: device(),
                    id: i as u16 + 1,
                    pressed: b,
                },
                at,
            );
        }
    }
    for (i, (&a, &b)) in old.hats.iter().zip(&new.hats).enumerate() {
        if a != b {
            changed = true;
            tx.send(
                InputEvent::Hat {
                    device: device(),
                    id: i as u8 + 1,
                    direction: b,
                },
                at,
            );
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full engine queue drops events without blocking and counts them until it drains.
    #[test]
    fn full_engine_queue_counts_lost_events() {
        let broadcast = Broadcast::default();
        let rx = broadcast.subscribe_timed();
        let lost = || match &broadcast.0.lock().unwrap()[0] {
            Subscriber::Timed(_, lost) => *lost,
            Subscriber::Plain(_) => unreachable!(),
        };
        for _ in 0..EVENT_BUFFER + 3 {
            broadcast.send(InputEvent::DevicesChanged, Instant::now());
        }
        assert_eq!(lost(), 3);
        rx.recv().unwrap();
        broadcast.send(InputEvent::DevicesChanged, Instant::now());
        assert_eq!(
            lost(),
            0,
            "reported and reset once the queue takes events again"
        );
        drop(rx);
        broadcast.send(InputEvent::DevicesChanged, Instant::now());
        assert!(
            broadcast.0.lock().unwrap().is_empty(),
            "gone with its receiver"
        );
    }
}
