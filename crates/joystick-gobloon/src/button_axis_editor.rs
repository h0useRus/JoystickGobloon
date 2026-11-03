//! Editor of Joystick Gobloon's button-axis action ("axis from buttons"): the axis it moves,
//! the input that moves it down, how the buttons move it and where it starts.

use eframe::egui::{self, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use gob_core::{Action, AxisMotion, AxisOutput, ButtonAxis, InputType};

use crate::action_editor::{Editor, LAST_GAMEPAD, LAST_VJOY_DEVICE, Op, py_float};
use crate::output_marks::OutputMarks;
use crate::style::{self, icon};

/// What the decrease input's listen button waits for: a button or key going down that was
/// not down when it was clicked.
#[derive(Clone, Default)]
struct Listening {
    /// (device GUID, button index) down at the click.
    held: Vec<(String, usize)>,
}

impl Editor<'_> {
    pub(crate) fn button_axis(&mut self, ui: &mut egui::Ui, action: &Action, axis: &ButtonAxis) {
        self.axis_target(ui, action, &axis.output);
        self.decrease_input(ui, action, axis);
        self.motion(ui, action, axis);
        ui.horizontal_wrapped(|ui| self.start_value(ui, action, axis.start));
    }

    /// The kind of axis (virtual stick, gamepad, logical device) and the axis itself.
    fn axis_target(&mut self, ui: &mut egui::Ui, action: &Action, output: &AxisOutput) {
        use gob_virtual::{ControlKind, GamepadControl};
        let id = action.id.clone();
        ui.horizontal_wrapped(|ui| {
            let gamepad = matches!(output, AxisOutput::Gamepad { .. });
            let logical = matches!(output, AxisOutput::Logical { .. });
            let vjoy = !gamepad && !logical;
            if style::icon_toggle(ui, vjoy, icon::JOYSTICK, &self.i18n.tr("bax-to-vjoy")).clicked()
                && !vjoy
            {
                let (device, axis) = self.vjoy_axis_for_new(ui.ctx());
                self.set(&id, "output-type", "string", "vjoy");
                self.set(&id, "vjoy-device-id", "int", device.to_string());
                self.set(&id, "vjoy-input-id", "int", axis.to_string());
            }
            // Gamepads only where the system has them (or the action already writes one).
            if (gamepad || self.max_gamepads > 0)
                && style::icon_toggle(
                    ui,
                    gamepad,
                    icon::GAME_CONTROLLER,
                    &self.i18n.tr("bax-to-gamepad"),
                )
                .clicked()
                && !gamepad
            {
                let (pad, control) = self.gamepad_axis_for_new(ui.ctx());
                self.set(&id, "output-type", "string", "gamepad");
                self.set(&id, "gamepad-id", "int", pad.to_string());
                self.set(&id, "control", "string", control);
            }
            if style::icon_toggle(ui, logical, icon::CPU, &self.i18n.tr("bax-to-logical")).clicked()
                && !logical
            {
                self.ops.push(Op::ButtonAxisLogical(id.clone()));
            }
            ui.separator();
            match output {
                AxisOutput::Vjoy { device, axis } => {
                    if let Some((d, n)) =
                        self.vjoy_target(ui, &id, "", *device, InputType::Axis, *axis)
                    {
                        if d != *device {
                            ui.ctx()
                                .data_mut(|m| m.insert_temp(egui::Id::unique(LAST_VJOY_DEVICE), d));
                            self.set(&id, "vjoy-device-id", "int", d.to_string());
                        }
                        if n != *axis {
                            self.set(&id, "vjoy-input-id", "int", n.to_string());
                        }
                    }
                }
                AxisOutput::Gamepad { gamepad, control } => {
                    let controls: Vec<GamepadControl> = GamepadControl::ALL
                        .into_iter()
                        .filter(|c| matches!(c.kind(), ControlKind::Stick | ControlKind::Trigger))
                        .collect();
                    if let Some((n, c)) =
                        self.gamepad_target(ui, &id, "", *gamepad, control, &controls)
                    {
                        if n != *gamepad {
                            ui.ctx()
                                .data_mut(|m| m.insert_temp(egui::Id::unique(LAST_GAMEPAD), n));
                            self.set(&id, "gamepad-id", "int", n.to_string());
                        }
                        if let Some(c) = c.filter(|c| c.name() != control) {
                            self.set(&id, "control", "string", c.name());
                        }
                    }
                }
                AxisOutput::Logical { id: input } => {
                    if let Some(n) = self.logical_picker(ui, &id, InputType::Axis, *input) {
                        self.set(&id, "logical-id", "int", n.to_string());
                    }
                }
            }
        });
        if let AxisOutput::Gamepad { gamepad, .. } = output {
            self.gamepad_missing(ui, *gamepad);
        }
    }

    /// Axis for a virtual stick output: the device picked last (else the first) and its first
    /// axis nothing writes yet.
    pub(crate) fn vjoy_axis_for_new(&self, ctx: &egui::Context) -> (u32, u32) {
        let devices = self.vjoy_devices();
        let last: Option<u32> = ctx.data(|m| m.get_temp(egui::Id::unique(LAST_VJOY_DEVICE)));
        let Some(device) = last
            .and_then(|l| devices.iter().find(|d| d.id == l))
            .or(devices.first())
        else {
            return (1, 1);
        };
        let used = self.used_vjoy_outputs();
        let axes: Vec<u32> = device.axes.iter().map(|&a| u32::from(a)).collect();
        let free = axes
            .iter()
            .find(|&&n| !used.contains(&(device.id, InputType::Axis, n)))
            .or(axes.first());
        (device.id, free.copied().unwrap_or(1))
    }

    /// Stick axis or trigger for a gamepad output: the gamepad picked last (else the first) and
    /// its first one nothing writes yet.
    fn gamepad_axis_for_new(&self, ctx: &egui::Context) -> (u32, &'static str) {
        use gob_virtual::{ControlKind, GamepadControl};
        let pads = self.gamepad_ids();
        let last: Option<u32> = ctx.data(|m| m.get_temp(egui::Id::unique(LAST_GAMEPAD)));
        let pad = last
            .filter(|l| pads.contains(l))
            .or(pads.first().copied())
            .unwrap_or(1);
        let used = self.profile.output_usage(Some(self.edit_mode()));
        let axes = GamepadControl::ALL
            .into_iter()
            .filter(|c| matches!(c.kind(), ControlKind::Stick | ControlKind::Trigger));
        let mut first = None;
        for c in axes {
            first.get_or_insert(c);
            if !used.contains_key(&OutputMarks::gamepad(pad, c.name())) {
                return (pad, c.name());
            }
        }
        (pad, first.map_or("left-x", |c| c.name()))
    }

    /// The input that moves the axis down: its name, a listen button and a remove button.
    fn decrease_input(&mut self, ui: &mut egui::Ui, action: &Action, axis: &ButtonAxis) {
        let listen_id = egui::Id::unique(("bax-listen", &action.id));
        let listening: Option<Listening> = ui.data(|d| d.get_temp(listen_id));
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(icon::ARROW_FAT_DOWN).size(style::ICON_SIZE))
                .on_hover_text(self.i18n.tr("bax-decrease"));
            match (&listening, &axis.decrease) {
                (Some(_), _) => {
                    ui.label(RichText::new(self.i18n.tr("bax-decrease-press")).weak());
                }
                (None, Some(input)) => {
                    // A click shows the input on the Devices page.
                    let name = RichText::new(self.decrease_name(input)).underline();
                    let r = ui
                        .add(egui::Label::new(name).sense(egui::Sense::click()))
                        .on_hover_text(self.i18n.tr("bax-decrease-show"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if r.clicked() {
                        request_jump(ui.ctx(), &input.device, &input.input_type, input.id);
                    }
                }
                (None, None) => {
                    ui.label(RichText::new(self.i18n.tr("bax-decrease-none")).weak());
                }
            }
            match listening {
                None => {
                    if style::record_button(ui, &self.i18n.tr("bax-decrease-listen")).clicked() {
                        // Keys must not reach a focused text field meanwhile.
                        ui.memory_mut(|m| {
                            if let Some(f) = m.focused() {
                                m.surrender_focus(f);
                            }
                        });
                        let held = self.held_buttons();
                        ui.data_mut(|d| d.insert_temp(listen_id, Listening { held }));
                    }
                    if axis.decrease.is_some()
                        && style::icon_button(
                            ui,
                            true,
                            icon::BACKSPACE,
                            &self.i18n.tr("bax-decrease-clear"),
                        )
                        .clicked()
                    {
                        self.ops.push(Op::SetDecrease {
                            action: action.id.clone(),
                            input: None,
                        });
                    }
                }
                Some(listening) => {
                    let cancel =
                        style::icon_button(ui, true, icon::X, &self.i18n.tr("keys-cancel"))
                            .clicked();
                    let heard = (!cancel).then(|| self.heard(ui, &listening)).flatten();
                    if cancel || heard.is_some() {
                        ui.data_mut(|d| d.remove::<Listening>(listen_id));
                    } else {
                        ui.ctx().request_repaint();
                    }
                    if let Some(input) = heard {
                        self.ops.push(Op::SetDecrease {
                            action: action.id.clone(),
                            input: Some(input),
                        });
                    }
                }
            }
        });
    }

    /// "Stick name: Button 3" or "Keyboard: A".
    fn decrease_name(&self, input: &gob_core::DecreaseInput) -> String {
        if input.input_type == "key" {
            let key = crate::key_capture::name(gob_core::key_from_input_id(input.id), self.i18n);
            return format!("{}: {key}", self.i18n.tr("keyboard-device"));
        }
        let device = self
            .device_choices()
            .into_iter()
            .find(|(g, _)| g.eq_ignore_ascii_case(&input.device))
            .map_or_else(|| self.i18n.tr("edit-merge-no-device"), |(_, n)| n);
        let button = self.i18n.trf("vjoy-button-n", &[("n", input.id.into())]);
        format!("{device}: {button}")
    }

    /// Buttons of the attached controllers that are down now.
    fn held_buttons(&self) -> Vec<(String, usize)> {
        self.inputs
            .iter()
            .filter(|d| !d.info.is_own())
            .flat_map(|d| {
                let guid = &d.info.guid;
                d.buttons
                    .iter()
                    .enumerate()
                    .filter(|(_, down)| **down)
                    .map(move |(n, _)| (guid.clone(), n))
            })
            .collect()
    }

    /// A button or key gone down since listening started (the bound input itself does not
    /// count): (device, name for the device list, input type, id).
    fn heard(&self, ui: &egui::Ui, listening: &Listening) -> Option<(String, String, String, u32)> {
        let bound = self.input;
        for d in self.inputs.iter().filter(|d| !d.info.is_own()) {
            for (n, down) in d.buttons.iter().enumerate() {
                let id = n as u32 + 1;
                let is_bound =
                    bound.is_some_and(|(g, b)| g.eq_ignore_ascii_case(&d.info.guid) && b == id);
                if *down
                    && !is_bound
                    && !listening
                        .held
                        .iter()
                        .any(|(g, b)| *g == d.info.guid && *b == n)
                {
                    let name = d.info.name.trim().to_string();
                    return Some((d.info.guid.clone(), name, "button".into(), id));
                }
            }
        }
        let key = crate::key_capture::key_down(ui)?;
        let id = gob_core::key_input_id(key);
        let is_bound = bound.is_some_and(|(g, b)| g == gob_core::KEYBOARD_GUID && b == id);
        (!is_bound).then(|| {
            (
                gob_core::KEYBOARD_GUID.to_string(),
                self.i18n.tr("keyboard-device"),
                "key".into(),
                id,
            )
        })
    }

    /// How the buttons move the axis, with the settings of that motion.
    fn motion(&mut self, ui: &mut egui::Ui, action: &Action, axis: &ButtonAxis) {
        let i18n = self.i18n;
        let id = action.id.clone();
        let name = |m: AxisMotion| i18n.tr(&format!("bax-{}", m.name()));
        let tip = i18n.tr("bax-motion-tip");
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(icon::PATH).size(style::ICON_SIZE))
                .on_hover_text(&tip);
            let mut picked = None;
            egui::ComboBox::from_id_salt(("bax-motion", &id))
                .selected_text(name(axis.motion))
                .show_ui(ui, |ui| {
                    for m in AxisMotion::ALL {
                        let item = ui
                            .selectable_label(m == axis.motion, name(m))
                            .on_hover_text(i18n.tr(&format!("bax-{}-tip", m.name())));
                        if item.clicked() {
                            picked = Some(m);
                        }
                    }
                })
                .response
                .on_hover_text(&tip);
            if let Some(m) = picked.filter(|&m| m != axis.motion) {
                self.set(&id, "motion", "string", m.name());
            }
            match axis.motion {
                AxisMotion::Set => {
                    self.axis_value(ui, &id, "set-value", "bax-value", axis.value);
                    let mut back = axis.return_on_release;
                    if ui
                        .add(style::switch(&mut back, i18n.tr("bax-return")))
                        .changed()
                    {
                        let v = if back { "True" } else { "False" };
                        self.set(&id, "return-on-release", "bool", v);
                    }
                }
                AxisMotion::Velocity => {
                    self.percent(ui, &id, "speed", "bax-speed", axis.speed, " %/s");
                    self.percent(
                        ui,
                        &id,
                        "acceleration",
                        "bax-acceleration",
                        axis.acceleration,
                        " %/s²",
                    );
                    self.percent(
                        ui,
                        &id,
                        "max-speed",
                        "bax-max-speed",
                        axis.max_speed,
                        " %/s",
                    );
                }
                AxisMotion::Stepped => {}
                AxisMotion::Encoder => {
                    self.percent(ui, &id, "step", "bax-step", axis.step, " %");
                    self.percent(ui, &id, "max-step", "bax-max-step", axis.max_step, " %");
                    ui.horizontal(|ui| {
                        self.caption(ui, "bax-fast-rate");
                        let mut rate = axis.fast_rate;
                        let spin = style::Spin::new(&mut rate)
                            .range(1.0..=1000.0)
                            .step(1.0)
                            .suffix(" /s");
                        if ui.add(spin).changed() {
                            self.set(&id, "fast-rate", "float", py_float(rate));
                        }
                    });
                    self.millis(ui, &id, "spring-ms", "bax-spring", axis.spring_ms, 0);
                }
                AxisMotion::Smooth => {
                    self.percent(ui, &id, "step", "bax-step", axis.step, " %");
                    self.millis(ui, &id, "response-ms", "bax-response", axis.response_ms, 10);
                }
            }
        });
        if axis.motion == AxisMotion::Stepped {
            self.positions(ui, &id, &axis.positions);
        }
    }

    /// Weak caption in front of a field.
    fn caption(&self, ui: &mut egui::Ui, key: &str) {
        ui.label(RichText::new(self.i18n.tr(key)).weak());
    }

    /// An axis value (-1..1) field.
    fn axis_value(&mut self, ui: &mut egui::Ui, id: &str, name: &str, key: &str, value: f32) {
        // A caption stays on the line of its field when the row wraps.
        ui.horizontal(|ui| {
            self.caption(ui, key);
            let mut v = value;
            let spin = style::Spin::new(&mut v)
                .range(-1.0..=1.0)
                .step(0.01)
                .decimals(3);
            if ui.add(spin).changed() {
                self.set(id, name, "float", py_float(v));
            }
        });
    }

    /// A part of the full travel (1.0 = end to end), shown in percent.
    fn percent(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        name: &str,
        key: &str,
        value: f32,
        suffix: &str,
    ) {
        ui.horizontal(|ui| {
            self.caption(ui, key);
            let max = if name == "step" || name == "max-step" {
                100.0
            } else {
                ButtonAxis::MAX_SPEED * 100.0
            };
            let mut v = value * 100.0;
            let spin = style::Spin::new(&mut v)
                .range(0.0..=max)
                .step(0.5)
                .decimals(1)
                .suffix(suffix);
            if ui.add(spin).changed() {
                self.set(id, name, "float", py_float(v / 100.0));
            }
        });
    }

    fn millis(&mut self, ui: &mut egui::Ui, id: &str, name: &str, key: &str, value: u32, min: u32) {
        ui.horizontal(|ui| {
            self.caption(ui, key);
            let mut v = value;
            let spin = style::Spin::new(&mut v)
                .range(min..=ButtonAxis::MAX_MS)
                .step(10.0)
                .suffix(" ms");
            if ui.add(spin).changed() {
                self.set(id, name, "int", v.to_string());
            }
        });
    }

    /// Stepped: the positions on a strip (click adds, drag moves, right click removes) and
    /// a row to spread a number of them evenly.
    fn positions(&mut self, ui: &mut egui::Ui, id: &str, positions: &[f32]) {
        let save = |editor: &mut Self, p: &[f32]| {
            let text = gob_core::button_axis::format_positions(p);
            editor.set(id, "positions", "string", text);
        };
        if let Some(p) = positions_strip(ui, id, positions, &self.i18n.tr("bax-positions-tip")) {
            save(self, &p);
        }
        ui.horizontal_wrapped(|ui| {
            let count_id = egui::Id::unique(("bax-even", id));
            let mut n: usize = ui
                .data(|d| d.get_temp(count_id))
                .unwrap_or(positions.len().max(2));
            ui.add(
                style::Spin::new(&mut n)
                    .range(2..=ButtonAxis::MAX_POSITIONS)
                    .step(1.0),
            )
            .on_hover_text(self.i18n.tr("bax-even-count"));
            ui.data_mut(|d| d.insert_temp(count_id, n));
            if style::icon_button(
                ui,
                true,
                icon::DOTS_THREE_OUTLINE,
                &self.i18n.tr("bax-even"),
            )
            .clicked()
            {
                save(self, &gob_core::button_axis::even_positions(n));
            }
            ui.separator();
            let shown = positions
                .iter()
                .map(|p| format!("{p:.2}"))
                .collect::<Vec<_>>()
                .join("  ");
            ui.label(RichText::new(shown).weak());
        });
    }

    /// Start value: written when the profile starts, or the axis is left where it is.
    fn start_value(&mut self, ui: &mut egui::Ui, action: &Action, start: Option<f32>) {
        let id = action.id.clone();
        let mut on = start.is_some();
        if ui
            .add(style::switch(&mut on, self.i18n.tr("bax-start")))
            .on_hover_text(self.i18n.tr("bax-start-tip"))
            .changed()
        {
            if on {
                self.set(&id, "start-value", "float", "0.0");
            } else {
                self.ops.push(Op::Unset {
                    action: id.clone(),
                    name: "start-value".into(),
                });
            }
        }
        if let Some(v) = start {
            let mut v = v;
            let spin = style::Spin::new(&mut v)
                .range(-1.0..=1.0)
                .step(0.01)
                .decimals(3);
            if ui.add(spin).changed() {
                self.set(&id, "start-value", "float", py_float(v));
            }
        }
    }
}

