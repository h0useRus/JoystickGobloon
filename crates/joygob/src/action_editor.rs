//! Action tree editor. Widgets read the profile and queue [`Op`]s, which are
//! applied after the frame so drawing never holds a mutable borrow.

use eframe::egui::{self, Align2, FontId, Pos2, RichText, Sense, Stroke, Vec2};
use gob_core::{child_groups, Action, ActionKind, InputType, Item, NewAction, Profile, Property};
use gob_i18n::I18n;

use crate::style::{self, icon};

#[derive(Debug, Clone)]
pub enum Op {
    /// Creates the binding (empty root), optionally with a first action.
    Bind {
        device: String,
        name: String,
        input_type: String,
        input_id: String,
        mode: String,
        add: Option<NewAction>,
    },
    Unbind {
        device: String,
        input_type: String,
        input_id: String,
        mode: String,
    },
    Add {
        parent: String,
        group: String,
        action: NewAction,
    },
    Remove {
        parent: String,
        group: String,
        index: usize,
    },
    Move {
        parent: String,
        group: String,
        from: usize,
        to: usize,
    },
    Set {
        action: String,
        name: String,
        ty: String,
        value: String,
    },
    Unset {
        action: String,
        name: String,
    },
    /// Property inside a group, e.g. a deadzone limit.
    SetInGroup {
        action: String,
        group: String,
        name: String,
        ty: String,
        value: String,
    },
    SetPoints {
        action: String,
        points: Vec<(f32, f32)>,
    },
    SetTargets {
        action: String,
        targets: Vec<String>,
    },
}

pub fn apply(p: &mut Profile, op: Op) {
    match op {
        Op::Bind {
            device,
            name,
            input_type,
            input_id,
            mode,
            add,
        } => {
            if !p.devices.iter().any(|d| d.id.eq_ignore_ascii_case(&device)) {
                p.devices.push(gob_core::Device {
                    id: device.clone(),
                    name,
                });
            }
            let root = p.bind(&device, &input_type, &input_id, &mode);
            if let Some(kind) = add {
                p.add_child(&root, "actions", Action::new(&kind));
            }
        }
        Op::Unbind {
            device,
            input_type,
            input_id,
            mode,
        } => p.unbind(&device, &input_type, &input_id, &mode),
        Op::Add {
            parent,
            group,
            action,
        } => {
            p.add_child(&parent, &group, Action::new(&action));
        }
        Op::Remove {
            parent,
            group,
            index,
        } => p.remove_child(&parent, &group, index),
        Op::Move {
            parent,
            group,
            from,
            to,
        } => p.move_child(&parent, &group, from, to),
        Op::Set {
            action,
            name,
            ty,
            value,
        } => {
            if let Some(a) = p.action_mut(&action) {
                a.set(&name, &ty, value);
            }
        }
        Op::Unset { action, name } => {
            if let Some(a) = p.action_mut(&action) {
                a.unset(&name);
            }
        }
        Op::SetInGroup {
            action,
            group,
            name,
            ty,
            value,
        } => {
            if let Some(a) = p.action_mut(&action) {
                let items = a.group_mut(&group);
                match items.iter_mut().find_map(|i| match i {
                    Item::Property(p) if p.name == name => Some(p),
                    _ => None,
                }) {
                    Some(prop) => prop.value = value,
                    None => items.push(Item::Property(Property { ty, name, value })),
                }
            }
        }
        Op::SetPoints { action, points } => {
            if let Some(a) = p.action_mut(&action) {
                *a.group_mut("control-points") = points
                    .iter()
                    .map(|&(x, y)| {
                        Item::Property(Property {
                            ty: "point2d".into(),
                            name: "point".into(),
                            value: format!("{},{}", py_float(x), py_float(y)),
                        })
                    })
                    .collect();
            }
        }
        Op::SetTargets { action, targets } => {
            if let Some(a) = p.action_mut(&action) {
                a.items
                    .retain(|i| !matches!(i, Item::Group { tag, .. } if tag == "target-mode"));
                let at = a
                    .items
                    .iter()
                    .position(|i| matches!(i, Item::Property(p) if p.name == "change-type"))
                    .map_or(0, |i| i + 1);
                for (n, t) in targets.into_iter().enumerate() {
                    let item = Item::Group {
                        tag: "target-mode".into(),
                        attrs: Vec::new(),
                        items: vec![Item::Property(Property {
                            ty: "string".into(),
                            name: "name".into(),
                            value: t,
                        })],
                    };
                    a.items.insert(at + n, item);
                }
            }
        }
    }
}

