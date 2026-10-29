//! Modifiers window: name the buttons that, while held, switch the inputs to another layer of
//! actions (see `gob_core::modifier`). A button is assigned by pressing it on a controller.

use std::collections::{HashMap, HashSet};

use eframe::egui::{self, RichText};
use gob_core::{Modifier, Profile};
use gob_i18n::I18n;
use gob_input::DeviceState;

use crate::style::{self, icon};

pub enum ModifierOp {
    /// Adds a modifier on a button; `device_name` lists the device in the profile.
    Add {
        name: String,
        device: String,
        device_name: String,
        button: u32,
    },
    Rename {
        old: String,
        new: String,
    },
    SetButton {
        name: String,
        device: String,
        device_name: String,
        button: u32,
    },
    Remove(String),
}

pub fn apply(p: &mut Profile, op: ModifierOp) {
    let list_device = |p: &mut Profile, device: &str, name: String| {
        if !p.devices.iter().any(|d| d.id.eq_ignore_ascii_case(device)) {
            p.devices.push(gob_core::Device {
                id: device.into(),
                name,
            });
        }
    };
    match op {
        ModifierOp::Add {
            name,
            device,
            device_name,
            button,
        } => {
            list_device(p, &device, device_name);
            p.add_modifier(Modifier {
                name,
                device_id: device,
                input_type: "button".into(),
                input_id: button.to_string(),
            });
        }
        ModifierOp::Rename { old, new } => {
            p.rename_modifier(&old, new.trim());
        }
        ModifierOp::SetButton {
            name,
            device,
            device_name,
            button,
        } => {
            list_device(p, &device, device_name);
            p.set_modifier_input(&name, &device, "button", &button.to_string());
        }
        ModifierOp::Remove(name) => {
            p.remove_modifier(&name);
        }
    }
}

/// What a pressed button will be used for.
#[derive(Clone, PartialEq)]
enum Capture {
    New,
    Reassign(String),
}

/// Text being edited per modifier (renames apply on Enter / focus loss) and the button capture.
#[derive(Default)]
pub struct ModifiersWindow {
    names: HashMap<String, String>,
    capture: Option<Capture>,
    /// Buttons already down when the capture started: only a new press counts.
    held: HashSet<(String, usize)>,
}

impl ModifiersWindow {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &I18n,
        profile: &Profile,
        devices: &[DeviceState],
        open: &mut bool,
    ) -> Vec<ModifierOp> {
        let mut ops = Vec::new();
        if let Some(capture) = self.capture.clone() {
            ctx.request_repaint();
            if let Some((device, name, button)) = self.pressed(devices) {
                let taken = profile
                    .modifier_at(&device, "button", &button.to_string())
                    .is_some();
                if !taken {
                    ops.push(match capture {
                        Capture::New => ModifierOp::Add {
                            name: next_name(profile),
                            device,
                            device_name: name,
                            button,
                        },
                        Capture::Reassign(modifier) => ModifierOp::SetButton {
                            name: modifier,
                            device,
                            device_name: name,
                            button,
                        },
                    });
                }
                self.capture = None;
            }
        }
        let title = i18n.tr("modifiers-edit");
        let window = egui::Window::new(&title)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center());
        style::dialog_window(ctx, window, &title, open, |ui| {
            if profile.modifiers.is_empty() {
                ui.label(RichText::new(i18n.tr("modifiers-none")).weak());
                ui.add_space(6.0);
            }
            egui::Grid::new("modifiers")
                .num_columns(3)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    for m in &profile.modifiers {
                        let text = self
                            .names
                            .entry(m.name.clone())
                            .or_insert_with(|| m.name.clone());
                        let r = ui.add(style::text_field(text).desired_width(80.0));
                        if r.lost_focus() && *text != m.name {
                            ops.push(ModifierOp::Rename {
                                old: m.name.clone(),
                                new: text.clone(),
                            });
                            self.names.clear();
                        }
                        let waiting = self.capture == Some(Capture::Reassign(m.name.clone()));
                        let caption = if waiting {
                            i18n.tr("modifiers-press")
                        } else {
                            button_name(i18n, profile, devices, m)
                        };
                        let button =
                            egui::Button::new(format!("{}  {caption}", icon::RADIO_BUTTON))
                                .selected(waiting);
                        if ui
                            .add(button)
                            .on_hover_text(i18n.tr("modifiers-assign"))
                            .clicked()
                        {
                            self.start(
                                devices,
                                (!waiting).then(|| Capture::Reassign(m.name.clone())),
                            );
                        }
                        if style::icon_button(ui, true, icon::TRASH, &i18n.tr("modifiers-remove"))
                            .clicked()
                        {
                            ops.push(ModifierOp::Remove(m.name.clone()));
                            self.names.clear();
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(4.0);
            let waiting = self.capture == Some(Capture::New);
            let text = if waiting {
                format!("{}  {}", icon::X, i18n.tr("modifiers-press"))
            } else {
                format!("{}  {}", icon::PLUS, i18n.tr("modifiers-add"))
            };
            if ui.add(egui::Button::new(text).selected(waiting)).clicked() {
                self.start(devices, (!waiting).then_some(Capture::New));
            }
        });
        if !*open {
            self.capture = None;
        }
        ops
    }

    /// Starts (or with `None` cancels) waiting for a button press.
    fn start(&mut self, devices: &[DeviceState], capture: Option<Capture>) {
        self.held = down(devices).collect();
        self.capture = capture;
    }

    /// The first button pressed since the capture started: device id, device name, button.
    fn pressed(&mut self, devices: &[DeviceState]) -> Option<(String, String, u32)> {
        let now: HashSet<(String, usize)> = down(devices).collect();
        // A button let go and pressed again counts as a new press.
        self.held.retain(|b| now.contains(b));
        let (guid, i) = now.into_iter().find(|b| !self.held.contains(b))?;
        let name = devices
            .iter()
            .find(|d| d.info.guid == guid)
            .map(|d| d.info.name.clone())
            .unwrap_or_default();
        Some((guid, name, i as u32 + 1))
    }
}

/// Pressed buttons of the physical controllers: (device id, 0-based button).
fn down(devices: &[DeviceState]) -> impl Iterator<Item = (String, usize)> + '_ {
    devices.iter().filter(|d| !d.info.is_own()).flat_map(|d| {
        d.buttons
            .iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .map(move |(i, _)| (d.info.guid.clone(), i))
    })
}

/// `M1`, `M2`, ...: the first free one.
fn next_name(profile: &Profile) -> String {
    (1..)
        .map(|n| format!("M{n}"))
        .find(|n| profile.modifier(n).is_none())
        .unwrap_or_default()
}

/// "Device name: Button 2" for a modifier.
fn button_name(i18n: &I18n, profile: &Profile, devices: &[DeviceState], m: &Modifier) -> String {
    let device = devices
        .iter()
        .find(|d| d.info.guid.eq_ignore_ascii_case(&m.device_id))
        .map(|d| d.info.name.trim().to_string())
        .or_else(|| profile.device_name(&m.device_id).map(String::from))
        .unwrap_or_else(|| m.device_id.clone());
    let n: u32 = m.input_id.trim().parse().unwrap_or(0);
    let button = i18n.trf("vjoy-button-n", &[("n", n.into())]);
    format!("{device}: {button}")
}
