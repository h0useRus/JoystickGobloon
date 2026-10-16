//! Devices page: live state of attached controllers merged with the profile's bindings.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Margin, Rect, RichText, Sense, Stroke, Vec2,
};
use gob_core::{same_device_name, Action, InputBinding, InputType};
use gob_i18n::I18n;
use gob_input::{DeviceState, InputEvent};

use crate::action_editor::{Editor, Op};
use crate::profile_view::{action_tree, summary, LoadedProfile};
use crate::style::{self, icon};

/// How long a row stays highlighted after its input was used.
const HIGHLIGHT: Duration = Duration::from_millis(700);
const LIVE_WIDTH: f32 = 110.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Kind {
    Axis,
    Button,
    Hat,
    /// Keyboard key; the id is Gremlin's `extended << 8 | scan code`.
    Key,
    Other,
}

/// Input kind and its 1-based id, as in Gremlin profiles.
type Key = (Kind, u32);

#[derive(Default)]
pub struct DevicesView {
    selected: Option<String>,
    /// The user picked the tab; otherwise a connected device is preferred.
    user_selected: bool,
    mode: Option<String>,
    activity: HashMap<(String, Key), Instant>,
    scroll_to: Option<(String, Key)>,
    /// The mode icon was clicked: the app opens the modes window.
    pub open_modes: bool,
}

struct Tab<'a> {
    guid: &'a str,
    name: &'a str,
    live: Option<&'a DeviceState>,
    /// Profile device (different GUID, same name) whose bindings are shown on this live device.
    alias: Option<&'a str>,
    /// The profile's logical device: inputs come from the profile and can be edited.
    logical: bool,
}

