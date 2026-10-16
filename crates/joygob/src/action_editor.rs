//! Action tree editor. Widgets read the profile and queue [`Op`]s, which are
//! applied after the frame so drawing never holds a mutable borrow.

use eframe::egui::{self, Align2, FontId, Pos2, RichText, Sense, Stroke, Vec2};
use gob_core::{
    child_groups, Action, ActionKind, InputType, Item, MergeOp, MouseAction, MouseButton,
    NewAction, Profile, Property,
};
use gob_i18n::I18n;

use crate::devices_view::hat_glyph;
use crate::style::{self, icon};

/// Builds an [`AddKind`] from an existing action id.
type MakeAdd = fn(String) -> AddKind;

/// What the add menu puts into a list.
#[derive(Debug, Clone)]
pub enum AddKind {
    New(NewAction),
    /// An existing library action, shared (merge-axis is bound to both of its axes).
    Reuse(String),
    /// A deep copy of an existing action.
    Duplicate(String),
}

fn add_to(p: &mut Profile, parent: &str, group: &str, kind: &AddKind) {
    match kind {
        // Id 0: the first logical input of that type, created when there is none.
        AddKind::New(NewAction::MapToLogicalDevice { input_type, id: 0 }) => {
            let existing = p
                .logical_inputs()
                .into_iter()
                .find(|i| i.input_type == *input_type)
                .map(|i| i.id);
            let id = existing.unwrap_or_else(|| p.add_logical_input(*input_type));
            let kind = NewAction::MapToLogicalDevice {
                input_type: *input_type,
                id,
            };
            p.add_child(parent, group, Action::new(&kind));
        }
        AddKind::New(kind) => {
            p.add_child(parent, group, Action::new(kind));
        }
        AddKind::Reuse(id) => {
            p.add_ref(parent, group, id);
        }
        AddKind::Duplicate(id) => {
            if let Some(copy) = p.duplicate(id) {
                p.add_ref(parent, group, &copy);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Op {
    /// Creates the binding (empty root), optionally with a first action.
    Bind {
        device: String,
        name: String,
        input_type: String,
        input_id: String,
        mode: String,
        add: Option<AddKind>,
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
        action: AddKind,
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
    Unset {
        action: String,
        name: String,
    },
    Set {
        action: String,
        name: String,
        ty: String,
        value: String,
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
    /// Replaces (or appends, at the end index) a condition of a condition action.
    SetCondition {
        action: String,
        index: usize,
        condition: gob_core::Condition,
    },
    RemoveCondition {
        action: String,
        index: usize,
    },
    /// Replaces (or appends, at the end index) a macro step.
    SetMacroStep {
        action: String,
        index: usize,
        step: gob_core::MacroStep,
    },
    RemoveMacroStep {
        action: String,
        index: usize,
    },
    /// Swaps step `index` with the next one.
    SwapMacroSteps {
        action: String,
        index: usize,
    },
    AddLogicalInput(InputType),
    RenameLogicalInput {
        input_type: InputType,
        id: u32,
        label: String,
    },
    RemoveLogicalInput {
        input_type: InputType,
        id: u32,
    },
    AddChainStep(String),
    RemoveChainStep {
        action: String,
        index: usize,
    },
    /// Keys of a map-to-keyboard action.
    SetKeys {
        action: String,
        keys: Vec<gob_core::Key>,
    },
    /// Preview in the editor; not an edit.
    PreviewSound {
        file: String,
        volume: u8,
    },
    /// Preview in the editor; not an edit.
    PreviewSpeech(gob_engine::SpeechRequest),
    /// hat-buttons: 4 or 8 directions.
    HatButtonCount {
        action: String,
        count: u8,
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
            let logical = device.eq_ignore_ascii_case(gob_core::LOGICAL_DEVICE_GUID);
            if !logical && !p.devices.iter().any(|d| d.id.eq_ignore_ascii_case(&device)) {
                p.devices.push(gob_core::Device {
                    id: device.clone(),
                    name,
                });
            }
            let root = p.bind(&device, &input_type, &input_id, &mode);
            if let Some(kind) = add {
                add_to(p, &root, "actions", &kind);
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
        } => add_to(p, &parent, &group, &action),
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
        Op::Unset { action, name } => {
            if let Some(a) = p.action_mut(&action) {
                a.unset(&name);
            }
        }
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
        Op::HatButtonCount { action, count } => p.set_hat_button_count(&action, count),
        Op::SetKeys { action, keys } => p.set_keys(&action, &keys),
        Op::AddChainStep(action) => p.add_chain_step(&action),
        Op::AddLogicalInput(input_type) => {
            p.add_logical_input(input_type);
        }
        Op::RenameLogicalInput {
            input_type,
            id,
            label,
        } => {
            p.rename_logical_input(input_type, id, &label);
        }
        Op::RemoveLogicalInput { input_type, id } => p.remove_logical_input(input_type, id),
        Op::SetMacroStep {
            action,
            index,
            step,
        } => p.set_macro_step(&action, index, &step),
        Op::RemoveMacroStep { action, index } => p.remove_macro_step(&action, index),
        Op::SwapMacroSteps { action, index } => p.swap_macro_steps(&action, index),
        Op::RemoveChainStep { action, index } => p.remove_chain_step(&action, index),
        // Handled by the app before edits are applied.
        Op::PreviewSound { .. } | Op::PreviewSpeech(_) => {}
        Op::SetCondition {
            action,
            index,
            condition,
        } => p.set_condition(&action, index, &condition),
        Op::RemoveCondition { action, index } => p.remove_condition(&action, index),
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

/// Actions that read two physical axes and are usually bound to both of them.
const TWO_AXIS_KINDS: [&str; 2] = ["merge-axis", "dual-axis-deadzone"];

/// Two-axis actions are told apart by their own `label` property.
fn merge_label(a: &Action, i18n: &I18n) -> String {
    match a.str("label").map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => l.to_string(),
        None => format!(
            "{} {}",
            i18n.tr(&format!("kind-{}", a.kind)),
            &a.id[..a.id.len().min(8)]
        ),
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
        "smart-toggle" => icon::TOGGLE_RIGHT,
        "split-axis" => icon::ARROWS_SPLIT,
        "pause-resume" => icon::PLAY_PAUSE,
        "run-command" => icon::TERMINAL_WINDOW,
        "map-to-mouse" => icon::MOUSE,
        "map-to-keyboard" => icon::KEYBOARD,
        "double-tap" => icon::NUMBER_CIRCLE_TWO,
        "chain" => icon::LIST_NUMBERS,
        "hat-buttons" => icon::COMPASS,
        "merge-axis" => icon::ARROWS_IN_SIMPLE,
        "axis-delta" => icon::ARROWS_DOWN_UP,
        "dual-axis-deadzone" => icon::CIRCLE_DASHED,
        "condition" => icon::GIT_BRANCH,
        "load-profile" => icon::FILE_ARROW_UP,
        "map-to-logical-device" => icon::CPU,
        "play-sound" => icon::SPEAKER_HIGH,
        "text-to-speech" => icon::CHAT_TEXT,
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
        list.push((
            icon::NUMBER_CIRCLE_TWO,
            "kind-double-tap",
            NewAction::DoubleTap,
        ));
        list.push((icon::LIST_NUMBERS, "kind-chain", NewAction::Chain));
        list.push((icon::KEYBOARD, "kind-macro", NewAction::Macro));
    }
    if behavior == InputType::Button {
        list.push((
            icon::KEYBOARD,
            "kind-map-to-keyboard",
            NewAction::MapToKeyboard,
        ));
    }
    list.push((
        icon::MOUSE,
        "kind-map-to-mouse",
        NewAction::MapToMouse {
            input_type: behavior,
        },
    ));
    if behavior == InputType::Button {
        list.push((
            icon::TOGGLE_RIGHT,
            "kind-smart-toggle",
            NewAction::SmartToggle,
        ));
        list.push((
            icon::PLAY_PAUSE,
            "kind-pause-resume",
            NewAction::PauseResume,
        ));
        list.push((
            icon::TERMINAL_WINDOW,
            "kind-run-command",
            NewAction::RunCommand,
        ));
        list.push((icon::SPEAKER_HIGH, "kind-play-sound", NewAction::PlaySound));
        list.push((
            icon::CHAT_TEXT,
            "kind-text-to-speech",
            NewAction::TextToSpeech,
        ));
        list.push((
            icon::FILE_ARROW_UP,
            "kind-load-profile",
            NewAction::LoadProfile,
        ));
    }
    if behavior == InputType::Hat {
        list.push((icon::COMPASS, "kind-hat-buttons", NewAction::HatButtons));
    }
    if behavior == InputType::Axis {
        list.push((icon::ARROWS_SPLIT, "kind-split-axis", NewAction::SplitAxis));
        list.push((
            icon::ARROWS_DOWN_UP,
            "kind-axis-delta",
            NewAction::AxisDelta,
        ));
    }
    if behavior != InputType::Other {
        list.push((
            icon::CPU,
            "kind-map-to-logical-device",
            NewAction::MapToLogicalDevice {
                input_type: behavior,
                id: 0,
            },
        ));
    }
    list.push((icon::GIT_BRANCH, "kind-condition", NewAction::Condition));
    list.push((icon::NOTE, "kind-description", NewAction::Description));
    list
}

pub struct Editor<'a> {
    pub profile: &'a Profile,
    pub i18n: &'a I18n,
    pub ops: &'a mut Vec<Op>,
    /// How the input is interpreted; decides which actions make sense.
    pub behavior: InputType,
    /// Configured vJoy devices (empty when vJoy is unavailable).
    pub vjoy: &'a [gob_vjoy::DeviceInfo],
    /// Attached controllers, for actions that name physical inputs.
    pub inputs: &'a [gob_input::DeviceState],
    /// Input being edited (profile device id, input id), for templates that refer to it.
    pub input: Option<(&'a str, u32)>,
}

impl Editor<'_> {
    /// "Add action" menu button; `make` turns the chosen template into an op. `parent`
    /// is the action the list belongs to (`None` for a new binding), to avoid loops.
    pub fn add_menu(
        &mut self,
        ui: &mut egui::Ui,
        parent: Option<&str>,
        make: impl Fn(AddKind) -> Op,
    ) {
        let mode = self.profile.modes.first().map_or("", |m| m.name.as_str());
        ui.menu_button(
            RichText::new(icon::PLUS_CIRCLE).size(style::ICON_SIZE),
            |ui| {
                for (glyph, key, kind) in addable(self.behavior, mode) {
                    if ui
                        .button(format!("{glyph}  {}", self.i18n.tr(key)))
                        .clicked()
                    {
                        self.ops.push(make(AddKind::New(kind)));
                        ui.close_menu();
                    }
                }
                if self.behavior == InputType::Axis {
                    self.merge_menu(ui, &make);
                }
                self.existing_menu(ui, parent, &make);
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
        self.add_menu(ui, Some(&parent), |kind: AddKind| Op::Add {
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
                ActionKind::DoubleTap {
                    threshold,
                    activate_on,
                    ..
                } => self.double_tap(ui, action, threshold, activate_on, depth),
                ActionKind::Chain { chains, timeout } => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon::CLOCK_COUNTDOWN).size(style::ICON_SIZE))
                            .on_hover_text(self.i18n.tr("edit-chain-timeout"));
                        let mut t = timeout;
                        let drag = egui::DragValue::new(&mut t)
                            .range(0.0..=60.0)
                            .speed(0.05)
                            .suffix(" s");
                        if ui.add(drag).changed() {
                            self.set(&action.id, "timeout", "float", py_float(t as f32));
                        }
                    });
                    if depth <= 8 {
                        for n in 0..chains.len() {
                            let group = format!("chain-{n}");
                            let title = self.i18n.trf("edit-chain-step", &[("n", (n + 1).into())]);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{}  {title}", icon::LIST_NUMBERS))
                                        .strong(),
                                );
                                let can = chains.len() > 1;
                                let tip = self.i18n.tr("edit-chain-remove");
                                if style::icon_button(ui, can, icon::TRASH, &tip).clicked() {
                                    self.ops.push(Op::RemoveChainStep {
                                        action: action.id.clone(),
                                        index: n,
                                    });
                                }
                            });
                            ui.indent((&group, &action.id), |ui| {
                                // Steps get button events, whatever the input is.
                                let saved =
                                    std::mem::replace(&mut self.behavior, InputType::Button);
                                self.container(ui, &action.id, &group, depth + 1);
                                self.behavior = saved;
                            });
                        }
                        let text = format!("{}  {}", icon::PLUS, self.i18n.tr("edit-chain-add"));
                        if ui.button(text).clicked() {
                            self.ops.push(Op::AddChainStep(action.id.clone()));
                        }
                    }
                }
                ActionKind::HatButtons { directions } => {
                    let eight = directions.len() == 9;
                    ui.horizontal(|ui| {
                        for (count, glyph, key) in [
                            (4, icon::NUMBER_CIRCLE_FOUR, "edit-hat-four"),
                            (8, icon::NUMBER_CIRCLE_EIGHT, "edit-hat-eight"),
                        ] {
                            let on = eight == (count == 8);
                            if style::icon_toggle(ui, on, glyph, &self.i18n.tr(key)).clicked()
                                && !on
                            {
                                self.ops.push(Op::HatButtonCount {
                                    action: action.id.clone(),
                                    count,
                                });
                            }
                        }
                    });
                    let keys: Vec<String> = directions
                        .iter()
                        .map(|(tag, ..)| format!("hat-{}", tag.to_ascii_lowercase()))
                        .collect();
                    let groups: Vec<(&str, &str, &str)> = directions
                        .iter()
                        .zip(&keys)
                        .map(|((tag, d, _), key)| (*tag, hat_glyph(*d), key.as_str()))
                        .collect();
                    self.nested(ui, &action.id, &groups, InputType::Button, depth);
                }
                ActionKind::Condition {
                    all, conditions, ..
                } => self.condition(ui, action, all, &conditions, depth),
                ActionKind::MapToLogicalDevice {
                    input_type,
                    id,
                    inverted,
                    relative,
                } => self.logical_target(ui, action, input_type, id, inverted, relative),
                ActionKind::LoadProfile { file } => {
                    let filter = ("file-filter-gremlin", &["xml"][..]);
                    self.file_field(ui, &action.id, "load-profile", file, Some(filter));
                    self.activation(ui, action);
                }
                ActionKind::PlaySound { file, volume } => {
                    let filter = ("file-filter-sound", &["wav", "mp3", "ogg", "flac"][..]);
                    self.file_field(ui, &action.id, "filename", file, Some(filter));
                    ui.horizontal(|ui| {
                        let can = std::path::Path::new(file).is_file();
                        if style::icon_button(ui, can, icon::PLAY, &self.i18n.tr("edit-preview"))
                            .clicked()
                        {
                            self.ops.push(Op::PreviewSound {
                                file: file.to_string(),
                                volume,
                            });
                        }
                        ui.label(RichText::new(icon::SPEAKER_HIGH).size(style::ICON_SIZE))
                            .on_hover_text(self.i18n.tr("edit-volume"));
                        let mut v = volume;
                        if ui.add(egui::Slider::new(&mut v, 0..=100)).changed() {
                            self.set(&action.id, "volume", "int", v.to_string());
                        }
                        ui.separator();
                        self.activation(ui, action);
                    });
                }
                ActionKind::TextToSpeech {
                    text,
                    voice,
                    queue_mode,
                    rate,
                    volume,
                    pitch,
                } => self.speech(ui, action, text, voice, queue_mode, [rate, volume, pitch]),
                ActionKind::DualAxisDeadzone {
                    axis1,
                    axis2,
                    inner,
                    outer,
                    ..
                } => {
                    let id = action.id.clone();
                    ui.horizontal(|ui| {
                        ui.label(self.i18n.tr("edit-merge-name"));
                        let mut label = action.str("label").unwrap_or_default().to_string();
                        let edit = egui::TextEdit::singleline(&mut label).desired_width(180.0);
                        if ui.add(edit).changed() {
                            self.set(&id, "label", "string", label);
                        }
                    });
                    ui.horizontal(|ui| {
                        for (name, value, glyph, key) in [
                            (
                                "inner-deadzone",
                                inner,
                                icon::CIRCLE_DASHED,
                                "edit-dual-inner",
                            ),
                            ("outer-deadzone", outer, icon::SQUARE, "edit-dual-outer"),
                        ] {
                            ui.label(RichText::new(glyph).size(style::ICON_SIZE))
                                .on_hover_text(self.i18n.tr(key));
                            let mut v = value as f32;
                            let drag = egui::DragValue::new(&mut v)
                                .range(0.0..=1.0)
                                .speed(0.005)
                                .fixed_decimals(3);
                            if ui.add(drag).changed() {
                                self.set(&id, name, "float", py_float(v));
                            }
                        }
                    });
                    if (outer - inner).abs() < 0.01 {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            self.i18n.tr("edit-dual-close"),
                        );
                    }
                    self.axis_pickers(ui, &id, [axis1, axis2]);
                    self.nested(
                        ui,
                        &action.id,
                        &[
                            (
                                "output1-actions",
                                icon::NUMBER_CIRCLE_ONE,
                                "edit-dual-first",
                            ),
                            (
                                "output2-actions",
                                icon::NUMBER_CIRCLE_TWO,
                                "edit-dual-second",
                            ),
                        ],
                        InputType::Axis,
                        depth,
                    );
                }
                ActionKind::AxisDelta { threshold, .. } => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon::RULER).size(style::ICON_SIZE))
                            .on_hover_text(self.i18n.tr("edit-delta-threshold"));
                        let mut t = threshold as f32;
                        let drag = egui::DragValue::new(&mut t)
                            .range(0.0001..=2.0)
                            .speed(0.005)
                            .fixed_decimals(3);
                        if ui.add(drag).changed() {
                            self.set(&action.id, "threshold", "float", py_float(t));
                        }
                    });
                    self.nested(
                        ui,
                        &action.id,
                        &[
                            (
                                "positive-actions",
                                icon::ARROW_FAT_UP,
                                "edit-delta-positive",
                            ),
                            (
                                "negative-actions",
                                icon::ARROW_FAT_DOWN,
                                "edit-delta-negative",
                            ),
                        ],
                        InputType::Button,
                        depth,
                    );
                }
                ActionKind::MergeAxis {
                    axis1,
                    axis2,
                    operation,
                    ..
                } => {
                    self.merge_axis(ui, action, [axis1, axis2], operation);
                    self.nested(
                        ui,
                        &action.id,
                        &[("actions", icon::ARROWS_IN_SIMPLE, "edit-merged")],
                        InputType::Axis,
                        depth,
                    );
                }
                ActionKind::MapToMouse(mouse) => self.mouse(ui, action, mouse),
                ActionKind::SmartToggle { delay, .. } => {
                    self.seconds(ui, &action.id, "delay", "edit-delay", delay);
                    self.nested(
                        ui,
                        &action.id,
                        &[("actions", icon::TOGGLE_RIGHT, "kind-smart-toggle")],
                        InputType::Button,
                        depth,
                    );
                }
                ActionKind::SplitAxis { split, .. } => {
                    ui.horizontal(|ui| {
                        ui.label(self.i18n.tr("edit-split"));
                        let mut v = split as f32;
                        let drag = egui::DragValue::new(&mut v)
                            .range(-1.0..=1.0)
                            .speed(0.005)
                            .fixed_decimals(3);
                        if ui.add(drag).changed() {
                            self.set(&action.id, "split-value", "float", py_float(v));
                        }
                    });
                    self.nested(
                        ui,
                        &action.id,
                        &[
                            ("lower-actions", icon::ARROW_LINE_DOWN, "edit-lower"),
                            ("upper-actions", icon::ARROW_LINE_UP, "edit-upper"),
                        ],
                        InputType::Axis,
                        depth,
                    );
                }
                ActionKind::PauseResume { operation } => {
                    ui.horizontal(|ui| {
                        for op in ["Pause", "Resume", "Toggle"] {
                            if ui
                                .radio(
                                    operation.trim() == op,
                                    self.i18n.tr(&format!("pause-op-{}", op.to_lowercase())),
                                )
                                .clicked()
                            {
                                self.set(&action.id, "operation", "string", op);
                            }
                        }
                        ui.separator();
                        self.activation(ui, action);
                    });
                }
                ActionKind::RunCommand {
                    executable,
                    arguments,
                } => {
                    egui::Grid::new(("run", &action.id))
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(self.i18n.tr("run-executable"));
                            ui.horizontal(|ui| {
                                let mut exe = executable.to_string();
                                if ui
                                    .add(egui::TextEdit::singleline(&mut exe).desired_width(320.0))
                                    .changed()
                                {
                                    self.set(&action.id, "executable", "string", exe);
                                }
                                if style::icon_button(
                                    ui,
                                    true,
                                    icon::FOLDER_OPEN,
                                    &self.i18n.tr("run-browse"),
                                )
                                .clicked()
                                {
                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        self.set(
                                            &action.id,
                                            "executable",
                                            "string",
                                            path.display().to_string(),
                                        );
                                    }
                                }
                            });
                            ui.end_row();
                            ui.label(self.i18n.tr("run-arguments"));
                            let mut args = arguments.to_string();
                            if ui
                                .add(egui::TextEdit::singleline(&mut args).desired_width(320.0))
                                .changed()
                            {
                                self.set(&action.id, "arguments", "string", args);
                            }
                            ui.end_row();
                        });
                    ui.horizontal(|ui| self.activation(ui, action));
                }
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
                ActionKind::MapToKeyboard { keys } => {
                    let id = egui::Id::new(("keys", &action.id));
                    if let Some(keys) = crate::key_capture::keys_field(ui, id, &keys, self.i18n) {
                        self.ops.push(Op::SetKeys {
                            action: action.id.clone(),
                            keys,
                        });
                    }
                    ui.horizontal(|ui| self.activation(ui, action));
                }
                ActionKind::Macro { steps, repeat, .. } => {
                    self.macro_editor(ui, action, &steps, repeat);
                    ui.horizontal(|ui| self.activation(ui, action));
                }
                ActionKind::Other => {
                    ui.label(RichText::new(self.i18n.tr("edit-not-editable")).weak());
                }
            }
        });
        ui.add_space(4.0);
    }

    pub(crate) fn set(&mut self, action: &str, name: &str, ty: &str, value: impl Into<String>) {
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
        // Configured vJoy devices; without vJoy, offer the ids a profile may use.
        let devices: Vec<gob_vjoy::DeviceInfo> = if self.vjoy.is_empty() {
            (1..=gob_vjoy::MAX_DEVICES)
                .map(|n| gob_vjoy::DeviceInfo {
                    id: n,
                    status: gob_vjoy::Status::Unknown,
                    axes: (1..=8).collect(),
                    buttons: 128,
                    cont_hats: 4,
                    disc_hats: 0,
                })
                .collect()
        } else {
            self.vjoy.to_vec()
        };
        let ty = if input_type == InputType::Other {
            self.behavior
        } else {
            input_type
        };
        let current = devices.iter().find(|d| d.id == device);
        let inputs: Vec<u32> = match (ty, current) {
            (InputType::Axis, Some(d)) => d.axes.iter().map(|&a| u32::from(a)).collect(),
            (InputType::Axis, None) => (1..=8).collect(),
            (InputType::Hat, Some(d)) => (1..=d.hats().max(1)).collect(),
            (InputType::Hat, None) => (1..=4).collect(),
            (_, Some(d)) => (1..=d.buttons.max(1)).collect(),
            (_, None) => (1..=128).collect(),
        };
        let name = |n: u32| match ty {
            InputType::Axis => self.i18n.tr(&format!("vjoy-axis-{n}")),
            InputType::Hat => self.i18n.trf("vjoy-hat-n", &[("n", n.into())]),
            _ => self.i18n.trf("vjoy-button-n", &[("n", n.into())]),
        };
        let device_name = |n: u32| self.i18n.trf("vjoy-device-n", &[("n", n.into())]);

        let mut set_device = None;
        let mut set_input = None;
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("vjoy-device", &id))
                .selected_text(device_name(device))
                .show_ui(ui, |ui| {
                    for d in &devices {
                        if ui
                            .selectable_label(d.id == device, device_name(d.id))
                            .clicked()
                        {
                            set_device = Some(d.id);
                        }
                    }
                });
            egui::ComboBox::from_id_salt(("vjoy-input", &id))
                .selected_text(name(input_id))
                .height(320.0)
                .show_ui(ui, |ui| {
                    for &n in &inputs {
                        if ui.selectable_label(n == input_id, name(n)).clicked() {
                            set_input = Some(n);
                        }
                    }
                });
        });
        if let Some(d) = set_device {
            self.set(&id, "vjoy-device-id", "int", d.to_string());
        }
        if let Some(n) = set_input {
            self.set(&id, "vjoy-input-id", "int", n.to_string());
        }

        ui.horizontal_wrapped(|ui| match ty {
            InputType::Axis => {
                let relative = action.str("axis-mode") == Some("relative");
                if ui.radio(!relative, self.i18n.tr("vjoy-absolute")).clicked() && relative {
                    self.set(&id, "axis-mode", "axis_mode", "absolute");
                }
                if ui.radio(relative, self.i18n.tr("vjoy-relative")).clicked() && !relative {
                    self.set(&id, "axis-mode", "axis_mode", "relative");
                }
                if relative {
                    let mut speed: f32 = action
                        .str("axis-scaling")
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(1.0);
                    ui.label(RichText::new(self.i18n.tr("vjoy-speed")).weak());
                    let drag = egui::DragValue::new(&mut speed)
                        .range(0.1..=50.0)
                        .speed(0.05)
                        .fixed_decimals(2);
                    if ui.add(drag).changed() {
                        self.set(&id, "axis-scaling", "float", py_float(speed));
                    }
                }
            }
            _ => {
                if ty == InputType::Button {
                    let mut inv = inverted;
                    if ui
                        .checkbox(&mut inv, self.i18n.tr("edit-inverted"))
                        .changed()
                    {
                        self.set(
                            &id,
                            "button-inverted",
                            "bool",
                            if inv { "True" } else { "False" },
                        );
                    }
                    ui.separator();
                }
                self.activation(ui, action);
            }
        });
    }

    fn mouse(&mut self, ui: &mut egui::Ui, action: &Action, mouse: MouseAction) {
        let id = action.id.clone();
        // Axes and hats always move the cursor; buttons can do either.
        if self.behavior == InputType::Button {
            ui.horizontal(|ui| {
                let motion = matches!(mouse, MouseAction::Motion { .. });
                if ui
                    .radio(!motion, self.i18n.tr("mouse-mode-button"))
                    .clicked()
                    && motion
                {
                    for name in ["direction", "min-speed", "max-speed", "time-to-max-speed"] {
                        self.ops.push(Op::Unset {
                            action: id.clone(),
                            name: name.into(),
                        });
                    }
                    self.set(&id, "mode", "string", "Button");
                    self.set(&id, "button", "string", MouseButton::Left.name());
                }
                if ui
                    .radio(motion, self.i18n.tr("mouse-mode-motion"))
                    .clicked()
                    && !motion
                {
                    self.ops.push(Op::Unset {
                        action: id.clone(),
                        name: "button".into(),
                    });
                    self.set(&id, "mode", "string", "Motion");
                    self.set(&id, "direction", "int", "0");
                    self.set(&id, "min-speed", "int", "50");
                    self.set(&id, "max-speed", "int", "250");
                    self.set(&id, "time-to-max-speed", "float", "1.0");
                }
            });
        }
        match mouse {
            MouseAction::Button(button) => {
                ui.horizontal_wrapped(|ui| {
                    let name = |b: MouseButton| {
                        self.i18n.tr(&format!(
                            "mouse-{}",
                            b.name().to_lowercase().replace(' ', "-")
                        ))
                    };
                    egui::ComboBox::from_id_salt(("mouse-button", &id))
                        .selected_text(name(button))
                        .show_ui(ui, |ui| {
                            for b in MouseButton::ALL {
                                if ui.selectable_label(b == button, name(b)).clicked() {
                                    self.set(&id, "button", "string", b.name());
                                }
                            }
                        });
                    ui.separator();
                    self.activation(ui, action);
                });
            }
            MouseAction::Motion {
                direction,
                min_speed,
                max_speed,
                time_to_max_speed,
            } => {
                egui::Grid::new(("mouse-motion", &id))
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(self.i18n.tr("mouse-direction"));
                        ui.horizontal(|ui| {
                            let mut d = direction.round() as i32;
                            if ui
                                .add(egui::DragValue::new(&mut d).range(0..=359).suffix("°"))
                                .changed()
                            {
                                self.set(&id, "direction", "int", d.to_string());
                            }
                            // Quick headings: up, right, down, left.
                            for (deg, glyph) in [
                                (0, icon::ARROW_UP),
                                (90, icon::ARROW_RIGHT),
                                (180, icon::ARROW_DOWN),
                                (270, icon::ARROW_LEFT),
                            ] {
                                if style::icon_toggle(ui, d == deg, glyph, &format!("{deg}°"))
                                    .clicked()
                                {
                                    self.set(&id, "direction", "int", deg.to_string());
                                }
                            }
                        });
                        ui.end_row();
                        for (key, name, value) in [
                            ("mouse-min-speed", "min-speed", min_speed),
                            ("mouse-max-speed", "max-speed", max_speed),
                        ] {
                            ui.label(self.i18n.tr(key));
                            let mut v = value.round() as i32;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut v)
                                        .range(0..=10_000)
                                        .speed(5)
                                        .suffix(" px/s"),
                                )
                                .changed()
                            {
                                self.set(&id, name, "int", v.to_string());
                            }
                            ui.end_row();
                        }
                        if self.behavior != InputType::Axis {
                            ui.label(self.i18n.tr("mouse-time-to-max"));
                            let mut t = time_to_max_speed;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut t)
                                        .range(0.0..=10.0)
                                        .speed(0.01)
                                        .suffix(" s"),
                                )
                                .changed()
                            {
                                self.set(&id, "time-to-max-speed", "float", py_float(t));
                            }
                            ui.end_row();
                        }
                    });
            }
        }
    }

    fn seconds(&mut self, ui: &mut egui::Ui, id: &str, name: &str, key: &str, value: f64) {
        ui.horizontal(|ui| {
            ui.label(self.i18n.tr(key));
            let mut v = value as f32;
            if ui
                .add(
                    egui::DragValue::new(&mut v)
                        .range(0.05..=10.0)
                        .speed(0.01)
                        .suffix(" s"),
                )
                .changed()
            {
                self.set(id, name, "float", py_float(v));
            }
        });
    }

    /// New merge-axis on the edited input, or one already in the profile: Gremlin binds
    /// a single merge-axis action to both of its axes.
    /// Gremlin's reference action: insert an action that already exists, linked (shared,
    /// edits apply everywhere) or as a copy.
    fn existing_menu(
        &mut self,
        ui: &mut egui::Ui,
        parent: Option<&str>,
        make: &impl Fn(AddKind) -> Op,
    ) {
        let mut candidates: Vec<(String, &Action)> = self
            .profile
            .library
            .iter()
            .filter(|a| a.kind != "root" && parent.is_none_or(|p| !self.profile.reaches(&a.id, p)))
            .map(|a| (crate::profile_view::display_name(a), a))
            .collect();
        if candidates.is_empty() {
            return;
        }
        candidates.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        ui.separator();
        let menus: [(&str, &str, MakeAdd); 2] = [
            (icon::LINK, "edit-link-existing", AddKind::Reuse),
            (icon::COPY, "edit-copy-existing", AddKind::Duplicate),
        ];
        for (glyph, key, kind) in menus {
            ui.menu_button(format!("{glyph}  {}", self.i18n.tr(key)), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for (name, a) in &candidates {
                            if ui.button(name).on_hover_text(&a.kind).clicked() {
                                self.ops.push(make(kind(a.id.clone())));
                                ui.close_menu();
                            }
                        }
                    });
            });
        }
    }

    fn merge_menu(&mut self, ui: &mut egui::Ui, make: &impl Fn(AddKind) -> Op) {
        if let Some((device, axis)) = self.input {
            let device = device.to_string();
            let templates = [
                (
                    "merge-axis",
                    NewAction::MergeAxis {
                        device: device.clone(),
                        axis,
                    },
                ),
                (
                    "dual-axis-deadzone",
                    NewAction::DualAxisDeadzone { device, axis },
                ),
            ];
            for (kind, template) in templates {
                let text = format!(
                    "{}  {}",
                    kind_icon(kind),
                    self.i18n.tr(&format!("kind-{kind}"))
                );
                if ui.button(text).clicked() {
                    self.ops.push(make(AddKind::New(template)));
                    ui.close_menu();
                }
            }
        }
        for a in self
            .profile
            .library
            .iter()
            .filter(|a| TWO_AXIS_KINDS.contains(&a.kind.as_str()))
        {
            let text = format!(
                "{}  {}  {}",
                icon::LINK,
                kind_icon(&a.kind),
                merge_label(a, self.i18n)
            );
            if ui
                .button(text)
                .on_hover_text(self.i18n.tr("edit-reuse"))
                .clicked()
            {
                self.ops.push(make(AddKind::Reuse(a.id.clone())));
                ui.close_menu();
            }
        }
    }

    fn merge_axis(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        axes: [(&str, u32); 2],
        operation: MergeOp,
    ) {
        let id = action.id.clone();
        ui.horizontal(|ui| {
            ui.label(self.i18n.tr("edit-merge-name"));
            let mut label = action.str("label").unwrap_or_default().to_string();
            let edit = egui::TextEdit::singleline(&mut label).desired_width(180.0);
            if ui.add(edit).changed() {
                self.set(&id, "label", "string", label);
            }
            egui::ComboBox::from_id_salt(("merge-op", &id))
                .selected_text(self.i18n.tr(&format!("merge-{}", operation.name())))
                .show_ui(ui, |ui| {
                    for op in MergeOp::ALL {
                        let text = self.i18n.tr(&format!("merge-{}", op.name()));
                        if ui.selectable_label(op == operation, text).clicked() {
                            self.set(&id, "operation", "string", op.name());
                        }
                    }
                });
        });
        self.axis_pickers(ui, &id, axes);
    }

    /// Device and axis dropdowns for `axis1-*` / `axis2-*` (merge-axis, dual-axis-deadzone).
    /// Logical input picker (same type) plus button / axis options.
    fn logical_target(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        input_type: InputType,
        id: u32,
        inverted: bool,
        relative: Option<f64>,
    ) {
        let aid = action.id.clone();
        let inputs: Vec<gob_core::LogicalInput> = self
            .profile
            .logical_inputs()
            .into_iter()
            .filter(|i| i.input_type == input_type)
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(icon::CPU).size(style::ICON_SIZE))
                .on_hover_text(self.i18n.tr("logical-device"));
            let shown = inputs
                .iter()
                .find(|i| i.id == id)
                .map_or_else(|| format!("#{id} ?"), |i| i.label.clone());
            egui::ComboBox::from_id_salt(("logical", &aid))
                .selected_text(shown)
                .width(180.0)
                .show_ui(ui, |ui| {
                    for i in &inputs {
                        if ui.selectable_label(i.id == id, &i.label).clicked() {
                            self.set(&aid, "logical-input-id", "int", i.id.to_string());
                        }
                    }
                });
            match input_type {
                InputType::Button => {
                    let mut inv = inverted;
                    if ui
                        .checkbox(&mut inv, self.i18n.tr("edit-inverted"))
                        .changed()
                    {
                        let v = if inv { "True" } else { "False" };
                        self.set(&aid, "button-inverted", "bool", v);
                    }
                }
                InputType::Axis => {
                    let rel = relative.is_some();
                    if ui.radio(!rel, self.i18n.tr("vjoy-absolute")).clicked() && rel {
                        self.set(&aid, "axis-mode", "axis_mode", "absolute");
                    }
                    if ui.radio(rel, self.i18n.tr("vjoy-relative")).clicked() && !rel {
                        self.set(&aid, "axis-mode", "axis_mode", "relative");
                    }
                    if let Some(speed) = relative {
                        let mut v = speed as f32;
                        let drag = egui::DragValue::new(&mut v)
                            .range(0.1..=50.0)
                            .speed(0.05)
                            .fixed_decimals(2);
                        if ui
                            .add(drag)
                            .on_hover_text(self.i18n.tr("vjoy-speed"))
                            .changed()
                        {
                            self.set(&aid, "axis-scaling", "float", py_float(v));
                        }
                    }
                }
                _ => {}
            }
            ui.separator();
            self.activation(ui, action);
        });
    }

    /// Path text field with a browse button for the string property `name`.
    fn file_field(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        name: &str,
        value: &str,
        filter: Option<(&str, &[&str])>,
    ) {
        ui.horizontal(|ui| {
            let mut path = value.to_string();
            let edit = egui::TextEdit::singleline(&mut path).desired_width(380.0);
            if ui.add(edit).changed() {
                self.set(id, name, "string", path);
            }
            if style::icon_button(ui, true, icon::FOLDER_OPEN, &self.i18n.tr("run-browse"))
                .clicked()
            {
                let mut dialog = rfd::FileDialog::new();
                if let Some((key, ext)) = filter {
                    dialog = dialog.add_filter(self.i18n.tr(key), ext);
                }
                if let Some(path) = dialog.pick_file() {
                    self.set(id, name, "string", path.display().to_string());
                }
            }
            if !value.is_empty() && !std::path::Path::new(value).is_file() {
                ui.label(RichText::new(icon::WARNING).color(ui.visuals().warn_fg_color))
                    .on_hover_text(self.i18n.tr("edit-file-missing"));
            }
        });
    }

    fn speech(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        text: &str,
        voice: Option<&str>,
        queue_mode: &str,
        [rate, volume, pitch]: [f64; 3],
    ) {
        let id = action.id.clone();
        ui.horizontal(|ui| {
            let can = !text.trim().is_empty();
            if style::icon_button(ui, can, icon::PLAY, &self.i18n.tr("edit-preview")).clicked() {
                let mode = self
                    .profile
                    .startup_mode()
                    .unwrap_or_else(|| "Default".into());
                let text = text
                    .replace("${current_mode}", &mode)
                    .replace("$current_mode", &mode);
                self.ops.push(Op::PreviewSpeech(gob_engine::SpeechRequest {
                    text,
                    voice: voice.map(String::from),
                    rate: rate as f32,
                    volume: volume as f32,
                    pitch: pitch as f32,
                    queue: gob_engine::SpeechQueue::Interrupt,
                }));
            }
            ui.label(RichText::new(icon::USER_SOUND).size(style::ICON_SIZE))
                .on_hover_text(self.i18n.tr("edit-tts-voice"));
            let default = self.i18n.tr("edit-tts-default-voice");
            let voices = gob_engine::voices();
            let shown = match voice {
                Some(v) if voices.iter().any(|x| x == v) => v.to_string(),
                Some(v) => format!("{v} ({})", self.i18n.tr("edit-tts-voice-missing")),
                None => default.clone(),
            };
            egui::ComboBox::from_id_salt(("tts-voice", &id))
                .selected_text(shown)
                .width(260.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(voice.is_none(), &default).clicked() && voice.is_some() {
                        self.ops.push(Op::Unset {
                            action: id.clone(),
                            name: "voice".into(),
                        });
                    }
                    for v in voices {
                        if ui.selectable_label(voice == Some(v.as_str()), v).clicked() {
                            self.set(&id, "voice", "string", v.clone());
                        }
                    }
                });
        });
        let mut t = text.to_string();
        let edit = egui::TextEdit::multiline(&mut t)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .hint_text(self.i18n.tr("edit-tts-hint"));
        if ui.add(edit).changed() {
            self.set(&id, "text", "string", t);
        }
        ui.horizontal_wrapped(|ui| {
            for (name, value, range, glyph, key) in [
                (
                    "playback-rate",
                    rate,
                    -1.0..=1.0,
                    icon::GAUGE,
                    "edit-tts-rate",
                ),
                (
                    "playback-volume",
                    volume,
                    0.0..=1.0,
                    icon::SPEAKER_HIGH,
                    "edit-volume",
                ),
                (
                    "playback-pitch",
                    pitch,
                    -1.0..=1.0,
                    icon::WAVE_SINE,
                    "edit-tts-pitch",
                ),
            ] {
                ui.label(RichText::new(glyph).size(style::ICON_SIZE))
                    .on_hover_text(self.i18n.tr(key));
                let mut v = value as f32;
                let drag = egui::DragValue::new(&mut v)
                    .range(range)
                    .speed(0.01)
                    .fixed_decimals(2);
                if ui.add(drag).changed() {
                    self.set(&id, name, "float", py_float(v));
                }
            }
            ui.separator();
            for (mode, glyph, key) in [
                ("queue-back", icon::QUEUE, "edit-tts-back"),
                ("queue-front", icon::ARROW_LINE_UP, "edit-tts-front"),
                ("interrupt", icon::HAND_PALM, "edit-tts-interrupt"),
            ] {
                if style::icon_toggle(ui, queue_mode == mode, glyph, &self.i18n.tr(key)).clicked() {
                    self.set(&id, "queue-mode", "string", mode);
                }
            }
            ui.separator();
            self.activation(ui, action);
        });
    }

    /// (GUID, name) of profile devices, then attached ones the profile does not know yet.
    pub(crate) fn device_choices(&self) -> Vec<(String, String)> {
        let mut devices: Vec<(String, String)> = self
            .profile
            .devices
            .iter()
            .map(|d| (d.id.to_ascii_lowercase(), d.name.trim().to_string()))
            .collect();
        for d in self.inputs.iter().filter(|d| !d.info.is_vjoy()) {
            if !devices.iter().any(|(g, _)| *g == d.info.guid) {
                devices.push((d.info.guid.clone(), d.info.name.trim().to_string()));
            }
        }
        devices
    }

    fn axis_pickers(&mut self, ui: &mut egui::Ui, id: &str, axes: [(&str, u32); 2]) {
        let devices = self.device_choices();
        for (n, (device, axis)) in axes.into_iter().enumerate() {
            let device = device.to_ascii_lowercase();
            let name = devices
                .iter()
                .find(|(g, _)| *g == device)
                .map_or_else(|| self.i18n.tr("edit-merge-no-device"), |(_, n)| n.clone());
            let live = self
                .inputs
                .iter()
                .find(|d| d.info.guid == device || gob_core::same_device_name(&d.info.name, &name));
            let slots: Vec<u32> = live.map_or((1..=8).collect(), |d| {
                d.info.axes.iter().map(|&a| u32::from(a)).collect()
            });
            let prefix = format!("axis{}", n + 1);
            ui.horizontal(|ui| {
                ui.label(RichText::new((n + 1).to_string()).strong());
                egui::ComboBox::from_id_salt((&prefix, "device", id))
                    .selected_text(&name)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (g, label) in &devices {
                            if ui.selectable_label(*g == device, label).clicked() {
                                self.set(id, &format!("{prefix}-guid"), "uuid", g.clone());
                            }
                        }
                    });
                egui::ComboBox::from_id_salt((&prefix, "axis", id))
                    .selected_text(self.i18n.tr(&format!("vjoy-axis-{axis}")))
                    .show_ui(ui, |ui| {
                        for s in slots {
                            let text = self.i18n.tr(&format!("vjoy-axis-{s}"));
                            if ui.selectable_label(s == axis, text).clicked() {
                                self.set(id, &format!("{prefix}-axis"), "int", s.to_string());
                            }
                        }
                    });
            });
        }
    }

    /// Child action lists of a container, each under an icon caption.
    pub(crate) fn nested(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        groups: &[(&str, &str, &str)],
        behavior: InputType,
        depth: usize,
    ) {
        if depth > 8 {
            return;
        }
        for &(group, glyph, key) in groups {
            ui.label(RichText::new(format!("{glyph}  {}", self.i18n.tr(key))).strong());
            ui.indent((group, id), |ui| {
                let saved = std::mem::replace(&mut self.behavior, behavior);
                self.container(ui, id, group, depth + 1);
                self.behavior = saved;
            });
        }
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

    fn double_tap(
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
                .on_hover_text(self.i18n.tr("edit-tap-threshold"));
            let mut t = threshold;
            let drag = egui::DragValue::new(&mut t)
                .range(0.05..=10.0)
                .speed(0.01)
                .suffix(" s");
            if ui.add(drag).changed() {
                self.set(&id, "threshold", "float", py_float(t as f32));
            }
            for (value, glyph, key) in [
                ("exclusive", icon::NUMBER_CIRCLE_ONE, "edit-tap-exclusive"),
                ("combined", icon::INTERSECT, "edit-tap-combined"),
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
            ("single-actions", icon::NUMBER_CIRCLE_ONE, "edit-single"),
            ("double-actions", icon::NUMBER_CIRCLE_TWO, "edit-double"),
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
        let dz = deadzone.map(|d| d as f32);
        let sel_id = ui.make_persistent_id(("curve-selected", &id));
        let mut selected: Option<usize> = ui
            .data(|d| d.get_temp(sel_id))
            .filter(|&i: &usize| i < pts.len());
        // The graph takes what is left next to the point editor, like Gremlin's.
        let size = (ui.available_width() - 200.0).clamp(220.0, 460.0);

        ui.horizontal_top(|ui| {
            let help = self.i18n.tr("edit-curve-help");
            let (new, sel) = curve_widget(ui, &id, &pts, dz, size, selected, &help);
            if sel != selected {
                selected = sel;
                ui.data_mut(|d| d.insert_temp(sel_id, sel));
            }
            if let Some(new) = new {
                self.ops.push(Op::SetPoints {
                    action: id.clone(),
                    points: new,
                });
            }
            ui.vertical(|ui| {
                let invert = format!("{}  {}", icon::FLIP_VERTICAL, self.i18n.tr("edit-invert"));
                if ui.button(invert).clicked() {
                    let inverted = pts.iter().map(|&(x, y)| (x, -y)).collect();
                    self.ops.push(Op::SetPoints {
                        action: id.clone(),
                        points: inverted,
                    });
                }
                ui.add_space(12.0);
                let Some(i) = selected else {
                    ui.label(RichText::new(self.i18n.tr("edit-curve-select")).weak());
                    return;
                };
                let last = pts.len() - 1;
                let (mut x, mut y) = pts[i];
                let (lo, hi) = if i == 0 || i == last {
                    (x, x)
                } else {
                    (pts[i - 1].0 + 0.01, pts[i + 1].0 - 0.01)
                };
                let mut changed = false;
                egui::Grid::new(("curve-point", &id))
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("X");
                        let drag = egui::DragValue::new(&mut x)
                            .range(lo..=hi)
                            .speed(0.005)
                            .fixed_decimals(4);
                        changed |= ui.add_enabled(i != 0 && i != last, drag).changed();
                        ui.end_row();
                        ui.label("Y");
                        let drag = egui::DragValue::new(&mut y)
                            .range(-1.0..=1.0)
                            .speed(0.005)
                            .fixed_decimals(4);
                        changed |= ui.add(drag).changed();
                        ui.end_row();
                    });
                if changed {
                    let mut new = pts.clone();
                    new[i] = (x, y);
                    self.ops.push(Op::SetPoints {
                        action: id.clone(),
                        points: new,
                    });
                }
            });
        });

        ui.label(RichText::new(self.i18n.tr("edit-deadzone")).weak());
        let [mut low, mut center_low, mut center_high, mut high] = dz;
        ui.horizontal(|ui| {
            let slider = ((size - 140.0) / 2.0).max(60.0);
            let field = |ui: &mut egui::Ui, v: &mut f32, range: std::ops::RangeInclusive<f32>| {
                ui.add(
                    egui::DragValue::new(v)
                        .range(range)
                        .speed(0.005)
                        .fixed_decimals(4),
                );
            };
            field(ui, &mut low, -1.0..=center_low);
            range_slider(ui, slider, &mut low, &mut center_low, -1.0, 0.0);
            field(ui, &mut center_low, low..=0.0);
            ui.add_space(8.0);
            field(ui, &mut center_high, 0.0..=high);
            range_slider(ui, slider, &mut center_high, &mut high, 0.0, 1.0);
            field(ui, &mut high, center_high..=1.0);
        });
        let v = [low, center_low, center_high, high];
        let changed: Vec<bool> = v.iter().zip(dz).map(|(a, b)| *a != b).collect();
        for (i, name) in ["low", "center-low", "center-high", "high"]
            .into_iter()
            .enumerate()
        {
            if changed[i] {
                self.ops.push(Op::SetInGroup {
                    action: id.clone(),
                    group: "deadzone".into(),
                    name: name.into(),
                    ty: "float".into(),
                    value: py_float(v[i]),
                });
            }
        }
    }
}

