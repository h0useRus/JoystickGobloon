//! Editor of Gremlin's condition action: all/any, the list of conditions and the
//! true/false action lists.

use eframe::egui::{self, RichText};
use gob_core::{Action, Comparator, Condition, ConditionSource, InputType};

use crate::action_editor::{Editor, Op};
use crate::devices_view::hat_glyph;
use crate::style::{self, icon};

/// Hat directions offered by the direction comparator, center last.
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

const TYPES: [(InputType, &str, &str); 3] = [
    (
        InputType::Axis,
        icon::ARROWS_OUT_LINE_HORIZONTAL,
        "input-axis-kind",
    ),
    (InputType::Button, icon::CIRCLE, "input-button-kind"),
    (InputType::Hat, icon::CROSSHAIR, "input-hat-kind"),
];

impl Editor<'_> {
    pub(crate) fn condition(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        all: bool,
        conditions: &[Condition],
        depth: usize,
    ) {
        let id = action.id.clone();
        ui.horizontal(|ui| {
            for (value, glyph, key) in [
                (true, icon::INTERSECT, "edit-cond-all"),
                (false, icon::UNITE, "edit-cond-any"),
            ] {
                if style::icon_toggle(ui, all == value, glyph, &self.i18n.tr(key)).clicked()
                    && all != value
                {
                    let op = if value { "all" } else { "any" };
                    self.set(&id, "logical-operator", "string", op);
                }
            }
        });
        for (index, cond) in conditions.iter().enumerate() {
            let frame = egui::Frame::new()
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(style::RADIUS)
                .inner_margin(6);
            frame.show(ui, |ui| {
                let mut edited = cond.clone();
                let mut remove = false;
                ui.horizontal_wrapped(|ui| {
                    self.source_editor(ui, &id, index, &mut edited);
                    ui.separator();
                    comparator_editor(ui, self, &id, index, &mut edited.comparator);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        remove =
                            style::icon_button(ui, true, icon::TRASH, &self.i18n.tr("edit-remove"))
                                .clicked();
                    });
                });
                if remove {
                    self.ops.push(Op::RemoveCondition {
                        action: id.clone(),
                        index,
                    });
                } else if edited != *cond {
                    self.ops.push(Op::SetCondition {
                        action: id.clone(),
                        index,
                        condition: edited,
                    });
                }
            });
        }
        self.add_condition_menu(ui, &id, conditions.len());
        let behavior = self.behavior;
        self.nested(
            ui,
            &id,
            &[
                ("true-actions", icon::CHECK_CIRCLE, "edit-cond-true"),
                ("false-actions", icon::X_CIRCLE, "edit-cond-false"),
            ],
            behavior,
            depth,
        );
    }

    fn add_condition_menu(&mut self, ui: &mut egui::Ui, id: &str, count: usize) {
        let mut add = None;
        ui.menu_button(
            RichText::new(format!("{}  {}", icon::PLUS, self.i18n.tr("edit-cond-add"))),
            |ui| {
                let device = self.input.map(|(d, _)| d.to_string()).unwrap_or_default();
                let choices = [
                    (
                        icon::CURSOR_CLICK,
                        "cond-current",
                        ConditionSource::CurrentInput,
                        self.behavior,
                    ),
                    (
                        icon::JOYSTICK,
                        "cond-joystick",
                        ConditionSource::Joystick(vec![(device, InputType::Button, 1)]),
                        InputType::Button,
                    ),
                    (
                        icon::GAME_CONTROLLER,
                        "cond-vjoy",
                        ConditionSource::Vjoy {
                            device: 1,
                            input_type: InputType::Button,
                            id: 1,
                        },
                        InputType::Button,
                    ),
                ];
                for (glyph, key, source, ty) in choices {
                    if ui
                        .button(format!("{glyph}  {}", self.i18n.tr(key)))
                        .clicked()
                    {
                        add = Some(Condition {
                            source,
                            comparator: Condition::default_comparator(ty),
                        });
                        ui.close_menu();
                    }
                }
            },
        );
        if let Some(condition) = add {
            self.ops.push(Op::SetCondition {
                action: id.to_string(),
                index: count,
                condition,
            });
        }
    }

    /// What the condition reads; changing an input type resets the comparator.
    fn source_editor(&mut self, ui: &mut egui::Ui, id: &str, index: usize, cond: &mut Condition) {
        let salt = (id.to_string(), index);
        match &mut cond.source {
            ConditionSource::CurrentInput => {
                ui.label(format!(
                    "{}  {}",
                    icon::CURSOR_CLICK,
                    self.i18n.tr("cond-current")
                ));
            }
            ConditionSource::Joystick(inputs) => {
                ui.label(RichText::new(icon::JOYSTICK).size(style::ICON_SIZE))
                    .on_hover_text(self.i18n.tr("cond-joystick"));
                let devices = self.device_choices();
                let mut new_type = None;
                for (n, (device, ty, input)) in inputs.iter_mut().enumerate() {
                    let name = devices
                        .iter()
                        .find(|(g, _)| g.eq_ignore_ascii_case(device))
                        .map_or_else(|| self.i18n.tr("edit-merge-no-device"), |(_, n)| n.clone());
                    egui::ComboBox::from_id_salt(("cond-dev", &salt, n))
                        .selected_text(name)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for (g, label) in &devices {
                                ui.selectable_value(device, g.clone(), label);
                            }
                        });
                    if n == 0 {
                        if let Some(t) = type_combo(ui, self, ("cond-type", &salt), *ty) {
                            new_type = Some(t);
                        }
                    }
                    ui.add(egui::DragValue::new(input).range(1..=128).prefix("#"));
                }
                if let Some(t) = new_type {
                    for (_, ty, _) in inputs.iter_mut() {
                        *ty = t;
                    }
                    cond.comparator = Condition::default_comparator(t);
                }
            }
            ConditionSource::Vjoy {
                device,
                input_type,
                id: input,
            } => {
                ui.label(RichText::new(icon::GAME_CONTROLLER).size(style::ICON_SIZE))
                    .on_hover_text(self.i18n.tr("cond-vjoy"));
                ui.add(egui::DragValue::new(device).range(1..=16).prefix("vJoy "));
                if let Some(t) = type_combo(ui, self, ("cond-type", &salt), *input_type) {
                    *input_type = t;
                    cond.comparator = Condition::default_comparator(t);
                }
                ui.add(egui::DragValue::new(input).range(1..=128).prefix("#"));
            }
            ConditionSource::Keyboard(keys) => {
                let text = keys
                    .iter()
                    .map(|k| format!("{:#04x}{}", k.scan_code, if k.extended { "e" } else { "" }))
                    .collect::<Vec<_>>()
                    .join(" + ");
                ui.label(format!("{}  {text}", icon::KEYBOARD))
                    .on_hover_text(self.i18n.tr("cond-keyboard"));
            }
            ConditionSource::Other(kind) => {
                ui.label(format!("{}  {kind}", icon::PUZZLE_PIECE))
                    .on_hover_text(self.i18n.tr("edit-not-editable"));
            }
        }
    }
}