impl DevicesView {
    /// Pressing a button or moving a hat selects its device and highlights the row.
    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = InputEvent>,
        devices: &[DeviceState],
    ) {
        for event in events {
            let (device, key) = match event {
                InputEvent::Button {
                    device,
                    id,
                    pressed: true,
                } => (device, (Kind::Button, u32::from(id))),
                InputEvent::Hat {
                    device,
                    id,
                    direction: Some(_),
                } => (device, (Kind::Hat, u32::from(id))),
                // Typing must not switch tabs: keys only light up their row.
                InputEvent::Key {
                    scan_code,
                    extended,
                    pressed: true,
                } => {
                    let id = gob_core::key_input_id(gob_core::Key {
                        scan_code,
                        extended,
                    });
                    let key = (gob_core::KEYBOARD_GUID.to_string(), (Kind::Key, id));
                    if self.selected.as_deref() == Some(gob_core::KEYBOARD_GUID) {
                        self.scroll_to = Some(key.clone());
                    }
                    self.activity.insert(key, Instant::now());
                    continue;
                }
                _ => continue,
            };
            if devices
                .iter()
                .any(|d| d.info.guid == device && d.info.is_vjoy())
            {
                continue;
            }
            self.selected = Some(device.clone());
            self.activity.insert((device.clone(), key), Instant::now());
            self.scroll_to = Some((device, key));
        }
    }

    /// Forget the selected mode, e.g. after loading another profile.
    pub fn reset_mode(&mut self) {
        self.mode = None;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: Option<&LoadedProfile>,
        devices: &[DeviceState],
        ops: &mut Vec<Op>,
        vjoy: &[gob_vjoy::DeviceInfo],
    ) {
        let mut tabs: Vec<Tab> = devices
            .iter()
            .filter(|d| !d.info.is_vjoy())
            .map(|d| Tab {
                guid: &d.info.guid,
                name: &d.info.name,
                live: Some(d),
                alias: None,
                logical: false,
            })
            .collect();
        let logical_name = i18n.tr("logical-device");
        let keyboard_name = i18n.tr("keyboard-device");
        if let Some(p) = profile {
            let known = |guid: &str| {
                p.profile
                    .devices
                    .iter()
                    .any(|d| d.id.eq_ignore_ascii_case(guid))
            };
            for d in &p.profile.devices {
                let logical = d.id.eq_ignore_ascii_case(gob_core::LOGICAL_DEVICE_GUID)
                    || d.id.eq_ignore_ascii_case(gob_core::KEYBOARD_GUID);
                if logical || tabs.iter().any(|t| t.guid.eq_ignore_ascii_case(&d.id)) {
                    continue;
                }
                // Same stick on this PC under another GUID: show its bindings on the live tab.
                let mut same: Vec<usize> = (0..tabs.len())
                    .filter(|&i| {
                        let t = &tabs[i];
                        t.live.is_some()
                            && t.alias.is_none()
                            && !known(t.guid)
                            && same_device_name(t.name, &d.name)
                    })
                    .collect();
                if same.len() == 1 {
                    tabs[same.pop().unwrap()].alias = Some(&d.id);
                } else {
                    tabs.push(Tab {
                        guid: &d.id,
                        name: d.name.trim(),
                        live: None,
                        alias: None,
                        logical: false,
                    });
                }
            }
            tabs.push(Tab {
                guid: gob_core::LOGICAL_DEVICE_GUID,
                name: &logical_name,
                live: None,
                alias: None,
                logical: true,
            });
            tabs.push(Tab {
                guid: gob_core::KEYBOARD_GUID,
                name: &keyboard_name,
                live: None,
                alias: None,
                logical: false,
            });
        }
        let valid = self
            .selected
            .as_ref()
            .is_some_and(|s| tabs.iter().any(|t| t.guid == s));
        let on_ghost = tabs.iter().any(|t| {
            self.selected.as_deref() == Some(t.guid)
                && t.live.is_none()
                && !t.logical
                && !is_keyboard(t)
        });
        if !valid || (!self.user_selected && on_ghost) {
            let first = tabs.iter().find(|t| t.live.is_some()).or(tabs.first());
            self.selected = first.map(|t| t.guid.to_string());
        }
        if let Some(p) = profile {
            let modes = &p.profile.modes;
            if !self
                .mode
                .as_ref()
                .is_some_and(|m| modes.iter().any(|x| &x.name == m))
            {
                self.mode = p
                    .profile
                    .startup_mode()
                    .or_else(|| modes.first().map(|m| m.name.clone()));
            }
        }

        // Device tabs on the left (wrapping), the mode selector pinned to the right.
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let (Some(p), Some(mode)) = (profile, self.mode.as_mut()) {
                    egui::ComboBox::from_id_salt("mode")
                        .selected_text(mode.as_str())
                        .show_ui(ui, |ui| {
                            for m in &p.profile.modes {
                                ui.selectable_value(mode, m.name.clone(), &m.name);
                            }
                        });
                    if style::icon_button(ui, true, icon::STACK, &i18n.tr("modes-edit")).clicked() {
                        self.open_modes = true;
                    }
                    ui.separator();
                }
                let tabs_layout = Layout::left_to_right(Align::Center).with_main_wrap(true);
                ui.with_layout(tabs_layout, |ui| {
                    for t in &tabs {
                        let selected = self.selected.as_deref() == Some(t.guid);
                        let (glyph, hint) = match (t.live, t.alias) {
                            _ if t.logical => (icon::CPU, Some("logical-device-tip")),
                            _ if is_keyboard(t) => (icon::KEYBOARD, Some("keyboard-tip")),
                            (Some(_), Some(_)) => (icon::LINK, Some("device-linked")),
                            (Some(_), None) => (icon::JOYSTICK, None),
                            (None, _) => (icon::PLUGS, Some("device-disconnected")),
                        };
                        let mut r = style::tab(ui, selected, glyph, t.name);
                        if let Some(key) = hint {
                            r = r.on_hover_text(i18n.tr(key));
                        }
                        if r.clicked() {
                            self.selected = Some(t.guid.to_string());
                            self.user_selected = true;
                        }
                    }
                });
            });
        });
        ui.separator();

        let Some(tab) = tabs
            .iter()
            .find(|t| self.selected.as_deref() == Some(t.guid))
        else {
            crate::app::empty_state(ui, icon::JOYSTICK, &i18n.tr("no-devices-attached"));
            return;
        };

        let mut rows: BTreeMap<Key, Bound> = BTreeMap::new();
        if let Some(d) = tab.live {
            rows.extend(
                d.info
                    .axes
                    .iter()
                    .map(|&a| ((Kind::Axis, u32::from(a)), Bound::None)),
            );
            rows.extend((1..=d.buttons.len() as u32).map(|b| ((Kind::Button, b), Bound::None)));
            rows.extend((1..=d.hats.len() as u32).map(|h| ((Kind::Hat, h), Bound::None)));
        }
        if let (Some(p), Some(mode)) = (profile, &self.mode) {
            // Walk from the current mode up through its parents; the nearest binding wins.
            let mut chain = vec![mode.clone()];
            while let Some(parent) = p
                .profile
                .modes
                .iter()
                .find(|m| Some(&m.name) == chain.last())
                .and_then(|m| m.parent.clone())
                .filter(|par| !chain.contains(par))
            {
                chain.push(parent);
            }
            for (depth, m) in chain.iter().enumerate() {
                for b in p.profile.inputs.iter().filter(|b| {
                    let own = b.device_id.eq_ignore_ascii_case(tab.guid)
                        || tab
                            .alias
                            .is_some_and(|a| b.device_id.eq_ignore_ascii_case(a));
                    own && &b.mode == m
                }) {
                    let kind = match b.input_type() {
                        InputType::Axis => Kind::Axis,
                        InputType::Button => Kind::Button,
                        InputType::Hat => Kind::Hat,
                        InputType::Other if b.input_type.trim() == "key" => Kind::Key,
                        InputType::Other => Kind::Other,
                    };
                    let key = (kind, b.input_id.parse().unwrap_or(0));
                    let slot = rows.entry(key).or_insert(Bound::None);
                    if matches!(slot, Bound::None) {
                        *slot = if depth == 0 {
                            Bound::Own(b)
                        } else {
                            Bound::Inherited(b)
                        };
                    }
                }
            }
        }

        if is_keyboard(tab) && profile.is_some() {
            // Bind a key by recording it; the row then lists it like any input.
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n.tr("keyboard-add")).weak());
                let field = egui::Id::new("keyboard-add-key");
                if let Some(keys) = crate::key_capture::keys_field(ui, field, &[], i18n) {
                    let mode = self.mode.clone().unwrap_or_default();
                    for k in keys {
                        ops.push(Op::Bind {
                            device: gob_core::KEYBOARD_GUID.into(),
                            name: i18n.tr("keyboard-device"),
                            input_type: "key".into(),
                            input_id: gob_core::key_input_id(k).to_string(),
                            mode: mode.clone(),
                            add: None,
                        });
                    }
                }
            });
            if rows.is_empty() {
                ui.label(RichText::new(i18n.tr("keyboard-empty")).weak());
            }
        }
        let mut labels: HashMap<Key, String> = HashMap::new();
        if let (true, Some(p)) = (tab.logical, profile) {
            for i in p.profile.logical_inputs() {
                let kind = match i.input_type {
                    InputType::Axis => Kind::Axis,
                    InputType::Hat => Kind::Hat,
                    _ => Kind::Button,
                };
                rows.entry((kind, i.id)).or_insert(Bound::None);
                labels.insert((kind, i.id), i.label);
            }
            ui.horizontal(|ui| {
                for (ty, glyph, key, name) in [
                    (
                        InputType::Axis,
                        icon::ARROWS_OUT_LINE_HORIZONTAL,
                        "logical-add-axis",
                        "input-axis-kind",
                    ),
                    (
                        InputType::Button,
                        icon::CIRCLE,
                        "logical-add-button",
                        "input-button-kind",
                    ),
                    (
                        InputType::Hat,
                        icon::CROSSHAIR,
                        "logical-add-hat",
                        "input-hat-kind",
                    ),
                ] {
                    let text = format!("{} {} {glyph}", icon::PLUS, i18n.tr(name));
                    if ui.button(text).on_hover_text(i18n.tr(key)).clicked() {
                        ops.push(Op::AddLogicalInput(ty));
                    }
                }
            });
            if labels.is_empty() {
                ui.label(RichText::new(i18n.tr("logical-empty")).weak());
            }
        }
        let index = profile
            .map(|p| p.profile.action_index())
            .unwrap_or_default();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (key, bound) in rows {
                    let label = labels.get(&key).map(String::as_str);
                    self.row(
                        ui, i18n, profile, ops, vjoy, devices, tab, key, bound, &index, label,
                    );
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: Option<&LoadedProfile>,
        ops: &mut Vec<Op>,
        vjoy: &[gob_vjoy::DeviceInfo],
        inputs: &[DeviceState],
        tab: &Tab,
        key: Key,
        bound: Bound,
        index: &HashMap<&str, &Action>,
        // Logical device input label: editable, with a delete button.
        label: Option<&str>,
    ) {
        let active = self
            .activity
            .get(&(tab.guid.to_string(), key))
            .is_some_and(|t| t.elapsed() < HIGHLIGHT);
        let fill = if active {
            ui.visuals().selection.bg_fill
        } else {
            Color32::TRANSPARENT
        };
        let frame = egui::Frame::new()
            .fill(fill)
            .corner_radius(6)
            .inner_margin(Margin::symmetric(4, 1));
        // Bindings are written for the profile's device id (the alias when linked by name).
        let device = tab.alias.unwrap_or(tab.guid).to_string();
        let input_type = match key.0 {
            Kind::Axis => "axis",
            Kind::Hat => "hat",
            Kind::Key => "key",
            Kind::Button | Kind::Other => "button",
        };
        // Keys act like buttons.
        let behavior = match key.0 {
            Kind::Key => InputType::Button,
            _ => InputType::parse(input_type),
        };
        let mode = self.mode.clone().unwrap_or_default();
        let bind = |add| Op::Bind {
            device: device.clone(),
            name: tab.name.to_string(),
            input_type: input_type.into(),
            input_id: key.1.to_string(),
            mode: mode.clone(),
            add,
        };
        let response = frame
            .show(ui, |ui| {
                let summary_of = |b: &InputBinding| {
                    b.configs
                        .first()
                        .and_then(|c| summary(index, &c.root_action))
                };
                let header = |ui: &mut egui::Ui, ops: &mut Vec<Op>| {
                    ui.label(RichText::new(kind_glyph(key.0)).size(16.0));
                    if key.0 == Kind::Key {
                        let name = crate::key_capture::name(gob_core::key_from_input_id(key.1));
                        ui.add_sized(
                            [110.0, 18.0],
                            egui::Label::new(RichText::new(name).strong()),
                        );
                    } else {
                        ui.add_sized(
                            [28.0, 18.0],
                            egui::Label::new(RichText::new(key.1.to_string()).monospace()),
                        );
                    }
                    match label {
                        Some(text) => {
                            let ty = InputType::parse(input_type);
                            // Edited text lives in egui memory; it is applied on Enter / focus loss.
                            let buf_id = egui::Id::new(("logical-label", key));
                            let mut edited: String = ui
                                .data(|d| d.get_temp(buf_id))
                                .unwrap_or_else(|| text.to_string());
                            let r = ui
                                .add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut edited));
                            if r.lost_focus() {
                                ui.data_mut(|d| d.remove::<String>(buf_id));
                                if edited.trim() != text {
                                    ops.push(Op::RenameLogicalInput {
                                        input_type: ty,
                                        id: key.1,
                                        label: edited,
                                    });
                                }
                            } else if r.has_focus() {
                                ui.data_mut(|d| d.insert_temp(buf_id, edited));
                            }
                            if style::icon_button(ui, true, icon::TRASH, &i18n.tr("logical-remove"))
                                .clicked()
                            {
                                ops.push(Op::RemoveLogicalInput {
                                    input_type: ty,
                                    id: key.1,
                                });
                            }
                        }
                        None => live_widget(ui, tab.live, key),
                    }
                    match bound {
                        Bound::Own(b) => {
                            if let Some(text) = summary_of(b) {
                                ui.label(text);
                            }
                        }
                        Bound::Inherited(b) => {
                            ui.label(RichText::new(icon::ARROW_BEND_UP_LEFT).weak())
                                .on_hover_text(
                                    i18n.trf("edit-inherited", &[("mode", b.mode.clone().into())]),
                                );
                            if let Some(text) = summary_of(b) {
                                ui.label(RichText::new(text).weak());
                            }
                        }
                        Bound::None => {
                            if let Some(p) = profile {
                                let mut editor = Editor {
                                    profile: &p.profile,
                                    i18n,
                                    ops,
                                    behavior,
                                    vjoy,
                                    inputs,
                                    input: Some((&device, key.1)),
                                };
                                editor.add_menu(ui, None, |kind| bind(Some(kind)));
                            }
                        }
                    }
                };
                let id = ui.make_persistent_id((tab.guid, key));
                match (bound, profile) {
                    (Bound::Own(b), Some(p)) => {
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            id,
                            debug_expanded(key),
                        )
                        .show_header(ui, |ui| header(ui, ops))
                        .body(|ui| {
                            let text = format!("{}  {}", icon::TRASH, i18n.tr("edit-unbind"));
                            if ui.button(text).clicked() {
                                ops.push(Op::Unbind {
                                    device: b.device_id.clone(),
                                    input_type: b.input_type.clone(),
                                    input_id: b.input_id.clone(),
                                    mode: b.mode.clone(),
                                });
                            }
                            let behavior = b.configs.first().map_or(behavior, |c| c.behavior());
                            let mut editor = Editor {
                                profile: &p.profile,
                                i18n,
                                ops,
                                behavior,
                                vjoy,
                                inputs,
                                input: Some((&b.device_id, key.1)),
                            };
                            for cfg in &b.configs {
                                editor.container(ui, &cfg.root_action, "actions", 0);
                            }
                        });
                    }
                    (Bound::Inherited(b), _) => {
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            id,
                            false,
                        )
                        .show_header(ui, |ui| header(ui, ops))
                        .body(|ui| {
                            for cfg in &b.configs {
                                action_tree(ui, index, &cfg.root_action, 0);
                            }
                            let text =
                                format!("{}  {}", icon::PENCIL_SIMPLE, i18n.tr("edit-override"));
                            if ui.button(text).clicked() {
                                ops.push(bind(None));
                            }
                        });
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.add_space(ui.spacing().indent);
                            header(ui, ops);
                        });
                    }
                }
            })
            .response;

        if self
            .scroll_to
            .as_ref()
            .is_some_and(|(g, k)| g == tab.guid && *k == key)
        {
            response.scroll_to_me(Some(Align::Center));
            self.scroll_to = None;
        }
        if active {
            ui.ctx().request_repaint_after(HIGHLIGHT);
        }
    }
}

