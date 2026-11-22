//! Background polling thread with a shared snapshot and an event channel.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::filter::Filtered;
use crate::{Calibration, DeviceState, InputEvent, Timed};

/// Calibration per (device GUID, axis slot); axes without one use DirectInput's range.
pub type Calibrations = std::collections::HashMap<(String, u8), Calibration>;

/// Re-enumerate devices this often to catch hot-plugging.
const RESCAN: Duration = Duration::from_secs(2);

/// A device that was found but did not open is tried again this often, not on every scan
/// (opening can be slow too).
const REOPEN: Duration = Duration::from_secs(30);
const EVENT_BUFFER: usize = 4096;
/// Range and default of [`InputService::set_poll_interval`].
pub const MIN_POLL_INTERVAL: Duration = Duration::from_millis(1);
pub const MAX_POLL_INTERVAL: Duration = Duration::from_millis(5);
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(1);
/// Largest [`crate::Calibration::jitter`]: 5 % of the travel.
pub const MAX_JITTER: u16 = 50;
/// Largest debounce time of [`InputService::set_debounce`].
pub const MAX_DEBOUNCE_MS: u16 = 100;

/// Shared, read-only side of an [`InputService`]: see [`InputService::handle`].
#[derive(Clone)]
pub struct InputHandle {
    snapshot: Arc<RwLock<Vec<DeviceState>>>,
    broadcast: Broadcast,
    scanned: Arc<AtomicBool>,
}

impl InputHandle {
    /// The input thread has looked for devices once: [`Self::devices`] lists what is attached
    /// (before that it may be empty only because nobody looked yet).
    pub fn scanned(&self) -> bool {
        self.scanned.load(Ordering::Acquire)
    }

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
    /// Button and hat debounce time per device, in milliseconds.
    debounce: Arc<RwLock<HashMap<String, u16>>>,
    /// Bumped on every calibration or debounce change, so the polling thread copies the maps
    /// only then.
    calibration_version: Arc<AtomicU64>,
    /// See [`InputService::set_event_mode`].
    event_mode: Arc<AtomicBool>,
    /// See [`InputService::set_poll_interval`], in microseconds.
    poll_interval: Arc<AtomicU64>,
    /// See [`InputService::polls_devices`].
    polling: Arc<AtomicBool>,
    /// See [`InputHandle::scanned`].
    scanned: Arc<AtomicBool>,
    /// Keyboard keys as events (low-level hook); `None` if it could not be installed.
    _keyboard: Option<crate::platform::keyboard::Hook>,
    /// See [`InputService::set_osc`].
    osc: Mutex<Osc>,
    on_change: Arc<dyn Fn() + Send + Sync>,
}

/// The OSC receiver: where it was asked to listen and how that went.
#[derive(Default)]
struct Osc {
    addr: Option<std::net::SocketAddr>,
    listener: Option<gob_osc::Listener>,
    status: OscStatus,
}

/// Whether OSC messages are received ([`InputService::set_osc`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OscStatus {
    #[default]
    Off,
    /// Listening on this port.
    Listening(u16),
    /// The port could not be taken (English, for the log and a tooltip).
    Failed(String),
}

