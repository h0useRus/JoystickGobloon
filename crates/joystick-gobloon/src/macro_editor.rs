//! Editor of Gremlin's macro action: repeat settings and the list of steps.

use eframe::egui::{self, RichText};
use gob_core::{Action, Key, MacroRepeat, MacroStep, MouseButton};

use crate::action_editor::{Editor, Op, py_float};
use crate::devices_view::hat_glyph;
use crate::style::{self, icon};

const DIRECTIONS: [Option<u16>; 9] = [
    Some(0),
    Some(4500),
    Some(9000),
    Some(13500),
    Some(18000),
    Some(22500),
    Some(27000),
    Some(31500),
    None,
];

impl Editor<'_> {
    pub(crate) fn macro_editor(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        steps: &[MacroStep],
        repeat: MacroRepeat,
    ) {
        let id = action.id.clone();
        self.repeat_row(ui, &id, repeat);
        for (index, step) in steps.iter().enumerate() {
            let mut edited = *step;
            let mut op = None;
            ui.horizontal(|ui| {
                ui.label(RichText::new(step_icon(step)).size(style::ICON_SIZE));
                self.step_fields(ui, &id, index, &mut edited);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if style::icon_button(ui, true, icon::TRASH, &self.i18n.tr("edit-remove"))
                        .clicked()
                    {
                        op = Some(Op::RemoveMacroStep {
                            action: id.clone(),
                            index,
                        });
                    }
                    let down = index + 1 < steps.len();
                    if style::icon_button(ui, down, icon::CARET_DOWN, &self.i18n.tr("edit-down"))
                        .clicked()
                    {
                        op = Some(Op::SwapMacroSteps {
                            action: id.clone(),
                            index,
                        });
                    }
                    if style::icon_button(ui, index > 0, icon::CARET_UP, &self.i18n.tr("edit-up"))
                        .clicked()
                    {
                        op = Some(Op::SwapMacroSteps {
                            action: id.clone(),
                            index: index - 1,
                        });
                    }
                });
            });
            if op.is_none() && edited != *step {
                op = Some(Op::SetMacroStep {
                    action: id.clone(),
                    index,
                    step: edited,
                });
            }
            self.ops.extend(op);
        }
        self.add_step_menu(ui, &id, steps.len());
    }

    fn repeat_row(&mut self, ui: &mut egui::Ui, id: &str, repeat: MacroRepeat) {
        let (mode, count, delay) = match repeat {
            MacroRepeat::Single => ("Single", None, None),
            MacroRepeat::Count { count, delay } => ("Count", Some(count), Some(delay)),
            MacroRepeat::Hold { delay } => ("Hold", None, Some(delay)),
            MacroRepeat::Toggle { delay } => ("Toggle", None, Some(delay)),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::REPEAT).size(style::ICON_SIZE))
                .on_hover_text(self.i18n.tr("macro-repeat"));
            egui::ComboBox::from_id_salt(("macro-repeat", id))
                .selected_text(
                    self.i18n
                        .tr(&format!("macro-repeat-{}", mode.to_lowercase())),
                )
                .show_ui(ui, |ui| {
                    for m in ["Single", "Count", "Hold", "Toggle"] {
                        let text = self.i18n.tr(&format!("macro-repeat-{}", m.to_lowercase()));
                        if ui.selectable_label(m == mode, text).clicked() && m != mode {
                            self.set(id, "repeat-mode", "string", m);
                        }
                    }
                });
            if let Some(mut n) = count {
                ui.add(style::Spin::new(&mut n).range(1..=1000).suffix(" ×"))
                    .on_hover_text(self.i18n.tr("macro-repeat-count-tip"));
                if Some(n) != count {
                    self.set(id, "repeat-count", "int", n.to_string());
                }
            }
            if let Some(d) = delay {
                ui.label(RichText::new(icon::TIMER).size(style::ICON_SIZE))
                    .on_hover_text(self.i18n.tr("macro-repeat-delay"));
                let mut v = d as f32;
                let drag = style::Spin::new(&mut v)
                    .range(0.0..=60.0)
                    .step(0.01)
                    .suffix(" s");
                if ui.add(drag).changed() {
                    self.set(id, "repeat-delay", "float", py_float(v));
                }
            }
        });
    }

    fn step_fields(&mut self, ui: &mut egui::Ui, id: &str, index: usize, step: &mut MacroStep) {
        match step {
            MacroStep::Pause(d) => {
                let mut v = *d as f32;
                if ui
                    .add(
                        style::Spin::new(&mut v)
                            .range(0.0..=600.0)
                            .step(0.01)
                            .suffix(" s"),
                    )
                    .changed()
                {
                    *d = f64::from(v);
                }
            }
            MacroStep::Key {
                scan_code,
                extended,
                pressed,
            } => {
                let key = Key {
                    scan_code: *scan_code,
                    extended: *extended,
                };
                let field = egui::Id::new(("macro-key", id, index));
                if let Some(keys) = crate::key_capture::keys_field(ui, field, &[key], self.i18n)
                    && let Some(k) = keys.first()
                {
                    *scan_code = k.scan_code;
                    *extended = k.extended;
                }
                press_toggle(ui, self, pressed);
            }
            MacroStep::MouseButton { button, pressed } => {
                egui::ComboBox::from_id_salt(("macro-mouse", id, index))
                    .selected_text(self.i18n.tr(mouse_key(*button)))
                    .show_ui(ui, |ui| {
                        for b in MouseButton::ALL {
                            ui.selectable_value(button, b, self.i18n.tr(mouse_key(b)));
                        }
                    });
                if !matches!(button, MouseButton::WheelUp | MouseButton::WheelDown) {
                    press_toggle(ui, self, pressed);
                }
            }
            MacroStep::MouseMotion { dx, dy } => {
                ui.add(style::Spin::new(dx).range(-2000..=2000).prefix("dx "));
                ui.add(style::Spin::new(dy).range(-2000..=2000).prefix("dy "));
            }
            MacroStep::VjoyButton {
                device,
                id: button,
                pressed,
            } => {
                ui.add(style::Spin::new(device).range(1..=16).prefix("vJoy #"));
                ui.add(style::Spin::new(button).range(1..=128).prefix("#"));
                press_toggle(ui, self, pressed);
            }
            MacroStep::VjoyAxis {
                device,
                id: axis,
                value,
                relative,
            } => {
                ui.add(style::Spin::new(device).range(1..=16).prefix("vJoy #"));
                egui::ComboBox::from_id_salt(("macro-axis", id, index))
                    .selected_text(self.i18n.tr(&format!("vjoy-axis-{axis}")))
                    .show_ui(ui, |ui| {
                        for n in 1..=8 {
                            ui.selectable_value(axis, n, self.i18n.tr(&format!("vjoy-axis-{n}")));
                        }
                    });
                let mut v = *value as f32;
                if ui
                    .add(
                        style::Spin::new(&mut v)
                            .range(-1.0..=1.0)
                            .step(0.01)
                            .decimals(2),
                    )
                    .changed()
                {
                    *value = f64::from(v);
                }
                ui.add(style::switch(relative, self.i18n.tr("vjoy-relative")));
            }
            MacroStep::VjoyHat {
                device,
                id: hat,
                direction,
            } => {
                ui.add(style::Spin::new(device).range(1..=16).prefix("vJoy #"));
                ui.add(style::Spin::new(hat).range(1..=4).prefix("#"));
                for d in DIRECTIONS {
                    let tip = self.i18n.tr(hat_key(d));
                    if style::icon_toggle(ui, *direction == d, hat_glyph(d), &tip).clicked() {
                        *direction = d;
                    }
                }
            }
            MacroStep::Other => {
                ui.label(RichText::new(self.i18n.tr("edit-not-editable")).weak());
            }
        }
    }

    fn add_step_menu(&mut self, ui: &mut egui::Ui, id: &str, count: usize) {
        let mut add = None;
        ui.menu_button(
            RichText::new(format!(
                "{}  {}",
                icon::PLUS,
                self.i18n.tr("macro-add-step")
            )),
            |ui| {
                let choices = [
                    (
                        "macro-step-key",
                        MacroStep::Key {
                            scan_code: 0x1E,
                            extended: false,
                            pressed: true,
                        },
                    ),
                    ("macro-step-pause", MacroStep::Pause(0.1)),
                    (
                        "macro-step-mouse-button",
                        MacroStep::MouseButton {
                            button: MouseButton::Left,
                            pressed: true,
                        },
                    ),
                    (
                        "macro-step-mouse-motion",
                        MacroStep::MouseMotion { dx: 0, dy: 0 },
                    ),
                    (
                        "macro-step-vjoy-button",
                        MacroStep::VjoyButton {
                            device: 1,
                            id: 1,
                            pressed: true,
                        },
                    ),
                    (
                        "macro-step-vjoy-axis",
                        MacroStep::VjoyAxis {
                            device: 1,
                            id: 1,
                            value: 0.0,
                            relative: false,
                        },
                    ),
                    (
                        "macro-step-vjoy-hat",
                        MacroStep::VjoyHat {
                            device: 1,
                            id: 1,
                            direction: None,
                        },
                    ),
                ];
                for (key, step) in choices {
                    let text = format!("{}  {}", step_icon(&step), self.i18n.tr(key));
                    if ui.button(text).clicked() {
                        add = Some(step);
                        ui.close_menu();
                    }
                }
            },
        );
        if let Some(step) = add {
            self.ops.push(Op::SetMacroStep {
                action: id.to_string(),
                index: count,
                step,
            });
        }
    }
}