#[derive(Clone, Copy)]
enum Bound<'a> {
    None,
    Own(&'a InputBinding),
    /// Bound in an ancestor mode and used through inheritance.
    Inherited(&'a InputBinding),
}

fn kind_glyph(kind: Kind) -> &'static str {
    match kind {
        Kind::Axis => icon::ARROWS_OUT_LINE_HORIZONTAL,
        Kind::Button => icon::RADIO_BUTTON,
        Kind::Hat => icon::ARROWS_OUT_CARDINAL,
        Kind::Key => icon::KEYBOARD,
        Kind::Other => icon::QUESTION,
    }
}

/// Fixed-width live indicator so the binding summaries line up.
fn live_widget(ui: &mut egui::Ui, live: Option<&DeviceState>, (kind, id): Key) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(LIVE_WIDTH, 16.0), Sense::hover());
    let Some(d) = live else { return };
    let i = id.saturating_sub(1) as usize;
    let accent = style::accent(ui);
    let faint = ui.visuals().faint_bg_color;
    let weak = ui.visuals().weak_text_color();
    let painter = ui.painter();
    let anchor = rect.left_center() + Vec2::new(8.0, 0.0);
    match kind {
        Kind::Axis => {
            let value = d.axis(id as u8).unwrap_or(0.0);
            let bar = Rect::from_min_size(
                rect.left_center() - Vec2::new(0.0, 4.0),
                Vec2::new(LIVE_WIDTH - 10.0, 8.0),
            );
            painter.rect_filled(bar, 4.0, faint);
            let center = bar.center().x;
            let end = center + value * bar.width() / 2.0;
            let fill = Rect::from_x_y_ranges(center.min(end)..=center.max(end), bar.y_range());
            painter.rect_filled(fill, 2.0, accent);
            painter.line_segment(
                [bar.center_top(), bar.center_bottom()],
                Stroke::new(1.0, weak),
            );
        }
        Kind::Button => {
            let pressed = d.buttons.get(i).copied().unwrap_or(false);
            let fill = if pressed {
                accent
            } else {
                Color32::TRANSPARENT
            };
            painter.circle(
                anchor,
                6.0,
                fill,
                Stroke::new(1.5, if pressed { accent } else { weak }),
            );
        }
        Kind::Hat => {
            let direction = d.hats.get(i).copied().flatten();
            let color = if direction.is_some() { accent } else { weak };
            painter.text(
                anchor,
                Align2::CENTER_CENTER,
                hat_glyph(direction),
                FontId::proportional(16.0),
                color,
            );
        }
        // Keys have no live state shown here; their row lights up when pressed.
        Kind::Key | Kind::Other => {}
    }
}

pub(crate) fn hat_glyph(direction: Option<u16>) -> &'static str {
    const ARROWS: [&str; 8] = [
        icon::ARROW_UP,
        icon::ARROW_UP_RIGHT,
        icon::ARROW_RIGHT,
        icon::ARROW_DOWN_RIGHT,
        icon::ARROW_DOWN,
        icon::ARROW_DOWN_LEFT,
        icon::ARROW_LEFT,
        icon::ARROW_UP_LEFT,
    ];
    match direction {
        Some(d) => ARROWS[((usize::from(d) + 2250) / 4500) % 8],
        None => icon::CIRCLE,
    }
}

/// Debug builds: `JOYGOB_EXPAND=axis1` (or button3, hat1) opens that row at start, for
/// screenshot checks of editors without clicking.
fn debug_expanded((kind, id): Key) -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    let name = match kind {
        Kind::Axis => "axis",
        Kind::Button => "button",
        Kind::Hat => "hat",
        Kind::Key => "key",
        Kind::Other => "other",
    };
    std::env::var("JOYGOB_EXPAND").is_ok_and(|v| v == format!("{name}{id}"))
}

fn is_keyboard(tab: &Tab) -> bool {
    tab.guid == gob_core::KEYBOARD_GUID
}
