//! Editor of Gremlin's macro action: repeat settings and the list of steps.

use eframe::egui::{self, RichText};
use gob_core::{Action, InputType, Key, MacroRepeat, MacroStep, MouseButton};
use gob_virtual::{ControlKind, GamepadControl};

use crate::action_editor::{Editor, Op, py_float, vjoy_outputs};
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
            let mut edited = step.clone();
            let mut op = None;
            // Buttons on the right first, so long steps wrap before them; one row high unless
            // the fields wrap.
            let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
            let buttons = egui::Layout::right_to_left(egui::Align::Center);
            ui.allocate_ui_with_layout(row, buttons, |ui| {
                if style::icon_button(ui, true, icon::TRASH, &self.i18n.tr("edit-remove")).clicked()
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
                let fields = egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true);
                ui.with_layout(fields, |ui| {
                    ui.label(RichText::new(step_icon(step)).size(style::ICON_SIZE))
                        .on_hover_text(self.i18n.tr(step_key(step)));
                    self.step_fields(ui, &id, index, &mut edited);
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
        let mut pads: Vec<u32> = steps
            .iter()
            .filter_map(|s| match *s {
                MacroStep::GamepadButton { gamepad, .. }
                | MacroStep::GamepadAxis { gamepad, .. }
                | MacroStep::GamepadDpad { gamepad, .. } => Some(gamepad),
                _ => None,
            })
            .collect();
        pads.sort_unstable();
        pads.dedup();
        for pad in pads {
            self.gamepad_missing(ui, pad);
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
                let field = egui::Id::unique(("macro-key", id, index));
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
                self.vjoy_step(ui, id, index, InputType::Button, device, button);
                press_toggle(ui, self, pressed);
            }
            MacroStep::VjoyAxis {
                device,
                id: axis,
                value,
                relative,
            } => {
                self.vjoy_step(ui, id, index, InputType::Axis, device, axis);
                axis_value(ui, value, -1.0);
                ui.add(style::switch(relative, self.i18n.tr("vjoy-relative")));
            }
            MacroStep::VjoyHat {
                device,
                id: hat,
                direction,
            } => {
                self.vjoy_step(ui, id, index, InputType::Hat, device, hat);
                hat_directions(ui, self, direction);
            }
            MacroStep::GamepadButton {
                gamepad,
                control,
                pressed,
            } => {
                self.gamepad_step(ui, id, index, ControlKind::Button, gamepad, control);
                press_toggle(ui, self, pressed);
            }
            MacroStep::GamepadAxis {
                gamepad,
                control,
                value,
            } => {
                self.gamepad_step(ui, id, index, ControlKind::Stick, gamepad, control);
                let trigger = GamepadControl::parse(control)
                    .is_some_and(|c| c.kind() == ControlKind::Trigger);
                axis_value(ui, value, if trigger { 0.0 } else { -1.0 });
            }
            MacroStep::GamepadDpad { gamepad, direction } => {
                let mut control = GamepadControl::Dpad.name().to_string();
                self.gamepad_step(ui, id, index, ControlKind::Dpad, gamepad, &mut control);
                hat_directions(ui, self, direction);
            }
            MacroStep::Other => {
                ui.label(RichText::new(self.i18n.tr("edit-not-editable")).weak());
            }
        }
    }

    /// Virtual stick and output of type `ty` of a step.
    fn vjoy_step(
        &self,
        ui: &mut egui::Ui,
        id: &str,
        index: usize,
        ty: InputType,
        device: &mut u32,
        output: &mut u32,
    ) {
        let salt = index.to_string();
        if let Some((d, n)) = self.vjoy_target(ui, id, &salt, *device, ty, *output) {
            *device = d;
            // Another device may not have the output: take its first one then.
            let outputs = vjoy_outputs(self.vjoy_devices().iter().find(|x| x.id == d), ty);
            *output = if outputs.contains(&n) {
                n
            } else {
                outputs.first().copied().unwrap_or(1)
            };
        }
    }

    /// Gamepad and control of a step: buttons for `ControlKind::Button`, sticks and triggers
    /// for `ControlKind::Stick`, only the gamepad for the d-pad.
    fn gamepad_step(
        &self,
        ui: &mut egui::Ui,
        id: &str,
        index: usize,
        kind: ControlKind,
        gamepad: &mut u32,
        control: &mut String,
    ) {
        let controls: Vec<GamepadControl> = GamepadControl::ALL
            .into_iter()
            .filter(|c| match kind {
                ControlKind::Button => c.kind() == ControlKind::Button,
                ControlKind::Stick | ControlKind::Trigger => {
                    matches!(c.kind(), ControlKind::Stick | ControlKind::Trigger)
                }
                ControlKind::Dpad => false,
            })
            .collect();
        let salt = index.to_string();
        if let Some((pad, c)) = self.gamepad_target(ui, id, &salt, *gamepad, control, &controls) {
            *gamepad = pad;
            if let Some(c) = c {
                *control = c.name().to_string();
            }
        }
    }

    fn add_step_menu(&mut self, ui: &mut egui::Ui, id: &str, count: usize) {
        // New steps take the first virtual stick and gamepad, and the stick's first axis.
        let devices = self.vjoy_devices();
        let device = devices.first().map_or(1, |d| d.id);
        let axis = vjoy_outputs(devices.first(), InputType::Axis)
            .first()
            .copied()
            .unwrap_or(1);
        let pad = self.gamepad_ids().first().copied().unwrap_or(1);
        let mut add = None;
        ui.menu_button(
            RichText::new(format!(
                "{}  {}",
                icon::PLUS,
                self.i18n.tr("macro-add-step")
            )),
            |ui| {
                let mut choices = vec![
                    MacroStep::Key {
                        scan_code: 0x1E,
                        extended: false,
                        pressed: true,
                    },
                    MacroStep::Pause(0.1),
                    MacroStep::MouseButton {
                        button: MouseButton::Left,
                        pressed: true,
                    },
                    MacroStep::MouseMotion { dx: 0, dy: 0 },
                    MacroStep::VjoyButton {
                        device,
                        id: 1,
                        pressed: true,
                    },
                    MacroStep::VjoyAxis {
                        device,
                        id: axis,
                        value: 0.0,
                        relative: false,
                    },
                    MacroStep::VjoyHat {
                        device,
                        id: 1,
                        direction: None,
                    },
                ];
                if self.max_gamepads > 0 {
                    choices.extend([
                        MacroStep::GamepadButton {
                            gamepad: pad,
                            control: GamepadControl::A.name().into(),
                            pressed: true,
                        },
                        MacroStep::GamepadAxis {
                            gamepad: pad,
                            control: GamepadControl::LeftX.name().into(),
                            value: 0.0,
                        },
                        MacroStep::GamepadDpad {
                            gamepad: pad,
                            direction: None,
                        },
                    ]);
                }
                for step in choices {
                    let text = format!("{}  {}", step_icon(&step), self.i18n.tr(step_key(&step)));
                    if ui.button(text).clicked() {
                        add = Some(step);
                        ui.close();
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

/// Axis value of a step, `low..=1`.
fn axis_value(ui: &mut egui::Ui, value: &mut f64, low: f32) {
    let mut v = (*value as f32).clamp(low, 1.0);
    if ui
        .add(
            style::Spin::new(&mut v)
                .range(low..=1.0)
                .step(0.01)
                .decimals(2),
        )
        .changed()
    {
        *value = f64::from(v);
    }
}

fn hat_directions(ui: &mut egui::Ui, editor: &Editor<'_>, direction: &mut Option<u16>) {
    for d in DIRECTIONS {
        let tip = editor.i18n.tr(hat_key(d));
        if style::icon_toggle(ui, *direction == d, hat_glyph(d), &tip).clicked() {
            *direction = d;
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
        MacroStep::GamepadButton { .. }
        | MacroStep::GamepadAxis { .. }
        | MacroStep::GamepadDpad { .. } => icon::GAME_CONTROLLER,
        MacroStep::Other => icon::PUZZLE_PIECE,
    }
}

/// Name of a step's type, as in the add menu.
fn step_key(step: &MacroStep) -> &'static str {
    match step {
        MacroStep::Pause(_) => "macro-step-pause",
        MacroStep::Key { .. } => "macro-step-key",
        MacroStep::MouseButton { .. } => "macro-step-mouse-button",
        MacroStep::MouseMotion { .. } => "macro-step-mouse-motion",
        MacroStep::VjoyButton { .. } => "macro-step-vjoy-button",
        MacroStep::VjoyAxis { .. } => "macro-step-vjoy-axis",
        MacroStep::VjoyHat { .. } => "macro-step-vjoy-hat",
        MacroStep::GamepadButton { .. } => "macro-step-pad-button",
        MacroStep::GamepadAxis { .. } => "macro-step-pad-axis",
        MacroStep::GamepadDpad { .. } => "macro-step-pad-dpad",
        MacroStep::Other => "edit-not-editable",
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