fn press_toggle(ui: &mut egui::Ui, editor: &Editor<'_>, pressed: &mut bool) {
    for (value, glyph, key) in [
        (true, icon::ARROW_LINE_DOWN, "macro-press"),
        (false, icon::ARROW_LINE_UP, "macro-release"),
    ] {
        if style::icon_toggle(ui, *pressed == value, glyph, &editor.i18n.tr(key)).clicked() {
            *pressed = value;
        }
    }
}

fn step_icon(step: &MacroStep) -> &'static str {
    match step {
        MacroStep::Pause(_) => icon::HOURGLASS,
        MacroStep::Key { .. } => icon::KEYBOARD,
        MacroStep::MouseButton { .. } => icon::MOUSE,
        MacroStep::MouseMotion { .. } => icon::ARROWS_OUT_CARDINAL,
        MacroStep::VjoyButton { .. } | MacroStep::VjoyAxis { .. } | MacroStep::VjoyHat { .. } => {
            icon::JOYSTICK
        }
        MacroStep::Other => icon::PUZZLE_PIECE,
    }
}

fn mouse_key(b: MouseButton) -> &'static str {
    match b {
        MouseButton::Left => "mouse-left",
        MouseButton::Right => "mouse-right",
        MouseButton::Middle => "mouse-middle",
        MouseButton::Forward => "mouse-forward",
        MouseButton::Back => "mouse-back",
        MouseButton::WheelUp => "mouse-wheel-up",
        MouseButton::WheelDown => "mouse-wheel-down",
    }
}

fn hat_key(d: Option<u16>) -> &'static str {
    match d {
        None => "hat-center",
        Some(0) => "hat-north",
        Some(4500) => "hat-north-east",
        Some(9000) => "hat-east",
        Some(13500) => "hat-south-east",
        Some(18000) => "hat-south",
        Some(22500) => "hat-south-west",
        Some(27000) => "hat-west",
        _ => "hat-north-west",
    }
}
