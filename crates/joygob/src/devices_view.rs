//! Devices page: live state of attached controllers merged with the profile's bindings.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Margin, Rect, RichText, Sense, Stroke, Vec2,
};
use gob_core::{Action, InputBinding, InputType};
use gob_i18n::I18n;
use gob_input::{DeviceState, InputEvent};

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
    Other,
}

/// Input kind and its 1-based id, as in Gremlin profiles.
type Key = (Kind, u32);

#[derive(Default)]
pub struct DevicesView {
    selected: Option<String>,
    mode: Option<String>,
    activity: HashMap<(String, Key), Instant>,
    scroll_to: Option<(String, Key)>,
}

struct Tab<'a> {
    guid: &'a str,
    name: &'a str,
    live: Option<&'a DeviceState>,
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
    ) {
        let mut tabs: Vec<Tab> = devices
            .iter()
            .filter(|d| !d.info.is_vjoy())
            .map(|d| Tab {
                guid: &d.info.guid,
                name: &d.info.name,
                live: Some(d),
            })
            .collect();
        if let Some(p) = profile {
            for d in &p.profile.devices {
                if !tabs.iter().any(|t| t.guid == d.id) {
                    tabs.push(Tab {
                        guid: &d.id,
                        name: d.name.trim(),
                        live: None,
                    });
                }
            }
        }
        if !self
            .selected
            .as_ref()
            .is_some_and(|s| tabs.iter().any(|t| t.guid == s))
        {
            self.selected = tabs.first().map(|t| t.guid.to_string());
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

        ui.horizontal_wrapped(|ui| {
            if let (Some(p), Some(mode)) = (profile, self.mode.as_mut()) {
                ui.label(RichText::new(icon::STACK).size(style::ICON_SIZE))
                    .on_hover_text(i18n.tr("label-mode"));
                egui::ComboBox::from_id_salt("mode")
                    .selected_text(mode.as_str())
                    .show_ui(ui, |ui| {
                        for m in &p.profile.modes {
                            ui.selectable_value(mode, m.name.clone(), &m.name);
                        }
                    });
                ui.separator();
            }
            for t in &tabs {
                let selected = self.selected.as_deref() == Some(t.guid);
                let glyph = if t.live.is_some() {
                    icon::JOYSTICK
                } else {
                    icon::PLUGS
                };
                let mut r = style::tab(ui, selected, glyph, t.name);
                if t.live.is_none() {
                    r = r.on_hover_text(i18n.tr("device-disconnected"));
                }
                if r.clicked() {
                    self.selected = Some(t.guid.to_string());
                }
            }
        });
        ui.separator();

        let Some(tab) = tabs
            .iter()
            .find(|t| self.selected.as_deref() == Some(t.guid))
        else {
            crate::app::empty_state(ui, icon::JOYSTICK, &i18n.tr("no-devices-attached"));
            return;
        };

        let mut rows: BTreeMap<Key, Option<&InputBinding>> = BTreeMap::new();
        if let Some(d) = tab.live {
            rows.extend(
                d.info
                    .axes
                    .iter()
                    .map(|&a| ((Kind::Axis, u32::from(a)), None)),
            );
            rows.extend((1..=d.buttons.len() as u32).map(|b| ((Kind::Button, b), None)));
            rows.extend((1..=d.hats.len() as u32).map(|h| ((Kind::Hat, h), None)));
        }
        if let (Some(p), Some(mode)) = (profile, &self.mode) {
            for b in p
                .profile
                .inputs
                .iter()
                .filter(|b| b.device_id == tab.guid && &b.mode == mode)
            {
                let kind = match b.input_type() {
                    InputType::Axis => Kind::Axis,
                    InputType::Button => Kind::Button,
                    InputType::Hat => Kind::Hat,
                    InputType::Other => Kind::Other,
                };
                rows.insert((kind, b.input_id.parse().unwrap_or(0)), Some(b));
            }
        }

        let index = profile
            .map(|p| p.profile.action_index())
            .unwrap_or_default();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (key, binding) in rows {
                    self.row(ui, tab, key, binding, &index);
                }
            });
    }

    fn row(
        &mut self,
        ui: &mut egui::Ui,
        tab: &Tab,
        key: Key,
        binding: Option<&InputBinding>,
        index: &HashMap<&str, &Action>,
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
        let response = frame
            .show(ui, |ui| {
                let header = |ui: &mut egui::Ui| {
                    ui.label(RichText::new(kind_glyph(key.0)).size(16.0));
                    ui.add_sized(
                        [28.0, 18.0],
                        egui::Label::new(RichText::new(key.1.to_string()).monospace()),
                    );
                    live_widget(ui, tab.live, key);
                    if let Some(text) = binding.and_then(|b| summary(index, &b.root_action)) {
                        ui.label(text);
                    }
                };
                match binding {
                    Some(b) => {
                        let id = ui.make_persistent_id((tab.guid, key));
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            id,
                            false,
                        )
                        .show_header(ui, header)
                        .body(|ui| action_tree(ui, index, &b.root_action, 0));
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.add_space(ui.spacing().indent);
                            header(ui);
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

fn kind_glyph(kind: Kind) -> &'static str {
    match kind {
        Kind::Axis => icon::ARROWS_OUT_LINE_HORIZONTAL,
        Kind::Button => icon::RADIO_BUTTON,
        Kind::Hat => icon::ARROWS_OUT_CARDINAL,
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
        Kind::Other => {}
    }
}

fn hat_glyph(direction: Option<u16>) -> &'static str {
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
