//! Device calibration (like Gremlin's): per axis low / center / high in raw DirectInput
//! units, captured by moving the axis or typed in. Saved calibrations are applied by
//! gob-input, so every page and the engine see calibrated values.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Vec2};
use gob_i18n::I18n;
use gob_input::{Calibration, Calibrations, DeviceState};

use crate::style::{self, icon};

const RAW_MIN: i32 = -32768;
const RAW_MAX: i32 = 32767;
/// Width of the button column on the right.
const BUTTONS: f32 = 170.0;

#[derive(Clone, Copy, PartialEq)]
enum Capture {
    /// Moving the stick around its rest position records the center band.
    Center { min: i32, max: i32 },
    /// Moving the axis to both ends records the extremes.
    Extrema { min: i32, max: i32 },
}

#[derive(Default)]
pub struct CalibrationView {
    device: Option<String>,
    /// Edited calibrations, by (device, axis slot); `true` = not saved yet.
    edits: HashMap<(String, u8), (Calibration, bool)>,
    capture: Option<((String, u8), Capture)>,
}

/// What the app has to do after a frame.
#[derive(Default)]
pub struct Output {
    /// Switch between the page and the separate window.
    pub toggle_window: bool,
    /// Calibrations to save and apply.
    pub save: Vec<(String, u8, Calibration)>,
}

impl CalibrationView {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        devices: &[DeviceState],
        saved: &Calibrations,
        detached: bool,
    ) -> Output {
        let mut out = Output::default();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        let physical: Vec<&DeviceState> = devices.iter().filter(|d| !d.info.is_vjoy()).collect();

        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.tr("nav-calibration")).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (glyph, key) = if detached {
                    (icon::ARROW_SQUARE_IN, "viewer-attach")
                } else {
                    (icon::ARROW_SQUARE_OUT, "viewer-detach")
                };
                out.toggle_window = style::icon_button(ui, true, glyph, &i18n.tr(key)).clicked();
            });
        });
        ui.separator();
        if physical.is_empty() {
            crate::app::empty_state(ui, icon::JOYSTICK, &i18n.tr("no-devices-attached"));
            return out;
        }
        if !self
            .device
            .as_ref()
            .is_some_and(|g| physical.iter().any(|d| &d.info.guid == g))
        {
            self.device = Some(physical[0].info.guid.clone());
        }
        let guid = self.device.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(i18n.tr("calibration-device"));
            let name = physical
                .iter()
                .find(|d| d.info.guid == guid)
                .map_or(String::new(), |d| d.info.name.trim().to_string());
            egui::ComboBox::from_id_salt("calibration-device")
                .selected_text(name)
                .width(260.0)
                .show_ui(ui, |ui| {
                    for d in &physical {
                        if ui
                            .selectable_label(d.info.guid == guid, d.info.name.trim())
                            .clicked()
                        {
                            self.device = Some(d.info.guid.clone());
                            self.capture = None;
                        }
                    }
                });
        });
        ui.add_space(4.0);
        let Some(device) = physical.iter().find(|d| d.info.guid == guid) else {
            return out;
        };

        // One width for every block, measured once: blocks sized from what is left after
        // the previous one would creep wider down the list.
        let width = ui.available_width() - 24.0;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for &slot in &device.info.axes {
                    let key = (guid.clone(), slot);
                    let raw = device.raw[usize::from(slot) - 1];
                    self.track(&key, raw);
                    let (mut cal, mut unsaved) = self
                        .edits
                        .get(&key)
                        .copied()
                        .unwrap_or((saved.get(&key).copied().unwrap_or_default(), false));
                    let before = cal;
                    let capture = self
                        .capture
                        .as_ref()
                        .filter(|(k, _)| *k == key)
                        .map(|(_, c)| *c);
                    let mut action = None;
                    axis_block(
                        ui,
                        i18n,
                        width,
                        slot,
                        raw,
                        &mut cal,
                        unsaved,
                        capture,
                        &mut action,
                    );
                    match action {
                        Some(Action::Reset) => cal = Calibration::default(),
                        Some(Action::Save) => {
                            out.save.push((guid.clone(), slot, cal));
                            unsaved = false;
                        }
                        Some(Action::Center) => {
                            self.capture = match capture {
                                Some(Capture::Center { .. }) => None,
                                _ => Some((key.clone(), Capture::Center { min: raw, max: raw })),
                            };
                        }
                        Some(Action::Extrema) => {
                            self.capture = match capture {
                                Some(Capture::Extrema { .. }) => None,
                                _ => Some((key.clone(), Capture::Extrema { min: raw, max: raw })),
                            };
                        }
                        None => {}
                    }
                    if cal != before {
                        unsaved = true;
                    }
                    self.edits.insert(key, (cal, unsaved));
                    ui.add_space(8.0);
                }
            });
        out
    }

    /// While capturing, widens the recorded range with the current raw value.
    fn track(&mut self, key: &(String, u8), raw: i32) {
        let Some((k, capture)) = &mut self.capture else {
            return;
        };
        if k != key {
            return;
        }
        let entry = self
            .edits
            .entry(key.clone())
            .or_insert((Calibration::default(), true));
        match capture {
            Capture::Center { min, max } => {
                *min = (*min).min(raw);
                *max = (*max).max(raw);
                entry.0.center_low = *min;
                entry.0.center_high = *max;
            }
            Capture::Extrema { min, max } => {
                *min = (*min).min(raw);
                *max = (*max).max(raw);
                entry.0.low = *min;
                entry.0.high = *max;
            }
        }
        entry.1 = true;
    }
}

enum Action {
    Reset,
    Save,
    Center,
    Extrema,
}

