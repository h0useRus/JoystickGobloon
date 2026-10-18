//! Attached joysticks with LED control (light-led): found on a background thread whenever
//! the attached devices change, so the GUI never waits for HID; the editor's "light it now"
//! writes in the background too.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use eframe::egui;
use gob_input::DeviceState;

#[derive(Default)]
pub struct Leds {
    found: Arc<Mutex<Vec<gob_lighting::Device>>>,
    /// HID paths of the attached devices the last search ran for.
    searched: Vec<String>,
}

impl Leds {
    /// Searches again (in the background) when the attached devices changed.
    pub fn update(&mut self, devices: &[DeviceState], ctx: &egui::Context) {
        let mut paths: Vec<String> = devices
            .iter()
            .filter(|d| !d.info.hid_path.is_empty())
            .map(|d| d.info.hid_path.to_ascii_lowercase())
            .collect();
        paths.sort_unstable();
        if paths == self.searched {
            return;
        }
        self.searched = paths;
        let (found, ctx) = (self.found.clone(), ctx.clone());
        let _ = std::thread::Builder::new()
            .name("led-search".into())
            .spawn(move || {
                let devices = gob_lighting::devices();
                log::info!("{} device(s) with LED control", devices.len());
                *found.lock().unwrap_or_else(|e| e.into_inner()) = devices;
                ctx.request_repaint();
            });
    }

    fn device(&self, hid_path: &str) -> Option<gob_lighting::Device> {
        self.found
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|d| d.is_at(hid_path))
            .cloned()
    }

    /// GUIDs of the attached devices with LED control.
    pub fn guids(&self, devices: &[DeviceState]) -> HashSet<String> {
        let found = self.found.lock().unwrap_or_else(|e| e.into_inner());
        devices
            .iter()
            .filter(|d| found.iter().any(|f| f.is_at(&d.info.hid_path)))
            .map(|d| d.info.guid.clone())
            .collect()
    }

    /// Sets one LED of the attached device `guid` now, in the background.
    pub fn preview(&self, devices: &[DeviceState], guid: &str, led: gob_lighting::Led) {
        let Some(device) = devices
            .iter()
            .find(|d| d.info.guid.eq_ignore_ascii_case(guid))
            .and_then(|d| self.device(&d.info.hid_path))
        else {
            log::warn!("light-led test: {guid} is not attached or has no LED control");
            return;
        };
        let _ = std::thread::Builder::new()
            .name("led-test".into())
            .spawn(move || {
                if let Err(e) = device.set(&[led]) {
                    log::warn!("light-led test on {}: {e}", device.name());
                }
            });
    }
}