/// The axis from -1 to 1 with a mark per position. Returns the changed positions.
fn positions_strip(ui: &mut egui::Ui, id: &str, positions: &[f32], help: &str) -> Option<Vec<f32>> {
    let width = ui.available_width().clamp(200.0, 480.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click_and_drag());
    let response = response.on_hover_text(help);
    let pad = 10.0;
    let to_x = |v: f32| rect.left() + pad + (v + 1.0) / 2.0 * (rect.width() - 2.0 * pad);
    let to_v = |x: f32| {
        ((x - rect.left() - pad) / (rect.width() - 2.0 * pad) * 2.0 - 1.0).clamp(-1.0, 1.0)
    };
    let round = |v: f32| (v * 1000.0).round() / 1000.0;
    let accent = style::accent(ui);
    let weak = ui.visuals().weak_text_color();
    let painter = ui.painter_at(rect);
    // The fill of a list box (the motion picker).
    let fill = ui.visuals().widgets.inactive.weak_bg_fill;
    painter.rect_filled(rect, style::RADIUS, fill);
    let y = rect.center().y;
    painter.line_segment(
        [Pos2::new(to_x(-1.0), y), Pos2::new(to_x(1.0), y)],
        Stroke::new(2.0_f32, weak.gamma_multiply(0.5)),
    );
    for t in [-1.0, -0.5, 0.0, 0.5, 1.0] {
        let strength = if t == 0.0 { 0.6 } else { 0.3 };
        let x = to_x(t);
        painter.line_segment(
            [
                Pos2::new(x, rect.top() + 6.0),
                Pos2::new(x, rect.bottom() - 6.0),
            ],
            Stroke::new(1.0_f32, weak.gamma_multiply(strength)),
        );
    }
    for &p in positions {
        let r = Rect::from_center_size(Pos2::new(to_x(p), y), Vec2::new(6.0, 16.0));
        painter.rect_filled(r, 2.0, accent);
    }

    let nearest = |pos: Pos2| {
        positions
            .iter()
            .enumerate()
            .map(|(i, &p)| (i, (to_x(p) - pos.x).abs()))
            .filter(|(_, d)| *d < 8.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let drag_id = egui::Id::unique(("bax-positions-drag", id));
    if response.drag_started()
        && let Some(i) = response.interact_pointer_pos().and_then(nearest)
    {
        ui.data_mut(|d| d.insert_temp(drag_id, i));
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<usize>(drag_id));
    }
    let pointer = response.interact_pointer_pos();
    if response.dragged() {
        let dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
        if let (Some(i), Some(pos)) = (dragging, pointer) {
            // Kept between its neighbours, so the order and the dragged mark stay.
            let lo = if i > 0 {
                positions[i - 1] + 0.001
            } else {
                -1.0
            };
            let hi = positions.get(i + 1).map_or(1.0, |n| n - 0.001);
            // max/min, not clamp: hand-written positions may be closer than the margins.
            let v = round(to_v(pos.x)).max(lo).min(hi);
            if v != positions[i] {
                let mut p = positions.to_vec();
                p[i] = v;
                return Some(p);
            }
        }
        return None;
    }
    let on_mark = pointer.and_then(nearest);
    if response.clicked()
        && on_mark.is_none()
        && positions.len() < ButtonAxis::MAX_POSITIONS
        && let Some(pos) = pointer
    {
        let mut p = positions.to_vec();
        p.push(round(to_v(pos.x)));
        p.sort_by(f32::total_cmp);
        return Some(p);
    }
    if response.secondary_clicked()
        && positions.len() > 2
        && let Some(i) = on_mark
    {
        let mut p = positions.to_vec();
        p.remove(i);
        return Some(p);
    }
    None
}

fn jump_id() -> egui::Id {
    egui::Id::unique("bax-show-decrease")
}

/// Asks the Devices page to show an input (profile device id, input type, id).
pub(crate) fn request_jump(ctx: &egui::Context, device: &str, input_type: &str, id: u32) {
    let to = (device.to_string(), input_type.to_string(), id);
    ctx.data_mut(|d| d.insert_temp(jump_id(), to));
}

/// The input [`request_jump`] asked for, once.
pub(crate) fn take_jump(ctx: &egui::Context) -> Option<(String, String, u32)> {
    ctx.data_mut(|d| d.remove_temp(jump_id()))
}