/// Float text the way Python prints it (`1.0`, `0.25`), as Gremlin writes values.
pub fn py_float(v: f32) -> String {
    let v = (v * 10_000.0).round() / 10_000.0;
    if v.fract() == 0.0 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

pub fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "map-to-vjoy" => icon::GAME_CONTROLLER,
        "response-curve" => icon::CHART_LINE,
        "change-mode" => icon::STACK,
        "tempo" => icon::TIMER,
        "description" => icon::NOTE,
        "macro" => icon::KEYBOARD,
        "map-to-mouse" => icon::MOUSE,
        _ => icon::PUZZLE_PIECE,
    }
}

/// Action types offered in the "add" menu: (icon, i18n key, template).
fn addable(behavior: InputType, mode: &str) -> Vec<(&'static str, &'static str, NewAction)> {
    let mut list = vec![(
        icon::GAME_CONTROLLER,
        "kind-map-to-vjoy",
        NewAction::MapToVjoy {
            input_type: behavior,
        },
    )];
    if behavior == InputType::Axis {
        list.push((
            icon::CHART_LINE,
            "kind-response-curve",
            NewAction::ResponseCurve,
        ));
    }
    if behavior == InputType::Button {
        list.push((
            icon::STACK,
            "kind-change-mode",
            NewAction::ChangeMode {
                target: mode.into(),
            },
        ));
        list.push((icon::TIMER, "kind-tempo", NewAction::Tempo));
    }
    list.push((icon::NOTE, "kind-description", NewAction::Description));
    list
}

pub struct Editor<'a> {
    pub profile: &'a Profile,
    pub i18n: &'a I18n,
    pub ops: &'a mut Vec<Op>,
    /// How the input is interpreted; decides which actions make sense.
    pub behavior: InputType,
}

