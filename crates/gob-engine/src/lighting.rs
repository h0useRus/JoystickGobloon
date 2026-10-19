//! Joystick LEDs (light-led) on a background thread, started on first use: HID requests
//! take milliseconds and must not hold up the engine. Commands queued meanwhile are merged
//! (the last one per LED wins) and each device gets one write. Failures are logged once
//! per device; they never stop the engine.
//!
//! When the runner stops (profile deactivated), every device the profile wrote to gets back
//! the state saved before its first write, then the user's default colors. VKB keeps written
//! colors until the stick restarts, so the defaults are what stands in for its own profile.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

use gob_core::LedMode;
use gob_lighting::{Led, Mode};

use crate::Light;

pub struct Lighting {
    /// Attached device GUID (lowercase) -> HID interface path.
    paths: HashMap<String, String>,
    /// Colors set when the runner stops, on the devices the profile wrote to.
    defaults: Vec<Light>,
    tx: Option<Sender<Light>>,
    thread: Option<JoinHandle<()>>,
}

impl Lighting {
    /// `paths`: GUID -> HID path of the attached devices (`DeviceInfo::hid_path`).
    pub fn new(paths: HashMap<String, String>, defaults: Vec<Light>) -> Self {
        Self {
            paths,
            defaults,
            tx: None,
            thread: None,
        }
    }

    pub fn send(&mut self, light: &Light) {
        if self.tx.is_none() {
            let (tx, rx) = channel();
            let paths = std::mem::take(&mut self.paths);
            let defaults = std::mem::take(&mut self.defaults);
            match std::thread::Builder::new()
                .name("gob-lighting".into())
                .spawn(move || run(&paths, &defaults, &rx))
            {
                Ok(t) => self.thread = Some(t),
                Err(e) => log::warn!("light-led: no lighting thread: {e}"),
            }
            self.tx = Some(tx);
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(light.clone());
        }
    }
}

impl Drop for Lighting {
    /// Finishes the queued writes, so a profile started next sets its colors after them.
    fn drop(&mut self) {
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The profile's LED mode as gob-lighting takes it.
pub fn led_mode(m: LedMode) -> Mode {
    match m {
        LedMode::Off => Mode::Off,
        LedMode::On => Mode::On,
        LedMode::SlowBlink => Mode::SlowBlink,
        LedMode::FastBlink => Mode::FastBlink,
        LedMode::FastestBlink => Mode::FastestBlink,
    }
}

/// Queued lights grouped per device, the last one per LED kept, in first-seen order.
fn batch(lights: impl IntoIterator<Item = Light>) -> Vec<(String, Vec<Led>)> {
    let mut out: Vec<(String, Vec<Led>)> = Vec::new();
    for l in lights {
        let led = Led {
            id: l.led,
            color: l.color,
            mode: led_mode(l.mode),
        };
        let pos = match out.iter().position(|(d, _)| *d == l.device) {
            Some(p) => p,
            None => {
                out.push((l.device, Vec::new()));
                out.len() - 1
            }
        };
        let leds = &mut out[pos].1;
        leds.retain(|x| x.id != led.id);
        leds.push(led);
    }
    out
}

fn run(paths: &HashMap<String, String>, defaults: &[Light], rx: &Receiver<Light>) {
    let devices = gob_lighting::devices();
    let mut warned = HashSet::new();
    // Devices written to, in the order first touched, with their state before that.
    let mut touched: Vec<(String, &gob_lighting::Device, Option<gob_lighting::Saved>)> = Vec::new();
    while let Ok(first) = rx.recv() {
        for (guid, leds) in batch(std::iter::once(first).chain(rx.try_iter())) {
            let device = paths
                .get(&guid)
                .and_then(|p| devices.iter().find(|d| d.is_at(p)));
            let result = match device {
                Some(d) => {
                    if !touched.iter().any(|(g, ..)| *g == guid) {
                        let saved = d
                            .save()
                            .map_err(|e| log::warn!("light-led {guid}: state not saved: {e}"))
                            .ok();
                        touched.push((guid.clone(), d, saved));
                    }
                    d.set(&leds).map_err(|e| e.to_string())
                }
                None => Err("device not attached or without LED control".into()),
            };
            if let Err(e) = result {
                if warned.insert(guid.clone()) {
                    log::warn!("light-led {guid}: {e}");
                }
            }
        }
    }
    // The runner stopped: the state before the profile, then the default colors.
    for (guid, device, saved) in &touched {
        let restored = saved.as_ref().map_or(Ok(()), |s| device.restore(s));
        let own: Vec<Light> = defaults
            .iter()
            .filter(|l| l.device == *guid)
            .cloned()
            .collect();
        let result = restored.and_then(|()| match batch(own).pop() {
            Some((_, leds)) => device.set(&leds),
            None => Ok(()),
        });
        match result {
            Ok(()) => log::info!("light-led {guid}: LEDs restored"),
            Err(e) => log::warn!("light-led {guid}: LEDs not restored: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(device: &str, led: u8, r: u8) -> Light {
        Light {
            device: device.into(),
            led,
            color: [r, 0, 0],
            mode: LedMode::On,
        }
    }

    #[test]
    fn batches_per_device_last_wins() {
        let b = batch([
            light("a", 10, 1),
            light("b", 10, 2),
            light("a", 11, 3),
            light("a", 10, 4),
        ]);
        let ids = |leds: &[Led]| leds.iter().map(|l| (l.id, l.color[0])).collect::<Vec<_>>();
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].0, "a");
        assert_eq!(ids(&b[0].1), [(11, 3), (10, 4)]);
        assert_eq!(ids(&b[1].1), [(10, 2)]);
    }
}