impl InputService {
    /// Starts polling in a background thread. `on_change` is called after every
    /// change (e.g. to request a GUI repaint).
    pub fn start(on_change: impl Fn() + Send + Sync + 'static) -> Self {
        let on_change = Arc::new(on_change);
        let calibrations: Arc<RwLock<Calibrations>> = Arc::default();
        let debounce: Arc<RwLock<HashMap<String, u16>>> = Arc::default();
        let calibration_version = Arc::new(AtomicU64::new(0));
        let event_mode = Arc::new(AtomicBool::new(false));
        let poll_interval = Arc::new(AtomicU64::new(DEFAULT_POLL_INTERVAL.as_micros() as u64));
        let polling = Arc::new(AtomicBool::new(true));
        let scanned = Arc::new(AtomicBool::new(false));
        let snapshot = Arc::new(RwLock::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let broadcast = Broadcast::default();
        let events = broadcast.subscribe();
        let thread = {
            let (snapshot, stop, tx) = (snapshot.clone(), stop.clone(), broadcast.clone());
            let (on_change, cal) = (on_change.clone(), calibrations.clone());
            let debounce = debounce.clone();
            let (version, mode) = (calibration_version.clone(), event_mode.clone());
            let (poll, polling) = (poll_interval.clone(), polling.clone());
            let scanned = scanned.clone();
            std::thread::Builder::new()
                .name("gob-input".into())
                .spawn(move || {
                    let shared = Shared {
                        snapshot: &snapshot,
                        stop: &stop,
                        tx: &tx,
                        calibrations: &cal,
                        debounce: &debounce,
                        calibration_version: &version,
                        event_mode: &mode,
                        poll_interval: &poll,
                        polling: &polling,
                        scanned: &scanned,
                    };
                    run(&shared, &*on_change)
                })
                .ok()
        };
        let keyboard = {
            let tx = broadcast.clone();
            let on_change = on_change.clone();
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
            debounce,
            calibration_version,
            event_mode,
            poll_interval,
            polling,
            scanned,
            _keyboard: keyboard,
            osc: Mutex::default(),
            on_change,
        }
    }

    /// Receives OSC messages on `addr` (`127.0.0.1:port` = from this computer only,
    /// `0.0.0.0:port` = from the network too) as [`InputEvent::Osc`]; `None` stops. Cheap when
    /// nothing changed: call it as often as the settings may have. A port that could not be
    /// taken is tried again only when `addr` changes.
    pub fn set_osc(&self, addr: Option<std::net::SocketAddr>) {
        let mut osc = self.osc.lock().unwrap_or_else(|e| e.into_inner());
        if osc.addr == addr {
            return;
        }
        // The old port is let go first: the new address may be the same port on another
        // interface.
        osc.listener = None;
        osc.addr = addr;
        osc.status = OscStatus::Off;
        let Some(addr) = addr else {
            log::info!("OSC: receiving stopped");
            return;
        };
        let (tx, on_change) = (self.broadcast.clone(), self.on_change.clone());
        let started = gob_osc::Listener::start(addr, move |message, _from| {
            let value = message.args.first().and_then(gob_osc::Arg::number);
            let event = InputEvent::Osc {
                address: message.address,
                value,
            };
            tx.send(event, Instant::now());
            on_change();
        });
        match started {
            Ok(listener) => {
                log::info!("OSC: receiving on {}", listener.local_addr());
                osc.status = OscStatus::Listening(listener.local_addr().port());
                osc.listener = Some(listener);
            }
            Err(e) => {
                log::warn!("OSC: cannot receive on {addr}: {e}");
                osc.status = OscStatus::Failed(e.to_string());
            }
        }
    }

    pub fn osc_status(&self) -> OscStatus {
        self.osc
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status
            .clone()
    }

    /// Sets the button and hat debounce time of one device (0 = off, at most
    /// [`MAX_DEBOUNCE_MS`]): after a change, further ones within that time are ignored, then
    /// the final state follows. Applies from the next poll.
    pub fn set_debounce(&self, device: &str, ms: u16) {
        let mut map = self.debounce.write().unwrap_or_else(|e| e.into_inner());
        if ms == 0 {
            map.remove(device);
        } else {
            map.insert(device.to_string(), ms.min(MAX_DEBOUNCE_MS));
        }
        drop(map);
        self.calibration_version.fetch_add(1, Ordering::Release);
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

    /// See [`InputHandle::scanned`].
    pub fn scanned(&self) -> bool {
        self.scanned.load(Ordering::Acquire)
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
            scanned: self.scanned.clone(),
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
    debounce: &'a RwLock<HashMap<String, u16>>,
    calibration_version: &'a AtomicU64,
    event_mode: &'a AtomicBool,
    poll_interval: &'a AtomicU64,
    polling: &'a AtomicBool,
    scanned: &'a AtomicBool,
}

/// Attached devices with their ids, as [`crate::platform::Backend::attached`] lists them.
type Attached<P> = Vec<(String, <P as crate::platform::Backend>::Found)>;

/// Looks for attached devices every [`RESCAN`] on a thread of its own, with a backend of its own,
/// and sends what it found; it ends with the input thread (the receiver dropped). `None` when
/// the thread did not start (the input thread then looks itself).
fn spawn_scanner<P: crate::platform::Backend>() -> Option<std::sync::mpsc::Receiver<Attached<P>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("input-scan".into())
        .spawn(move || {
            let Some(platform) = P::new() else {
                return;
            };
            loop {
                std::thread::sleep(RESCAN);
                let at = Instant::now();
                let attached = platform.attached();
                let took = at.elapsed();
                if took > Duration::from_millis(100) {
                    log::debug!("looking for controllers took {} ms", took.as_millis());
                }
                if tx.send(attached).is_err() {
                    return;
                }
            }
        });
    match spawned {
        Ok(_) => Some(rx),
        Err(e) => {
            log::warn!("input: the device scan thread did not start ({e}): scanning in place");
            None
        }
    }
}

/// The input thread: finds, opens and reads controllers through this platform's backend.
fn run(shared: &Shared, on_change: &dyn Fn()) {
    use crate::platform::{Backend, Device, Platform};

    let tx = shared.tx;
    let Some(platform) = Platform::new() else {
        return;
    };
    let mut devices: Vec<<Platform as Backend>::Device> = Vec::new();
    // The attached devices: looked up here, or sent by the scanning thread.
    let scans = if Platform::SCAN_ON_THREAD {
        spawn_scanner::<Platform>()
    } else {
        None
    };
    let mut attached = platform.attached();
    // Devices that did not open, and when they may be tried again.
    let mut failed: HashMap<String, Instant> = HashMap::new();
    let mut next_scan = Instant::now();
    // The first scan, and the one after a device was lost, always reports the device list.
    let mut announce = true;
    let mut events = shared.event_mode.load(Ordering::Relaxed);
    let mut cal = Calibrations::new();
    let mut debounce: HashMap<String, u16> = HashMap::new();
    // What every device last sent, by device id (see `filter`).
    let mut filters: HashMap<String, Filtered> = HashMap::new();
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
        let mut rescan = Instant::now() >= next_scan;
        match &scans {
            Some(rx) => {
                if let Some(list) = rx.try_iter().last() {
                    attached = list;
                    rescan = true;
                }
            }
            None if rescan => attached = platform.attached(),
            None => {}
        }
        if rescan {
            next_scan = Instant::now() + RESCAN;
            let now = Instant::now();
            failed.retain(|id, retry| *retry > now && attached.iter().any(|(a, _)| a == id));
            let before = devices.len();
            devices.retain(|d| attached.iter().any(|(id, _)| *id == d.state().info.guid));
            filters.retain(|id, _| devices.iter().any(|d| d.state().info.guid == *id));
            for (id, found) in &attached {
                if devices.iter().any(|d| d.state().info.guid == *id) || failed.contains_key(id) {
                    continue;
                }
                let Some(mut dev) = platform.open(found, events) else {
                    failed.insert(id.clone(), now + REOPEN);
                    continue;
                };
                dev.set_calibrations(&cal);
                let mut filter = Filtered::new(dev.state());
                filter.configure(&cal, debounce.get(id).copied().unwrap_or(0));
                filters.insert(id.clone(), filter);
                if Platform::EXCLUSIVE {
                    dev.set_exclusive(crate::exclusive::wanted(id));
                }
                devices.push(dev);
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
            debounce.clone_from(&shared.debounce.read().unwrap_or_else(|e| e.into_inner()));
            for dev in &mut devices {
                dev.set_calibrations(&cal);
            }
            for (id, filter) in &mut filters {
                filter.configure(&cal, debounce.get(id).copied().unwrap_or(0));
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
        // A device's layout changed: the snapshot is built anew.
        let mut relayout = false;
        for dev in &mut devices {
            let at = Instant::now();
            // Looked up by reference: an owned key would allocate per device and pass.
            if !filters.contains_key(&dev.state().info.guid) {
                let filter = Filtered::new(dev.state());
                filters.insert(dev.state().info.guid.clone(), filter);
            }
            let Some(filter) = filters.get_mut(&dev.state().info.guid) else {
                continue;
            };
            let read = dev.read(&mut |event| {
                if let Some(event) = filter.event(event, at) {
                    tx.send(event, at);
                    changed = true;
                }
            });
            if !read {
                lost = true;
                continue;
            }
            if filter.follow_layout(dev.state()) {
                relayout = true;
                changed = true;
            }
            changed |= filter.update(dev.state(), at, &mut |event| tx.send(event, at));
            // Raw values matter to the calibration page even when the scaled value stays.
            changed |= dev.previous().raw != dev.state().raw;
        }
        if lost || relayout {
            next_scan = Instant::now(); // rescan (and announce) right away
            announce = true;
        }
        if changed {
            let mut snapshot = shared.snapshot.write().unwrap_or_else(|e| e.into_inner());
            let same_devices = !relayout
                && snapshot.len() == devices.len()
                && snapshot
                    .iter()
                    .zip(&devices)
                    .all(|(s, d)| s.info.guid == d.state().info.guid);
            // The filtered view of each device: what was sent.
            fn shown<'a>(
                filters: &'a HashMap<String, Filtered>,
                d: &'a impl Device,
            ) -> &'a DeviceState {
                filters
                    .get(&d.state().info.guid)
                    .map_or(d.state(), |f| &f.out)
            }
            if same_devices {
                // Usual case: only values changed, update them in place without allocating.
                for (s, d) in snapshot.iter_mut().zip(&devices) {
                    s.copy_readings(shown(&filters, d));
                }
            } else {
                *snapshot = devices.iter().map(|d| shown(&filters, d).clone()).collect();
            }
            drop(snapshot);
            on_change();
        }
        // The devices found by the first pass are in the snapshot now.
        shared.scanned.store(true, Ordering::Release);
        let polled = !events || devices.iter().any(|d| !d.signals());
        shared.polling.store(polled, Ordering::Relaxed);
        let poll = Duration::from_micros(shared.poll_interval.load(Ordering::Relaxed));
        // A debounced button may owe its final state when its hold ends.
        let wake = filters
            .values()
            .filter_map(Filtered::next_deadline)
            .min()
            .map(|d| d.saturating_duration_since(Instant::now()));
        platform.wait(&devices, events, poll, wake);
    }
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