impl Editor<'_> {
    /// "Add action" menu button; `make` turns the chosen template into an op.
    pub fn add_menu(&mut self, ui: &mut egui::Ui, make: impl Fn(NewAction) -> Op) {
        let mode = self.profile.modes.first().map_or("", |m| m.name.as_str());
        ui.menu_button(
            RichText::new(icon::PLUS_CIRCLE).size(style::ICON_SIZE),
            |ui| {
                for (glyph, key, kind) in addable(self.behavior, mode) {
                    if ui
                        .button(format!("{glyph}  {}", self.i18n.tr(key)))
                        .clicked()
                    {
                        self.ops.push(make(kind));
                        ui.close_menu();
                    }
                }
            },
        )
        .response
        .on_hover_text(self.i18n.tr("edit-add"));
    }

    /// Children of `parent`'s `group` as editable cards, plus an add button.
    pub fn container(&mut self, ui: &mut egui::Ui, parent: &str, group: &str, depth: usize) {
        let Some(action) = self.profile.action(parent) else {
            return;
        };
        let children = action.children(group);
        for (i, id) in children.iter().enumerate() {
            self.card(ui, parent, group, i, children.len(), id, depth);
        }
        let (parent, group) = (parent.to_string(), group.to_string());
        self.add_menu(ui, |kind| Op::Add {
            parent: parent.clone(),
            group: group.clone(),
            action: kind,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn card(
        &mut self,
        ui: &mut egui::Ui,
        parent: &str,
        group: &str,
        index: usize,
        count: usize,
        id: &str,
        depth: usize,
    ) {
        let Some(action) = self.profile.action(id) else {
            ui.colored_label(ui.visuals().error_fg_color, format!("? {id}"));
            return;
        };
        let frame = egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(style::RADIUS)
            .inner_margin(8);
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(kind_icon(&action.kind)).size(style::ICON_SIZE))
                    .on_hover_text(&action.kind);
                let mut label = action
                    .label()
                    .filter(|l| *l != "None")
                    .unwrap_or_default()
                    .to_string();
                let edit = egui::TextEdit::singleline(&mut label)
                    .hint_text(&action.kind)
                    .desired_width(260.0);
                if ui.add(edit).changed() {
                    self.set(id, "action-label", "string", label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (p, g) = (parent.to_string(), group.to_string());
                    if style::icon_button(ui, true, icon::TRASH, &self.i18n.tr("edit-remove"))
                        .clicked()
                    {
                        self.ops.push(Op::Remove {
                            parent: p.clone(),
                            group: g.clone(),
                            index,
                        });
                    }
                    if style::icon_button(
                        ui,
                        index + 1 < count,
                        icon::CARET_DOWN,
                        &self.i18n.tr("edit-down"),
                    )
                    .clicked()
                    {
                        self.ops.push(Op::Move {
                            parent: p.clone(),
                            group: g.clone(),
                            from: index,
                            to: index + 1,
                        });
                    }
                    if style::icon_button(ui, index > 0, icon::CARET_UP, &self.i18n.tr("edit-up"))
                        .clicked()
                    {
                        self.ops.push(Op::Move {
                            parent: p,
                            group: g,
                            from: index,
                            to: index - 1,
                        });
                    }
                });
            });
            match action.kind() {
                ActionKind::MapToVjoy {
                    device,
                    input_type,
                    input_id,
                    inverted,
                } => self.vjoy(ui, action, device, input_type, input_id, inverted),
                ActionKind::ChangeMode { change_type, .. } => {
                    self.change_mode(ui, action, change_type)
                }
                ActionKind::ResponseCurve {
                    deadzone, points, ..
                } => self.curve(ui, action, deadzone, &points),
                ActionKind::Tempo {
                    threshold,
                    activate_on,
                    ..
                } => self.tempo(ui, action, threshold, activate_on, depth),
                ActionKind::Description { text } => {
                    let mut text = text.to_string();
                    if ui
                        .add(
                            egui::TextEdit::multiline(&mut text)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY),
                        )
                        .changed()
                    {
                        self.set(id, "description", "string", text);
                    }
                }
                ActionKind::Root { .. } => {
                    for g in child_groups("root") {
                        self.container(ui, id, g, depth + 1);
                    }
                }
                ActionKind::Other => {
                    ui.label(RichText::new(self.i18n.tr("edit-not-editable")).weak());
                }
            }
        });
        ui.add_space(4.0);
    }

    fn set(&mut self, action: &str, name: &str, ty: &str, value: impl Into<String>) {
        self.ops.push(Op::Set {
            action: action.into(),
            name: name.into(),
            ty: ty.into(),
            value: value.into(),
        });
    }

    fn activation(&mut self, ui: &mut egui::Ui, action: &Action) {
        let current = action.str("activation-mode").unwrap_or("both");
        for (value, glyph, key) in [
            ("press", icon::ARROW_LINE_DOWN, "edit-on-press"),
            ("release", icon::ARROW_LINE_UP, "edit-on-release"),
            ("both", icon::ARROWS_DOWN_UP, "edit-on-both"),
        ] {
            if style::icon_toggle(ui, current == value, glyph, &self.i18n.tr(key)).clicked() {
                self.set(&action.id, "activation-mode", "activation-mode", value);
            }
        }
    }

    fn vjoy(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        device: u32,
        input_type: InputType,
        input_id: u32,
        inverted: bool,
    ) {
        let id = action.id.clone();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("vJoy").weak());
            let mut dev = device;
            if ui
                .add(egui::DragValue::new(&mut dev).range(1..=16).prefix("#"))
                .changed()
            {
                self.set(&id, "vjoy-device-id", "int", dev.to_string());
            }
            let types = [
                (InputType::Axis, icon::ARROWS_OUT_LINE_HORIZONTAL, "axis"),
                (InputType::Button, icon::RADIO_BUTTON, "button"),
                (InputType::Hat, icon::ARROWS_OUT_CARDINAL, "hat"),
            ];
            for (ty, glyph, name) in types {
                if style::icon_toggle(
                    ui,
                    input_type == ty,
                    glyph,
                    &self.i18n.tr(&format!("input-{name}-kind")),
                )
                .clicked()
                    && input_type != ty
                {
                    self.set(&id, "vjoy-input-type", "input_type", name);
                    // Keep the type-specific properties Gremlin expects.
                    if ty == InputType::Axis {
                        self.ops.push(Op::Unset {
                            action: id.clone(),
                            name: "button-inverted".into(),
                        });
                        self.set(&id, "axis-mode", "axis_mode", "absolute");
                        self.set(&id, "axis-scaling", "float", "1.0");
                    } else {
                        self.ops.push(Op::Unset {
                            action: id.clone(),
                            name: "axis-mode".into(),
                        });
                        self.ops.push(Op::Unset {
                            action: id.clone(),
                            name: "axis-scaling".into(),
                        });
                        if ty == InputType::Button {
                            self.set(&id, "button-inverted", "bool", "False");
                        } else {
                            self.ops.push(Op::Unset {
                                action: id.clone(),
                                name: "button-inverted".into(),
                            });
                        }
                    }
                }
            }
            let mut n = input_id;
            let max = if input_type == InputType::Button {
                128
            } else if input_type == InputType::Axis {
                8
            } else {
                4
            };
            if ui
                .add(egui::DragValue::new(&mut n).range(1..=max))
                .changed()
            {
                self.set(&id, "vjoy-input-id", "int", n.to_string());
            }
            if input_type == InputType::Button {
                let mut inv = inverted;
                if ui
                    .toggle_value(&mut inv, icon::SWAP)
                    .on_hover_text(self.i18n.tr("edit-inverted"))
                    .changed()
                {
                    self.set(
                        &id,
                        "button-inverted",
                        "bool",
                        if inv { "True" } else { "False" },
                    );
                }
            }
            ui.separator();
            self.activation(ui, action);
        });
    }

    fn change_mode(&mut self, ui: &mut egui::Ui, action: &Action, change_type: &str) {
        let id = action.id.clone();
        let targets: Vec<String> = action
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Group { tag, items, .. } if tag == "target-mode" => {
                    items.iter().find_map(|i| match i {
                        Item::Property(p) if p.name == "name" => Some(p.value.clone()),
                        _ => None,
                    })
                }
                _ => None,
            })
            .collect();
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("change-type", &id))
                .selected_text(change_type)
                .show_ui(ui, |ui| {
                    for t in ["Switch", "Temporary", "Cycle", "Previous", "Unwind"] {
                        if ui.selectable_label(change_type == t, t).clicked() {
                            self.set(&id, "change-type", "string", t);
                        }
                    }
                });
            let modes: Vec<String> = self.profile.modes.iter().map(|m| m.name.clone()).collect();
            match change_type {
                "Switch" | "Temporary" => {
                    let current = targets.first().cloned().unwrap_or_default();
                    egui::ComboBox::from_id_salt(("target", &id))
                        .selected_text(&current)
                        .show_ui(ui, |ui| {
                            for m in &modes {
                                if ui.selectable_label(&current == m, m).clicked() {
                                    self.ops.push(Op::SetTargets {
                                        action: id.clone(),
                                        targets: vec![m.clone()],
                                    });
                                }
                            }
                        });
                }
                "Cycle" => {
                    for m in &modes {
                        let mut on = targets.contains(m);
                        if ui.checkbox(&mut on, m).changed() {
                            let next: Vec<String> = modes
                                .iter()
                                .filter(|x| if *x == m { on } else { targets.contains(x) })
                                .cloned()
                                .collect();
                            self.ops.push(Op::SetTargets {
                                action: id.clone(),
                                targets: next,
                            });
                        }
                    }
                }
                _ => {}
            }
            ui.separator();
            self.activation(ui, action);
        });
    }

    fn tempo(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        threshold: f64,
        activate_on: &str,
        depth: usize,
    ) {
        let id = action.id.clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::TIMER).size(style::ICON_SIZE))
                .on_hover_text(self.i18n.tr("edit-threshold"));
            let mut t = threshold;
            if ui
                .add(
                    egui::DragValue::new(&mut t)
                        .range(0.05..=10.0)
                        .speed(0.01)
                        .suffix(" s"),
                )
                .changed()
            {
                self.set(&id, "threshold", "float", py_float(t as f32));
            }
            for (value, glyph, key) in [
                ("press", icon::ARROW_LINE_DOWN, "edit-on-press"),
                ("release", icon::ARROW_LINE_UP, "edit-on-release"),
            ] {
                if style::icon_toggle(ui, activate_on == value, glyph, &self.i18n.tr(key)).clicked()
                {
                    self.set(&id, "activate-on", "string", value);
                }
            }
        });
        if depth > 8 {
            return;
        }
        for (group, glyph, key) in [
            ("short-actions", icon::CURSOR_CLICK, "edit-short"),
            ("long-actions", icon::HAND_TAP, "edit-long"),
        ] {
            ui.label(RichText::new(format!("{glyph}  {}", self.i18n.tr(key))).strong());
            ui.indent((group, &id), |ui| {
                let saved = self.behavior;
                self.behavior = InputType::Button;
                self.container(ui, &id, group, depth + 1);
                self.behavior = saved;
            });
        }
    }

    fn curve(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        deadzone: [f64; 4],
        points: &[(f64, f64)],
    ) {
        let id = action.id.clone();
        let pts: Vec<(f32, f32)> = points.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
        ui.horizontal(|ui| {
            if let Some(new) = curve_widget(ui, &id, &pts, &self.i18n.tr("edit-curve-help")) {
                self.ops.push(Op::SetPoints {
                    action: id.clone(),
                    points: new,
                });
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(self.i18n.tr("edit-deadzone")).weak());
                for (i, name) in ["low", "center-low", "center-high", "high"]
                    .into_iter()
                    .enumerate()
                {
                    let mut v = deadzone[i] as f32;
                    let range = if i < 2 { -1.0..=0.0 } else { 0.0..=1.0 };
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .range(range)
                                .speed(0.005)
                                .fixed_decimals(2),
                        )
                        .changed()
                    {
                        self.ops.push(Op::SetInGroup {
                            action: id.clone(),
                            group: "deadzone".into(),
                            name: name.into(),
                            ty: "float".into(),
                            value: py_float(v),
                        });
                    }
                }
                if ui
                    .button(format!(
                        "{}  {}",
                        icon::FLIP_VERTICAL,
                        self.i18n.tr("edit-invert")
                    ))
                    .clicked()
                {
                    let inverted = pts.iter().map(|&(x, y)| (x, -y)).collect();
                    self.ops.push(Op::SetPoints {
                        action: id.clone(),
                        points: inverted,
                    });
                }
            });
        });
    }
}