#[allow(clippy::too_many_arguments)]
fn axis_block(
    ui: &mut egui::Ui,
    i18n: &I18n,
    width: f32,
    slot: u8,
    raw: i32,
    cal: &mut Calibration,
    unsaved: bool,
    capture: Option<Capture>,
    action: &mut Option<Action>,
) {
    let frame = egui::Frame::new()
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(6)
        .inner_margin(8);
    frame.show(ui, |ui| {
        // `width` is the block's outer width; the frame adds 8 px margins on both sides.
        let inner = width - 16.0;
        ui.set_width(inner);
        // Two columns: bars and values on the left, buttons on the right (same width).
        let gap = ui.spacing().item_spacing.x;
        let bar_w = (inner - BUTTONS - gap).max(160.0);
        let h = ui.spacing().interact_size.y;
        let accent = style::accent(ui);
        let text_color = ui.visuals().text_color();
        let weak = ui.visuals().weak_text_color();

        // Name on the left, "Raw <value>" centered over the bars; "With center" starts
        // where the button column starts.
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, h), Sense::hover());
            let painter = ui.painter();
            let body = egui::TextStyle::Body.resolve(ui.style());
            painter.text(
                rect.left_center(),
                egui::Align2::LEFT_CENTER,
                i18n.tr(&format!("vjoy-axis-{slot}")),
                egui::FontId::new(body.size, body.family.clone()),
                text_color,
            );
            let raw_text = format!("{}  {raw}", i18n.tr("calibration-raw"));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                raw_text,
                body,
                weak,
            );
            ui.checkbox(&mut cal.with_center, i18n.tr("calibration-with-center"));
        });
        // Raw position with the calibration marks | Reset + Save.
        ui.horizontal(|ui| {
            let raw_t = |v: i32| (v - RAW_MIN) as f32 / (RAW_MAX - RAW_MIN) as f32;
            bar(ui, bar_w, raw_t(raw), Some(cal), accent.gamma_multiply(0.6));
            let reset =
                egui::Button::new(i18n.tr("calibration-reset")).corner_radius(style::RADIUS);
            // Save keeps its natural width (icon + padding); Reset takes the rest of the column.
            let save_w = 34.0_f32.max(h);
            if ui.add_sized([BUTTONS - save_w - gap, h], reset).clicked() {
                *action = Some(Action::Reset);
            }
            let save = egui::Button::new(icon::FLOPPY_DISK).corner_radius(style::RADIUS);
            if ui
                .add_enabled_ui(unsaved, |ui| ui.add_sized([save_w, h], save))
                .inner
                .on_hover_text(i18n.tr("calibration-save"))
                .clicked()
            {
                *action = Some(Action::Save);
            }
        });
        // Calibrated output | Calibrate center.
        ui.horizontal(|ui| {
            bar(ui, bar_w, (cal.apply(raw) + 1.0) / 2.0, None, accent);
            let on = matches!(capture, Some(Capture::Center { .. }));
            let center = egui::Button::new(i18n.tr("calibration-center"))
                .selected(on)
                .corner_radius(style::RADIUS);
            let r = ui
                .add_enabled_ui(cal.with_center, |ui| ui.add_sized([BUTTONS, h], center))
                .inner
                .on_hover_text(i18n.tr("calibration-center-tip"));
            if r.clicked() {
                *action = Some(Action::Center);
            }
        });
        // Values under the bars | Calibrate extrema.
        ui.horizontal(|ui| {
            let (row, _) = ui.allocate_exact_size(Vec2::new(bar_w, h), Sense::hover());
            let w = 72.0;
            let mid = row.center().x;
            let mut fields: Vec<(f32, &mut i32)> = vec![(row.left(), &mut cal.low)];
            if cal.with_center {
                fields.push((mid - w - gap / 2.0, &mut cal.center_low));
                fields.push((mid + gap / 2.0, &mut cal.center_high));
            }
            fields.push((row.right() - w, &mut cal.high));
            for (left, v) in fields {
                let rect = Rect::from_min_size(Pos2::new(left, row.top()), Vec2::new(w, h));
                ui.put(
                    rect,
                    egui::DragValue::new(v).range(RAW_MIN..=RAW_MAX).speed(16.0),
                );
            }
            let on = matches!(capture, Some(Capture::Extrema { .. }));
            let extrema = egui::Button::new(i18n.tr("calibration-extrema"))
                .selected(on)
                .corner_radius(style::RADIUS);
            if ui
                .add_sized([BUTTONS, h], extrema)
                .on_hover_text(i18n.tr("calibration-extrema-tip"))
                .clicked()
            {
                *action = Some(Action::Extrema);
            }
        });
    });
}

/// Horizontal bar filled to `t` (0..1); `range` marks the calibration on the raw scale.
fn bar(ui: &mut egui::Ui, width: f32, t: f32, range: Option<&Calibration>, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4, ui.visuals().extreme_bg_color);
    let fill = Rect::from_min_max(
        rect.min,
        Pos2::new(
            rect.left() + rect.width() * t.clamp(0.0, 1.0),
            rect.bottom(),
        ),
    );
    painter.rect_filled(fill, 4, color);
    if let Some(c) = range {
        let x =
            |v: i32| rect.left() + rect.width() * (v - RAW_MIN) as f32 / (RAW_MAX - RAW_MIN) as f32;
        let marks = if c.with_center {
            vec![c.low, c.center_low, c.center_high, c.high]
        } else {
            vec![c.low, c.high]
        };
        for v in marks {
            let px = x(v);
            painter.line_segment(
                [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
                egui::Stroke::new(2.0, ui.visuals().text_color()),
            );
        }
    }
}