fn type_combo(
    ui: &mut egui::Ui,
    editor: &Editor<'_>,
    salt: impl std::hash::Hash,
    current: InputType,
) -> Option<InputType> {
    let mut picked = None;
    let caption = |t: InputType| {
        TYPES
            .iter()
            .find(|(x, ..)| *x == t)
            .map_or(String::new(), |(_, g, k)| {
                format!("{g}  {}", editor.i18n.tr(k))
            })
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(caption(current))
        .show_ui(ui, |ui| {
            for (t, ..) in TYPES {
                if ui.selectable_label(t == current, caption(t)).clicked() && t != current {
                    picked = Some(t);
                }
            }
        });
    picked
}

fn comparator_editor(
    ui: &mut egui::Ui,
    editor: &Editor<'_>,
    id: &str,
    index: usize,
    comparator: &mut Comparator,
) {
    match comparator {
        Comparator::Range { lower, upper } => {
            ui.label(RichText::new(icon::ARROWS_OUT_LINE_HORIZONTAL).size(style::ICON_SIZE))
                .on_hover_text(editor.i18n.tr("cond-range"));
            ui.add(
                egui::DragValue::new(lower)
                    .range(-1.0..=1.0)
                    .speed(0.01)
                    .fixed_decimals(2),
            );
            ui.label("…");
            ui.add(
                egui::DragValue::new(upper)
                    .range(-1.0..=1.0)
                    .speed(0.01)
                    .fixed_decimals(2),
            );
            if *lower > *upper {
                std::mem::swap(lower, upper);
            }
        }
        Comparator::Pressed(p) => {
            for (value, glyph, key) in [
                (true, icon::ARROW_LINE_DOWN, "cond-pressed"),
                (false, icon::ARROW_LINE_UP, "cond-released"),
            ] {
                if style::icon_toggle(ui, *p == value, glyph, &editor.i18n.tr(key)).clicked() {
                    *p = value;
                }
            }
        }
        Comparator::Direction(dirs) => {
            ui.push_id((id, index, "dirs"), |ui| {
                for d in DIRECTIONS {
                    let on = dirs.contains(&d);
                    let tip = editor.i18n.tr(&format!(
                        "hat-{}",
                        match d {
                            None => "center",
                            Some(0) => "north",
                            Some(4500) => "north-east",
                            Some(9000) => "east",
                            Some(13500) => "south-east",
                            Some(18000) => "south",
                            Some(22500) => "south-west",
                            Some(27000) => "west",
                            _ => "north-west",
                        }
                    ));
                    if style::icon_toggle(ui, on, hat_glyph(d), &tip).clicked() {
                        if on {
                            dirs.retain(|x| *x != d);
                        } else {
                            dirs.push(d);
                        }
                    }
                }
            });
        }
    }
}