/// Drag points to move them, click empty space to invert, double-click to add
/// a point, right-click a point to remove it. End points stay at x = -1 and
/// x = 1. Returns the new points when changed.
fn curve_widget(
    ui: &mut egui::Ui,
    id: &str,
    points: &[(f32, f32)],
    help: &str,
) -> Option<Vec<(f32, f32)>> {
    const SIZE: f32 = 200.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(SIZE), Sense::click_and_drag());
    let response = response.on_hover_text(help);
    let to_screen = |(x, y): (f32, f32)| {
        Pos2::new(
            rect.center().x + x * SIZE / 2.0,
            rect.center().y - y * SIZE / 2.0,
        )
    };
    let to_curve = |p: Pos2| {
        (
            ((p.x - rect.center().x) / (SIZE / 2.0)).clamp(-1.0, 1.0),
            ((rect.center().y - p.y) / (SIZE / 2.0)).clamp(-1.0, 1.0),
        )
    };
    let accent = style::accent(ui);
    let weak = ui.visuals().weak_text_color();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, style::RADIUS, ui.visuals().extreme_bg_color);
    painter.line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, weak.gamma_multiply(0.4)),
    );
    painter.line_segment(
        [rect.center_top(), rect.center_bottom()],
        Stroke::new(1.0, weak.gamma_multiply(0.4)),
    );
    let line: Vec<Pos2> = points.iter().map(|&p| to_screen(p)).collect();
    painter.add(egui::Shape::line(line.clone(), Stroke::new(2.0, accent)));
    for p in &line {
        painter.circle(*p, 5.0, ui.visuals().panel_fill, Stroke::new(2.0, accent));
    }
    painter.text(
        rect.left_top() + Vec2::splat(4.0),
        Align2::LEFT_TOP,
        "1",
        FontId::monospace(10.0),
        weak,
    );

    let drag_id = ui.make_persistent_id(("curve-drag", id));
    let nearest = |pos: Pos2| {
        line.iter()
            .enumerate()
            .map(|(i, p)| (i, p.distance(pos)))
            .filter(|(_, d)| *d < 10.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let mut pts = points.to_vec();
    let last = pts.len().saturating_sub(1);
    if response.drag_started() {
        if let Some(i) = response.interact_pointer_pos().and_then(nearest) {
            ui.data_mut(|d| d.insert_temp(drag_id, i));
        }
    }
    if response.dragged() {
        let dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
        if let (Some(i), Some(pos)) = (dragging, response.interact_pointer_pos()) {
            let (mut x, y) = to_curve(pos);
            x = if i == 0 {
                -1.0
            } else if i == last {
                1.0
            } else {
                x.clamp(pts[i - 1].0 + 0.01, pts[i + 1].0 - 0.01)
            };
            pts[i] = (x, y);
            return Some(pts);
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<usize>(drag_id));
    }
    let on_point = response.interact_pointer_pos().and_then(nearest).is_some();
    let mut changed = false;
    // A click on empty space inverts. A double-click is two clicks, so it inverts
    // twice (no net change) and then adds a point.
    if response.clicked() && !on_point {
        pts.iter_mut().for_each(|p| p.1 = -p.1);
        changed = true;
    }
    if response.double_clicked() && !on_point {
        if let Some(pos) = response.interact_pointer_pos() {
            let p = to_curve(pos);
            if let Some(at) = pts.iter().position(|q| q.0 > p.0).filter(|&at| at > 0) {
                pts.insert(at, p);
            }
        }
    }
    if changed {
        return Some(pts);
    }
    if response.secondary_clicked() {
        if let Some(i) = response
            .interact_pointer_pos()
            .and_then(nearest)
            .filter(|&i| i > 0 && i < last)
        {
            pts.remove(i);
            return Some(pts);
        }
    }
    None
}
