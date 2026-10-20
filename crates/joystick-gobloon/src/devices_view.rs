//! Devices page: live state of attached controllers merged with the profile's bindings.

use std::collections::{BTreeMap, HashMap};

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Margin, Rect, RichText, Sense, Stroke, Vec2,
};
use gob_core::{Action, InputBinding, InputType, same_device_name};
use gob_i18n::I18n;
use gob_input::{DeviceState, InputEvent};

use crate::action_editor::{Editor, Op};
use crate::profile_view::{LoadedProfile, action_tree};
use crate::style::{self, icon};

const LIVE_WIDTH: f32 = 110.0;
/// Axis change (of -1..1) that counts as using the axis, like pressing a button.
const AXIS_MOVE: f32 = 0.25;
/// Width of the input name in the list.
const NAME_WIDTH: f32 = 104.0;

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
    scroll_to: Option<(String, Key)>,
    /// Axis value when it last counted as moved (per device and axis).
    axis_ref: HashMap<(String, u8), f32>,
    /// Device whose info window is open.
    info: Option<String>,
    /// JOYSTICK_GOBLOON_INFO was handled (debug builds).
    info_shown: bool,
    /// Selected input per device tab (the one the right panel edits).
    selected_input: HashMap<String, Key>,
    /// The mode icon was clicked: the app opens the modes window.
    pub open_modes: bool,
    /// HidHide state of connected devices (GUID -> hidden), filled by the app; devices
    /// without an entry have no eye icon.
    pub hidden: HashMap<String, bool>,
    /// The eye icon was clicked: (device GUID, hide). The app applies it.
    pub hide_request: Option<(String, bool)>,
    /// GUIDs of connected devices with LED control (light-led), filled by the app.
    pub leds: std::collections::HashSet<String>,
    /// Experimental features (light-led) are on, filled by the app.
    pub experimental: bool,
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
    /// Pressing a button, moving a hat or clearly moving an axis selects its device and
    /// scrolls the list to the input (`focus` off: events are only drained). The row is not
    /// highlighted: its live widget already shows the input; a click selects it.
    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = InputEvent>,
        devices: &[DeviceState],
        focus: bool,
    ) {
        for event in events {
            if !focus {
                continue;
            }
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
                // An axis counts once it really moved, not on jitter.
                InputEvent::Axis { device, id, value } => {
                    let reference = self.axis_ref.entry((device.clone(), id)).or_insert(value);
                    if (value - *reference).abs() < AXIS_MOVE {
                        continue;
                    }
                    *reference = value;
                    (device, (Kind::Axis, u32::from(id)))
                }
                // Typing must not switch tabs: keys only scroll an open keyboard tab.
                InputEvent::Key {
                    scan_code,
                    extended,
                    pressed: true,
                } => {
                    let id = gob_core::key_input_id(gob_core::Key {
                        scan_code,
                        extended,
                    });
                    if self.selected.as_deref() == Some(gob_core::KEYBOARD_GUID) {
                        self.scroll_to = Some((gob_core::KEYBOARD_GUID.into(), (Kind::Key, id)));
                    }
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
        vjoy: &[gob_virtual::DeviceInfo],
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
            // The keyboard, then the virtual (logical) device always last.
            tabs.push(Tab {
                guid: gob_core::KEYBOARD_GUID,
                name: &keyboard_name,
                live: None,
                alias: None,
                logical: false,
            });
            tabs.push(Tab {
                guid: gob_core::LOGICAL_DEVICE_GUID,
                name: &logical_name,
                live: None,
                alias: None,
                logical: true,
            });
        }
        // Debug builds: JOYSTICK_GOBLOON_TAB=keyboard | logical opens that tab at start (screenshots).
        if cfg!(debug_assertions) && self.selected.is_none() {
            let guid = match std::env::var("JOYSTICK_GOBLOON_TAB").as_deref() {
                Ok("keyboard") => Some(gob_core::KEYBOARD_GUID),
                Ok("logical") => Some(gob_core::LOGICAL_DEVICE_GUID),
                _ => None,
            };
            if let Some(g) = guid.filter(|g| tabs.iter().any(|t| t.guid == *g)) {
                self.selected = Some(g.to_string());
                self.user_selected = true;
            }
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
                        // Real devices (connected or from the profile) have an info icon
                        // inside their tab; connected ones known to HidHide also an eye.
                        let mut r = if !t.logical && !is_keyboard(t) {
                            let info_tip = i18n.tr("device-info");
                            let hidden = self.hidden.get(t.guid).copied();
                            let eye_tip = i18n.tr(if hidden == Some(true) {
                                "hidhide-hidden"
                            } else {
                                "hidhide-visible"
                            });
                            let led_tip = i18n.tr("device-leds");
                            let mut icons = Vec::new();
                            if let Some(hidden) = hidden {
                                let (glyph, color) = style::eye(ui, hidden);
                                icons.push(style::TabIcon {
                                    glyph,
                                    color: Some(color),
                                    tip: &eye_tip,
                                });
                            }
                            // Shows only that light-led can drive it; clicking does nothing.
                            let leds = t.live.is_some() && self.leds.contains(t.guid);
                            if leds {
                                icons.push(style::TabIcon {
                                    glyph: icon::LIGHTBULB,
                                    color: None,
                                    tip: &led_tip,
                                });
                            }
                            let info_index = icons.len();
                            icons.push(style::TabIcon {
                                glyph: icon::INFO,
                                // The info icon is always in the accent green.
                                color: Some(style::accent(ui)),
                                tip: &info_tip,
                            });
                            let (r, clicked) =
                                style::tab_with_icons(ui, selected, glyph, t.name, &icons);
                            match (clicked, hidden) {
                                (Some(0), Some(hidden)) => {
                                    self.hide_request = Some((t.guid.to_string(), !hidden));
                                }
                                (Some(i), _) if i == info_index => {
                                    self.info = Some(t.guid.to_string());
                                }
                                _ => {}
                            }
                            r
                        } else {
                            style::tab(ui, selected, glyph, t.name)
                        };
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
            // Debug builds: JOYSTICK_GOBLOON_INFO=1 opens the first device's info at start (screenshots).
            if cfg!(debug_assertions)
                && !self.info_shown
                && std::env::var("JOYSTICK_GOBLOON_INFO").is_ok()
            {
                self.info_shown = true;
                self.info = tabs
                    .iter()
                    .find(|t| t.live.is_some())
                    .map(|t| t.guid.to_string());
            }
            self.info_window(ui.ctx(), i18n, &tabs);
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

        // Logical inputs come from the profile; they are listed even when unbound.
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
        }
        let index = profile
            .map(|p| p.profile.action_index())
            .unwrap_or_default();

        // The selected input of this tab: kept per device, else the first row.
        let selected = self
            .selected_input
            .get(tab.guid)
            .copied()
            .filter(|k| rows.contains_key(k))
            .or_else(|| rows.keys().copied().find(|&k| debug_expanded(k)))
            .or_else(|| rows.keys().next().copied());

        // Left: the inputs as cards. Right: the selected input's actions.
        egui::SidePanel::left(egui::Id::new(("inputs", tab.guid)))
            .resizable(true)
            .default_width(420.0)
            .min_width(280.0)
            .show_inside(ui, |ui| {
                self.list_tools(ui, i18n, profile, ops, tab, rows.is_empty());
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for (&key, &bound) in &rows {
                            let label = labels.get(&key).map(String::as_str);
                            let is_sel = selected == Some(key);
                            if self.card(ui, i18n, tab, key, bound, &index, label, is_sel) {
                                self.selected_input.insert(tab.guid.to_string(), key);
                            }
                        }
                    });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(Margin::symmetric(10, 4)))
            .show_inside(ui, |ui| {
                let Some(key) = selected else {
                    return;
                };
                let bound = rows.get(&key).copied().unwrap_or(Bound::None);
                let label = labels.get(&key).map(String::as_str);
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        self.detail(
                            ui, i18n, profile, ops, vjoy, devices, tab, key, bound, &index, label,
                        );
                    });
            });
    }

    /// Name, inputs, VID/PID, Windows joystick id and GUID of a device (copyable GUID).
    fn info_window(&mut self, ctx: &egui::Context, i18n: &I18n, tabs: &[Tab]) {
        let Some(guid) = self.info.clone() else {
            return;
        };
        let Some(tab) = tabs.iter().find(|t| t.guid == guid) else {
            self.info = None;
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{}  {}", icon::INFO, tab.name))
            .id(egui::Id::new("device-info"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                let none = || "—".to_string();
                let live = tab.live.map(|d| &d.info);
                let ids = live
                    .map(|d| gob_input::joystick_ids(d.vendor_id, d.product_id))
                    .unwrap_or_default();
                let rows: [(&str, String); 7] = [
                    ("device-info-name", tab.name.to_string()),
                    (
                        "device-info-axes",
                        live.map_or_else(none, |d| d.axes.len().to_string()),
                    ),
                    (
                        "device-info-buttons",
                        live.map_or_else(none, |d| d.buttons.to_string()),
                    ),
                    (
                        "device-info-hats",
                        live.map_or_else(none, |d| d.hats.to_string()),
                    ),
                    (
                        "device-info-vid",
                        live.map_or_else(none, |d| format!("{:04X}", d.vendor_id)),
                    ),
                    (
                        "device-info-pid",
                        live.map_or_else(none, |d| format!("{:04X}", d.product_id)),
                    ),
                    (
                        "device-info-joystick-id",
                        if ids.is_empty() {
                            none()
                        } else {
                            ids.iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        },
                    ),
                ];
                egui::Grid::new("device-info-grid")
                    .num_columns(2)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        for (key, value) in rows {
                            ui.label(RichText::new(i18n.tr(key)).weak());
                            ui.label(RichText::new(value).monospace());
                            ui.end_row();
                        }
                        ui.label(RichText::new(i18n.tr("device-info-guid")).weak());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tab.guid).monospace());
                            let tip = i18n.tr("device-info-copy");
                            if style::icon_button(ui, true, icon::COPY, &tip).clicked() {
                                ui.ctx().copy_text(tab.guid.to_string());
                            }
                        });
                        ui.end_row();
                    });
                if tab.live.is_none() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(i18n.tr("device-disconnected")).weak());
                }
            });
        if !open {
            self.info = None;
        }
    }

    /// Above the list: record a key (Keyboard tab) or add logical inputs (Logical device).
    fn list_tools(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: Option<&LoadedProfile>,
        ops: &mut Vec<Op>,
        tab: &Tab,
        empty: bool,
    ) {
        let Some(p) = profile else { return };
        if is_keyboard(tab) {
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
            if empty {
                ui.label(RichText::new(i18n.tr("keyboard-empty")).weak());
            }
            ui.separator();
        }
        if tab.logical {
            ui.horizontal_wrapped(|ui| {
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
            if p.profile.logical_inputs().is_empty() {
                ui.label(RichText::new(i18n.tr("logical-empty")).weak());
            }
            ui.separator();
        }
    }

    /// One input in the list: name, live state and description on the first line, the
    /// actions on the second. Returns true when clicked.
    #[allow(clippy::too_many_arguments)]
    fn card(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        tab: &Tab,
        key: Key,
        bound: Bound,
        index: &HashMap<&str, &Action>,
        label: Option<&str>,
        selected: bool,
    ) -> bool {
        let visuals = ui.visuals();
        let fill = if selected {
            // A light accent tint: the live bar's track uses faint_bg_color and must show.
            style::accent(ui).gamma_multiply(0.08)
        } else {
            Color32::TRANSPARENT
        };
        let stroke = if selected {
            Stroke::new(1.5_f32, style::accent(ui))
        } else {
            visuals.widgets.noninteractive.bg_stroke
        };
        let binding = match bound {
            Bound::Own(b) | Bound::Inherited(b) => Some(b),
            Bound::None => None,
        };
        let root = binding
            .and_then(|b| b.configs.first())
            .and_then(|c| index.get(c.root_action.as_str()).copied());
        let inherited = matches!(bound, Bound::Inherited(_));
        let response = egui::Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(6)
            .inner_margin(Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(kind_glyph(key.0)).size(15.0));
                    // Fixed-width name column so the live bars line up.
                    let name = input_name(i18n, key, label);
                    let used = ui.label(RichText::new(name).strong()).rect.width();
                    ui.add_space((NAME_WIDTH - used).max(0.0));
                    if tab.live.is_some() {
                        live_widget(ui, tab.live, key);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if inherited {
                            let mode = binding.map(|b| b.mode.clone()).unwrap_or_default();
                            ui.label(RichText::new(icon::ARROW_BEND_UP_LEFT).weak())
                                .on_hover_text(
                                    i18n.trf("edit-inherited", &[("mode", mode.into())]),
                                );
                        }
                        if let Some(text) = root.and_then(description) {
                            ui.add(egui::Label::new(RichText::new(text).weak()).truncate());
                        }
                    });
                });
                if let Some(root) = root {
                    let children: Vec<&Action> = root
                        .child_ids()
                        .into_iter()
                        .filter_map(|id| index.get(id).copied())
                        .collect();
                    if !children.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                            for a in children {
                                let glyph = crate::action_editor::kind_icon(&a.kind);
                                chip(ui, glyph, &chip_text(i18n, a), inherited);
                            }
                        });
                    }
                }
            })
            .response;
        let response = ui.interact(
            response.rect,
            ui.id().with(("card", tab.guid, key)),
            Sense::click(),
        );
        if self
            .scroll_to
            .as_ref()
            .is_some_and(|(g, k)| g == tab.guid && *k == key)
        {
            response.scroll_to_me(Some(Align::Center));
            self.scroll_to = None;
        }
        ui.add_space(2.0);
        response.clicked()
    }

    /// The selected input's editor (what used to open under its row).
    #[allow(clippy::too_many_arguments)]
    fn detail(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: Option<&LoadedProfile>,
        ops: &mut Vec<Op>,
        vjoy: &[gob_virtual::DeviceInfo],
        inputs: &[DeviceState],
        tab: &Tab,
        key: Key,
        bound: Bound,
        index: &HashMap<&str, &Action>,
        // Logical device input label: editable, with a delete button.
        label: Option<&str>,
    ) {
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

        // Title: input name (a logical input's label is editable here).
        ui.horizontal(|ui| {
            ui.label(RichText::new(kind_glyph(key.0)).size(18.0));
            match label {
                Some(text) => {
                    let ty = InputType::parse(input_type);
                    // Edited text lives in egui memory; it is applied on Enter / focus loss.
                    let buf_id = egui::Id::new(("logical-label", key));
                    let mut edited: String = ui
                        .data(|d| d.get_temp(buf_id))
                        .unwrap_or_else(|| text.to_string());
                    let r = ui.add_sized([200.0, 22.0], egui::TextEdit::singleline(&mut edited));
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
                None => {
                    ui.heading(input_name(i18n, key, None));
                }
            }
            if tab.live.is_some() {
                live_widget(ui, tab.live, key);
            }
        });
        ui.add_space(4.0);

        let Some(p) = profile else { return };
        match bound {
            Bound::Own(b) => {
                let root = b.configs.first().map(|c| c.root_action.clone());
                ui.horizontal(|ui| {
                    // The binding's description: the root action's label, like Gremlin.
                    if let Some(root_id) = &root
                        && let Some(a) = p.profile.action(root_id)
                    {
                        let mut text = a
                            .label()
                            .filter(|l| *l != "None")
                            .unwrap_or_default()
                            .to_string();
                        let edit = egui::TextEdit::singleline(&mut text)
                            .hint_text(i18n.tr("edit-binding-description"))
                            .desired_width((ui.available_width() - 40.0).max(120.0));
                        if ui.add(edit).changed() {
                            ops.push(Op::Set {
                                action: root_id.clone(),
                                name: "action-label".into(),
                                ty: "string".into(),
                                value: text,
                            });
                        }
                    }
                    if style::icon_button(ui, true, icon::TRASH, &i18n.tr("edit-unbind")).clicked()
                    {
                        ops.push(Op::Unbind {
                            device: b.device_id.clone(),
                            input_type: b.input_type.clone(),
                            input_id: b.input_id.clone(),
                            mode: b.mode.clone(),
                        });
                    }
                });
                ui.add_space(4.0);
                let behavior = b.configs.first().map_or(behavior, |c| c.behavior());
                let mut editor = Editor {
                    profile: &p.profile,
                    i18n,
                    ops,
                    behavior,
                    vjoy,
                    inputs,
                    input: Some((&b.device_id, key.1)),
                    leds: &self.leds,
                    experimental: self.experimental,
                };
                for cfg in &b.configs {
                    editor.container(ui, &cfg.root_action, "actions", 0);
                }
            }
            Bound::Inherited(b) => {
                ui.label(
                    RichText::new(format!(
                        "{}  {}",
                        icon::ARROW_BEND_UP_LEFT,
                        i18n.trf("edit-inherited", &[("mode", b.mode.clone().into())])
                    ))
                    .weak(),
                );
                for cfg in &b.configs {
                    action_tree(ui, index, &cfg.root_action, 0);
                }
                ui.add_space(6.0);
                let text = format!("{}  {}", icon::PENCIL_SIMPLE, i18n.tr("edit-override"));
                if ui.button(text).clicked() {
                    ops.push(bind(None));
                }
            }
            Bound::None => {
                ui.label(RichText::new(i18n.tr("edit-unbound")).weak());
                let mut editor = Editor {
                    profile: &p.profile,
                    i18n,
                    ops,
                    behavior,
                    vjoy,
                    inputs,
                    input: Some((&device, key.1)),
                    leds: &self.leds,
                    experimental: self.experimental,
                };
                editor.add_menu(ui, None, |kind| bind(Some(kind)));
            }
        }
    }
}