/// Slider with two handles for a `lo..=hi` sub-range of `min..=max`.
fn range_slider(
    ui: &mut egui::Ui,
    width: f32,
    lo: &mut f32,
    hi: &mut f32,
    min: f32,
    max: f32,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::click_and_drag());
    let pad = 8.0;
    let to_x = |v: f32| rect.left() + pad + (v - min) / (max - min) * (rect.width() - 2.0 * pad);
    let to_v = |x: f32| {
        (min + (x - rect.left() - pad) / (rect.width() - 2.0 * pad) * (max - min)).clamp(min, max)
    };
    let handle_id = response.id.with("handle");
    let mut changed = false;
    if response.drag_started() {
        if let Some(p) = response.interact_pointer_pos() {
            let upper = (p.x - to_x(*hi)).abs() < (p.x - to_x(*lo)).abs();
            ui.data_mut(|d| d.insert_temp(handle_id, upper));
        }
    }
    if response.dragged() {
        let upper: Option<bool> = ui.data(|d| d.get_temp(handle_id));
        if let (Some(upper), Some(p)) = (upper, response.interact_pointer_pos()) {
            let v = (to_v(p.x) * 10_000.0).round() / 10_000.0;
            if upper {
                *hi = v.max(*lo);
            } else {
                *lo = v.min(*hi);
            }
            changed = true;
        }
    }
    let accent = style::accent(ui);
    let painter = ui.painter();
    let y = rect.center().y;
    painter.line_segment(
        [Pos2::new(to_x(min), y), Pos2::new(to_x(max), y)],
        Stroke::new(4.0, ui.visuals().widgets.inactive.bg_fill),
    );
    painter.line_segment(
        [Pos2::new(to_x(*lo), y), Pos2::new(to_x(*hi), y)],
        Stroke::new(4.0, accent),
    );
    for v in [*lo, *hi] {
        painter.circle(
            Pos2::new(to_x(v), y),
            7.0,
            ui.visuals().panel_fill,
            Stroke::new(1.5, ui.visuals().text_color()),
        );
    }
    changed
}

