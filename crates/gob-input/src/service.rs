//! Background polling thread with a shared snapshot and an event channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{DeviceState, InputEvent};

const POLL_INTERVAL: Duration = Duration::from_millis(4);
/// Re-enumerate devices every this many polls (~2 s) to catch hot-plugging.
const RESCAN_EVERY: u32 = 500;
const EVENT_BUFFER: usize = 4096;

pub struct InputService {
    snapshot: Arc<RwLock<Vec<DeviceState>>>,
    events: Receiver<InputEvent>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InputService {
    /// Starts polling in a background thread. `on_change` is called after every
    /// change (e.g. to request a GUI repaint).
    pub fn start(on_change: impl Fn() + Send + 'static) -> Self {
        let snapshot = Arc::new(RwLock::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::sync_channel(EVENT_BUFFER);
        let thread = {
            let (snapshot, stop) = (snapshot.clone(), stop.clone());
            std::thread::Builder::new()
                .name("gob-input".into())
                .spawn(move || run(&snapshot, &stop, &tx, &on_change))
                .ok()
        };
        Self {
            snapshot,
            events,
            stop,
            thread,
        }
    }

    /// Current state of every attached device.
    pub fn devices(&self) -> RwLockReadGuard<'_, Vec<DeviceState>> {
        self.snapshot.read().unwrap_or_else(|e| e.into_inner())
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
fn run(_: &RwLock<Vec<DeviceState>>, _: &AtomicBool, _: &SyncSender<InputEvent>, _: &dyn Fn()) {}

#[cfg(windows)]
fn run(
    snapshot: &RwLock<Vec<DeviceState>>,
    stop: &AtomicBool,
    tx: &SyncSender<InputEvent>,
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
                if !devices.iter().any(|d| d.state.info.guid == guid_of(inst)) {
                    if let Ok(dev) = di.open(inst) {
                        devices.push(dev);
                    }
                }
            }
            if devices.len() != before || tick == 0 {
                let _ = tx.try_send(InputEvent::DevicesChanged);
                changed = true;
            }
        }
        tick = tick.wrapping_add(1);

        let mut lost = false;
        for dev in &mut devices {
            let previous = dev.state.clone();
            if !dev.poll() {
                lost = true;
                continue;
            }
            changed |= emit_changes(&previous, &dev.state, tx);
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
fn emit_changes(old: &DeviceState, new: &DeviceState, tx: &SyncSender<InputEvent>) -> bool {
    let device = || new.info.guid.clone();
    let mut changed = false;
    for &id in &new.info.axes {
        let i = usize::from(id) - 1;
        // Ignore sensor noise below ~0.05%.
        if (old.axes[i] - new.axes[i]).abs() > 0.0005 {
            changed = true;
            let _ = tx.try_send(InputEvent::Axis {
                device: device(),
                id,
                value: new.axes[i],
            });
        }
    }
    for (i, (&a, &b)) in old.buttons.iter().zip(&new.buttons).enumerate() {
        if a != b {
            changed = true;
            let _ = tx.try_send(InputEvent::Button {
                device: device(),
                id: i as u16 + 1,
                pressed: b,
            });
        }
    }
    for (i, (&a, &b)) in old.hats.iter().zip(&new.hats).enumerate() {
        if a != b {
            changed = true;
            let _ = tx.try_send(InputEvent::Hat {
                device: device(),
                id: i as u8 + 1,
                direction: b,
            });
        }
    }
    changed
}