/// Name like Gremlin shows it: X Axis ... Slider, Dial; Button 3; Hat 1; key name.
fn input_name(i18n: &I18n, (kind, id): Key, label: Option<&str>) -> String {
    if let Some(l) = label {
        return l.to_string();
    }
    match kind {
        Kind::Axis if (1..=8).contains(&id) => i18n.tr(&format!("vjoy-axis-{id}")),
        Kind::Axis => i18n.trf("viewer-axis", &[("n", id.into())]),
        Kind::Button => i18n.trf("vjoy-button-n", &[("n", id.into())]),
        Kind::Hat => i18n.trf("vjoy-hat-n", &[("n", id.into())]),
        Kind::Key => crate::key_capture::name(gob_core::key_from_input_id(id)),
        Kind::Other => id.to_string(),
    }
}

/// The binding's description: the root action's label (Gremlin shows it there too).
fn description(root: &Action) -> Option<String> {
    root.label()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "None")
        .map(String::from)
}

/// The chip's tooltip: the action's name, plus the target of a vJoy mapping
/// (e.g. "Map to vJoy: vJoy 1 A2") or the text of a description.
fn chip_text(i18n: &I18n, a: &Action) -> String {
    let key = format!("kind-{}", a.kind);
    let name = i18n.tr(&key);
    let name = if name == key { a.kind.clone() } else { name };
    if let gob_core::ActionKind::MapToVjoy {
        device,
        input_type,
        input_id,
        ..
    } = a.kind()
    {
        let t = match input_type {
            InputType::Axis => "A",
            InputType::Hat => "H",
            _ => "B",
        };
        return format!("{name}: vJoy #{device} {t}{input_id}");
    }
    if let gob_core::ActionKind::Description { text } = a.kind() {
        let text = text.trim();
        if !text.is_empty() {
            return format!("{name}: {text}");
        }
    }
    name
}

/// An action of a binding: its icon in a small frame, the name as tooltip.
fn chip(ui: &mut egui::Ui, glyph: &str, tooltip: &str, weak: bool) {
    let visuals = ui.visuals();
    let text = RichText::new(glyph).size(style::ICON_SIZE - 2.0);
    let text = if weak { text.weak() } else { text };
    egui::Frame::new()
        .stroke(visuals.widgets.noninteractive.bg_stroke)
        .corner_radius(3)
        .inner_margin(Margin::symmetric(4, 1))
        // The label takes the hover, so the tooltip goes on it.
        .show(ui, |ui| ui.label(text).on_hover_text(tooltip));
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
                Stroke::new(1.0_f32, weak),
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
                Stroke::new(1.5_f32, if pressed { accent } else { weak }),
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

/// Debug builds: `JOYSTICK_GOBLOON_EXPAND=axis1` (or button3, hat1) opens that row at start, for
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
    std::env::var("JOYSTICK_GOBLOON_EXPAND").is_ok_and(|v| v == format!("{name}{id}"))
}

fn is_keyboard(tab: &Tab) -> bool {
    tab.guid == gob_core::KEYBOARD_GUID
}