/// Drag points to move them, click empty space to invert, double-click to add
/// a point, right-click a point to remove it. End points stay at x = -1 and
/// x = 1. Returns the new points when changed and the selected point.
fn curve_widget(
    ui: &mut egui::Ui,
    id: &str,
    points: &[(f32, f32)],
    deadzone: [f32; 4],
    size: f32,
    selected: Option<usize>,
    help: &str,
) -> (Option<Vec<(f32, f32)>>, Option<usize>) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let response = response.on_hover_text(help);
    let half = size / 2.0;
    let to_screen =
        |(x, y): (f32, f32)| Pos2::new(rect.center().x + x * half, rect.center().y - y * half);
    let to_curve = |p: Pos2| {
        (
            ((p.x - rect.center().x) / half).clamp(-1.0, 1.0),
            ((rect.center().y - p.y) / half).clamp(-1.0, 1.0),
        )
    };
    let accent = style::accent(ui);
    let weak = ui.visuals().weak_text_color();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, style::RADIUS, ui.visuals().extreme_bg_color);

    // Deadzones: input ranges that produce no change, shaded across the graph.
    let band = |a: f32, b: f32| {
        if b > a {
            let r = egui::Rect::from_x_y_ranges(
                to_screen((a, 0.0)).x..=to_screen((b, 0.0)).x,
                rect.y_range(),
            );
            painter.rect_filled(r, 0.0, accent.gamma_multiply(0.08));
        }
    };
    let [low, center_low, center_high, high] = deadzone;
    band(-1.0, low);
    band(center_low, center_high);
    band(high, 1.0);

    for i in 1..8 {
        let t = -1.0 + i as f32 * 0.25;
        let strength = if i == 4 { 0.45 } else { 0.15 };
        let stroke = Stroke::new(1.0, weak.gamma_multiply(strength));
        painter.line_segment([to_screen((t, -1.0)), to_screen((t, 1.0))], stroke);
        painter.line_segment([to_screen((-1.0, t)), to_screen((1.0, t))], stroke);
    }
    let line: Vec<Pos2> = points.iter().map(|&p| to_screen(p)).collect();
    painter.add(egui::Shape::line(line.clone(), Stroke::new(2.0, accent)));
    for (i, p) in line.iter().enumerate() {
        let fill = if Some(i) == selected {
            accent
        } else {
            ui.visuals().panel_fill
        };
        painter.circle(*p, 6.0, fill, Stroke::new(2.0, accent));
    }
    painter.text(
        rect.left_top() + Vec2::splat(4.0),
        Align2::LEFT_TOP,
        "1",
        FontId::monospace(10.0),
        weak,
    );

    let nearest = |pos: Pos2| {
        line.iter()
            .enumerate()
            .map(|(i, p)| (i, p.distance(pos)))
            .filter(|(_, d)| *d < 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let drag_id = ui.make_persistent_id(("curve-drag", id));
    let mut selected = selected;
    let mut pts = points.to_vec();
    let last = pts.len().saturating_sub(1);
    if response.drag_started() {
        if let Some(i) = response.interact_pointer_pos().and_then(nearest) {
            ui.data_mut(|d| d.insert_temp(drag_id, i));
            selected = Some(i);
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
            return (Some(pts), selected);
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<usize>(drag_id));
    }
    let on_point = response.interact_pointer_pos().and_then(nearest);
    if response.clicked() {
        if let Some(i) = on_point {
            selected = Some(i);
        }
    }
    // A single click on empty space inverts, but only once it is clear it was
    // not the first half of a double-click (which adds a point instead).
    const DOUBLE_CLICK: f64 = 0.3;
    let pending_id = ui.make_persistent_id(("curve-invert", id));
    let now = ui.input(|i| i.time);
    if response.double_clicked() {
        ui.data_mut(|d| d.remove::<f64>(pending_id));
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| on_point.is_none())
        {
            let p = to_curve(pos);
            if let Some(at) = pts.iter().position(|q| q.0 > p.0).filter(|&at| at > 0) {
                pts.insert(at, p);
                return (Some(pts), Some(at));
            }
        }
    } else if response.clicked() && on_point.is_none() {
        ui.data_mut(|d| d.insert_temp(pending_id, now));
    }
    if let Some(clicked_at) = ui.data(|d| d.get_temp::<f64>(pending_id)) {
        if now - clicked_at >= DOUBLE_CLICK {
            ui.data_mut(|d| d.remove::<f64>(pending_id));
            pts.iter_mut().for_each(|p| p.1 = -p.1);
            return (Some(pts), selected);
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(DOUBLE_CLICK));
    }
    if response.secondary_clicked() {
        if let Some(i) = on_point.filter(|&i| i > 0 && i < last) {
            pts.remove(i);
            return (Some(pts), None);
        }
    }
    (None, selected)
}
