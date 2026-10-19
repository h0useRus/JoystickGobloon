//! Attached joysticks with LED control (light-led): found on a background thread whenever
//! the attached devices change, so the GUI never waits for HID; the editor's "light it now"
//! writes in the background too.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use eframe::egui;
use gob_core::LedMode;
use gob_i18n::I18n;
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

    /// Attached devices with LED control.
    pub fn attached(&self, devices: &[DeviceState]) -> Vec<LedDevice> {
        let guids = self.guids(devices);
        devices
            .iter()
            .filter(|d| guids.contains(&d.info.guid))
            .map(|d| LedDevice {
                guid: d.info.guid.clone(),
                name: d.info.name.trim().to_string(),
                vkb: d.info.vendor_id == gob_lighting::vkb::VENDOR_ID,
            })
            .collect()
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

    /// Sets LEDs of the attached device `guid` now, in the background.
    pub fn preview(&self, devices: &[DeviceState], guid: &str, leds: Vec<gob_lighting::Led>) {
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
                if let Err(e) = device.set(&leds) {
                    log::warn!("light-led test on {}: {e}", device.name());
                }
            });
    }
}

/// An attached device with LED control, for the settings page.
pub struct LedDevice {
    pub guid: String,
    pub name: String,
    /// LEDs have names (see [`led_widgets`]).
    pub vkb: bool,
}

/// One LED as the editors show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedValue {
    pub id: u8,
    pub color: [u8; 3],
    pub mode: LedMode,
}

impl LedValue {
    pub fn to_led(self) -> gob_lighting::Led {
        gob_lighting::Led {
            id: self.id,
            color: self.color,
            mode: gob_engine::led_mode(self.mode),
        }
    }
}

/// LEDs with names: VKB Gladiator NXT / EVO. Other numbers are typed in.
const VKB_LEDS: [(u8, &str); 3] = [
    (10, "led-vkb-grip"),
    (11, "led-vkb-hat"),
    (0, "led-vkb-base"),
];

/// LED number (with VKB names when `vkb`), color and mode in the current row; true when
/// `led` was changed.
pub fn led_widgets(
    ui: &mut egui::Ui,
    i18n: &I18n,
    salt: impl Hash + Copy,
    vkb: bool,
    led: &mut LedValue,
) -> bool {
    let before = *led;
    ui.label(egui::RichText::new(crate::style::icon::LIGHTBULB).size(crate::style::ICON_SIZE))
        .on_hover_text(i18n.tr("edit-led-id"));
    if vkb {
        let text = VKB_LEDS
            .iter()
            .find(|(n, _)| *n == led.id)
            .map_or_else(|| led.id.to_string(), |(_, k)| i18n.tr(k));
        egui::ComboBox::from_id_salt(("led-id", salt))
            .selected_text(text)
            .show_ui(ui, |ui| {
                for (n, key) in VKB_LEDS {
                    let text = format!("{n}  {}", i18n.tr(key));
                    if ui.selectable_label(n == led.id, text).clicked() {
                        led.id = n;
                    }
                }
            });
    }
    ui.add(egui::DragValue::new(&mut led.id));
    ui.separator();
    egui::color_picker::color_edit_button_srgb(ui, &mut led.color)
        .on_hover_text(i18n.tr("edit-led-color"));
    egui::ComboBox::from_id_salt(("led-mode", salt))
        .selected_text(i18n.tr(&format!("led-mode-{}", led.mode.name())))
        .show_ui(ui, |ui| {
            for m in LedMode::ALL {
                let text = i18n.tr(&format!("led-mode-{}", m.name()));
                if ui.selectable_label(m == led.mode, text).clicked() {
                    led.mode = m;
                }
            }
        })
        .response
        .on_hover_text(i18n.tr("edit-led-mode"));
    *led != before
}
