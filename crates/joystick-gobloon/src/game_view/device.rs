//! The Device tab: a picture of the player's own controller with the game actions on each of its
//! controls. Actions are dragged from the list onto a control; a control's menu unbinds them or
//! teaches the control (press it, its numbering is kept for this device). Binding goes the same
//! way as pressing the control: through the profile's output it writes, mapping it first when it
//! writes none.

use std::collections::HashMap;

use gob_devices::{Control, DevicePlugin, Direction, Input, Inputs, Kind};

use super::bind::{Aim, bindings_of, outputs_of, remove_ops, route_of};
use super::capture::Pressed;
use super::graph::Graph;
use super::state::State;
use super::*;
use crate::device_pictures::{PartLine, part_label};

/// Width of the action list.
const LIST_WIDTH: f32 = 260.0;
/// How far an axis has to move from where it rested to count when teaching.
const AXIS_TRAVEL: f32 = 0.5;

/// Which actions the list shows.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Show {
    #[default]
    All,
    Unbound,
    Bound,
}

/// Learning which input each control part sends.
struct Teach {
    device: String,
    plugin: String,
    /// (control id, part) still to press, the current one first.
    queue: Vec<(String, String)>,
    /// What the device read when the current part was asked for.
    rest: Option<Snapshot>,
    taught: Inputs,
    /// The last press refused because another control sends it: (input, that control).
    clash: Option<(Input, String)>,
}

#[derive(Clone)]
struct Snapshot {
    buttons: Vec<bool>,
    hats: Vec<Option<u16>>,
    axes: [f32; 8],
}

impl Snapshot {
    fn of(d: &DeviceState) -> Self {
        Self {
            buttons: d.buttons.clone(),
            hats: d.hats.clone(),
            axes: d.axes,
        }
    }
}

#[derive(Default)]
pub(super) struct DeviceTab {
    pictures: crate::device_pictures::DevicePictures,
    /// The device shown (its id on this computer).
    device: Option<String>,
    view: String,
    category: Option<String>,
    show: Show,
    teach: Option<Teach>,
    /// An action dropped on a control with several parts: which part it goes to.
    choose: Option<(String, String)>,
    /// An action to bind to a pressed control, once the page has what binding needs.
    pub(super) pending: Option<(String, Pressed)>,
    /// Axis motion of the shown device (its id): an axis counts as used while it moves.
    motion: Option<(String, Motion)>,
}

/// The press a control part stands for.
fn pressed_of(device: &DeviceState, input: Input) -> Pressed {
    let (kind, id, hat) = match input {
        Input::Button(n) => (InputType::Button, u32::from(n), None),
        Input::Axis(slot) => (InputType::Axis, u32::from(slot), None),
        Input::Hat(n) => (InputType::Hat, u32::from(n), None),
        Input::HatDirection(n, d) => (InputType::Hat, u32::from(n), Some(direction(d))),
    };
    Pressed {
        device: device.info.guid.clone(),
        device_name: device.info.name.clone(),
        kind,
        id,
        hat,
    }
}

fn direction(d: Direction) -> HatDirection {
    match d {
        Direction::Up => HatDirection::Up,
        Direction::Right => HatDirection::Right,
        Direction::Down => HatDirection::Down,
        Direction::Left => HatDirection::Left,
    }
}

/// Whether the device sends `input` now.
fn active(d: &DeviceState, input: Input) -> bool {
    match input {
        Input::Button(n) => d.buttons.get(usize::from(n).wrapping_sub(1)) == Some(&true),
        Input::Axis(slot) => d
            .axes
            .get(usize::from(slot).wrapping_sub(1))
            .is_some_and(|v| v.abs() > 0.5),
        Input::Hat(n) => d
            .hats
            .get(usize::from(n).wrapping_sub(1))
            .is_some_and(Option::is_some),
        Input::HatDirection(n, dir) => d
            .hats
            .get(usize::from(n).wrapping_sub(1))
            .copied()
            .flatten()
            .is_some_and(|a| super::graph::hat_has(a, direction(dir))),
    }
}

/// How long an axis counts as used after it last moved, seconds.
const AXIS_LIVE: f64 = 0.6;
/// How far an axis has to move between frames to count as moving (noise stays below it).
const AXIS_STEP: f32 = 0.04;

/// Where each axis was and when it last moved.
struct Motion {
    at: [f32; 8],
    moved: [f64; 8],
}

impl Motion {
    fn new(d: &DeviceState) -> Self {
        Self {
            at: d.axes,
            moved: [f64::NEG_INFINITY; 8],
        }
    }

    /// Takes the new reading at `now` (seconds); returns the axes moving lately.
    fn update(&mut self, d: &DeviceState, now: f64) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..8 {
            if (d.axes[i] - self.at[i]).abs() > AXIS_STEP {
                self.at[i] = d.axes[i];
                self.moved[i] = now;
            }
            if now - self.moved[i] < AXIS_LIVE {
                out.push(i as u8 + 1);
            }
        }
        out
    }
}

/// What the device sends now: buttons held, hat directions and the `moving` axes.
fn live_inputs(now: &DeviceState, moving: &[u8]) -> Vec<Input> {
    let mut out = Vec::new();
    for (i, on) in now.buttons.iter().enumerate() {
        if *on {
            out.push(Input::Button(i as u16 + 1));
        }
    }
    for (i, h) in now.hats.iter().enumerate() {
        if let Some(angle) = h {
            out.push(Input::HatDirection(
                i as u8 + 1,
                Direction::of_angle(*angle),
            ));
        }
    }
    out.extend(moving.iter().map(|slot| Input::Axis(*slot)));
    out
}

/// An input as the Devices page names it ("Button 7", "Hat 1 ↑", "X Axis").
fn input_text(i18n: &I18n, input: Input) -> String {
    let name = |kind: &str, n: u8| crate::output_marks::input_name(i18n, kind, &n.to_string());
    match input {
        Input::Button(n) => crate::output_marks::input_name(i18n, "button", &n.to_string()),
        Input::Axis(slot) => name("axis", slot),
        Input::Hat(n) => name("hat", n),
        Input::HatDirection(n, d) => {
            let arrow = match d {
                Direction::Up => icon::ARROW_UP,
                Direction::Right => icon::ARROW_RIGHT,
                Direction::Down => icon::ARROW_DOWN,
                Direction::Left => icon::ARROW_LEFT,
            };
            format!("{} {arrow}", name("hat", n))
        }
    }
}

/// The first input that changed from `rest`: a button or hat direction that went on, or an
/// axis that moved far.
/// Every input that changed from `rest`, in that order.
fn changed(rest: &Snapshot, now: &DeviceState, axis_wanted: bool) -> Vec<Input> {
    let mut out = Vec::new();
    if axis_wanted {
        for (i, (v, r)) in now.axes.iter().zip(&rest.axes).enumerate() {
            if (v - r).abs() > AXIS_TRAVEL {
                out.push(Input::Axis(i as u8 + 1));
            }
        }
        return out;
    }
    for (i, (on, was)) in now.buttons.iter().zip(&rest.buttons).enumerate() {
        if *on && !*was {
            out.push(Input::Button(i as u16 + 1));
        }
    }
    // A hat counts when it points a new way: also one resting off its centre (a mini stick
    // that never reads centred).
    for (i, (h, was)) in now.hats.iter().zip(&rest.hats).enumerate() {
        if let Some(angle) = h {
            let d = Direction::of_angle(*angle);
            if was.is_none_or(|w| Direction::of_angle(w) != d) {
                out.push(Input::HatDirection(i as u8 + 1, d));
            }
        }
    }
    out
}

impl GameView {
    /// The Device tab.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn device_tab(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        ctx: &Context,
        game: &str,
        graph: &Graph,
        state: &State,
        ops: &mut Vec<Op>,
    ) {
        // The connected controllers a plugin has a picture of.
        let fitting: Vec<(&DeviceState, String)> = {
            let catalog = self.device.pictures.catalog();
            ctx.devices
                .iter()
                .filter(|d| !d.info.is_own())
                .filter_map(|d| {
                    let i = &d.info;
                    catalog
                        .find(i.vendor_id, i.product_id, &i.name)
                        .map(|p| (d, p.id().to_string()))
                })
                .collect()
        };
        if fitting.is_empty() {
            crate::app::empty_state(ui, icon::JOYSTICK, &i18n.tr("game-device-none"));
            return;
        }
        let tab = &mut self.device;
        if !fitting
            .iter()
            .any(|(d, _)| Some(&d.info.guid) == tab.device.as_ref())
        {
            tab.device = Some(fitting[0].0.info.guid.clone());
            tab.view.clear();
        }
        let Some((device, plugin_id)) = fitting
            .iter()
            .find(|(d, _)| Some(&d.info.guid) == tab.device.as_ref())
            .cloned()
        else {
            return;
        };

        // Device and view chips.
        ui.horizontal_wrapped(|ui| {
            for (d, _) in &fitting {
                let selected = Some(&d.info.guid) == tab.device.as_ref();
                if ui
                    .selectable_label(selected, format!("{}  {}", icon::JOYSTICK, d.info.name))
                    .clicked()
                {
                    tab.device = Some(d.info.guid.clone());
                    tab.view.clear();
                    tab.teach = None;
                }
            }
        });
        let Some(plugin) = tab.pictures.catalog().get(&plugin_id).cloned() else {
            return;
        };
        if !plugin.manifest.views.iter().any(|v| v.id == tab.view)
            && let Some(v) = plugin.manifest.views.first()
        {
            tab.view = v.id.clone();
        }
        if plugin.manifest.views.len() > 1 {
            ui.horizontal(|ui| {
                for v in &plugin.manifest.views {
                    if ui.selectable_label(v.id == tab.view, &v.name).clicked() {
                        tab.view = v.id.clone();
                    }
                }
            });
        }
        self.teach_banner(ui, i18n, device, &plugin);
        ui.add_space(style::SPACE_S);

        // What each output drives, for the cards.
        let mut drives: HashMap<&GameOutput, Vec<&str>> = HashMap::new();
        for (action, bindings) in state.all() {
            for b in bindings.iter().filter(|b| b.mark.in_effect()) {
                drives.entry(b.output).or_default().push(action);
            }
        }
        // The page's state is rebuilt every frame in no fixed order: sorted, the cards keep still.
        for actions in drives.values_mut() {
            actions.sort_unstable();
        }
        let name_of = |id: &str| {
            graph
                .actions
                .iter()
                .find(|a| a.id == id)
                .map_or_else(|| id.to_string(), |a| a.name.clone())
        };

        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(LIST_WIDTH, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.action_list(ui, i18n, graph, state),
            );
            ui.separator();
            ui.vertical(|ui| {
                self.device_picture(
                    ui, i18n, ctx, game, device, &plugin, state, &drives, &name_of, ops,
                );
            });
        });
        self.part_dialog(ui, i18n, device, &plugin);
    }

    /// The game's actions to drag onto a control: search (the page's), category and bound or
    /// unbound.
    fn action_list(&mut self, ui: &mut egui::Ui, i18n: &I18n, graph: &Graph, state: &State) {
        let tab = &mut self.device;
        let mut categories: Vec<&str> = graph.actions.iter().map(|a| a.category.as_str()).collect();
        categories.dedup();
        let all = i18n.tr("game-device-categories");
        egui::ComboBox::from_id_salt("game-device-category")
            .width(LIST_WIDTH - 8.0)
            .selected_text(tab.category.clone().unwrap_or_else(|| all.clone()))
            .show_ui(ui, |ui| {
                if ui.selectable_label(tab.category.is_none(), &all).clicked() {
                    tab.category = None;
                }
                for c in &categories {
                    if ui
                        .selectable_label(tab.category.as_deref() == Some(c), *c)
                        .clicked()
                    {
                        tab.category = Some(c.to_string());
                    }
                }
            });
        ui.horizontal(|ui| {
            for (show, key) in [
                (Show::All, "game-device-show-all"),
                (Show::Unbound, "game-device-show-unbound"),
                (Show::Bound, "game-device-show-bound"),
            ] {
                if ui
                    .selectable_label(tab.show == show, i18n.tr(key))
                    .clicked()
                {
                    tab.show = show;
                }
            }
        });
        let listed: Vec<&super::graph::ActionNode> = graph
            .actions
            .iter()
            .filter(|a| tab.category.as_ref().is_none_or(|c| *c == a.category))
            .filter(|a| {
                let bound = on_joystick(state, &a.id);
                match tab.show {
                    Show::All => true,
                    Show::Unbound => !bound,
                    Show::Bound => bound,
                }
            })
            .collect();
        ui.label(
            RichText::new(i18n.trf("game-device-count", &[("n", (listed.len() as i64).into())]))
                .weak()
                .small(),
        );
        egui::ScrollArea::vertical()
            .id_salt("game-device-actions")
            .auto_shrink([false, false])
            .show_rows(ui, 34.0, listed.len(), |ui, rows| {
                for a in &listed[rows] {
                    let bound = on_joystick(state, &a.id);
                    let id = egui::Id::unique(("game-device-drag", &a.id));
                    ui.dnd_drag_source(id, a.id.clone(), |ui| {
                        ui.set_width(LIST_WIDTH - 16.0);
                        ui.horizontal(|ui| {
                            let mark = if bound {
                                icon::LINK
                            } else {
                                icon::DOTS_SIX_VERTICAL
                            };
                            ui.label(RichText::new(mark).weak());
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.label(&a.name);
                                ui.label(RichText::new(&a.category).weak().small());
                            });
                        });
                    })
                    .response
                    .on_hover_text(i18n.tr("game-device-drag"));
                }
            });
    }

    /// The picture with a card per control.
    #[allow(clippy::too_many_arguments)]
    fn device_picture(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        ctx: &Context,
        game: &str,
        device: &DeviceState,
        plugin: &DevicePlugin,
        state: &State,
        drives: &HashMap<&GameOutput, Vec<&str>>,
        name_of: &dyn Fn(&str) -> String,
        ops: &mut Vec<Op>,
    ) {
        let tab = &mut self.device;
        let dark = ui.visuals().dark_mode;
        let Some((texture, size)) = tab.pictures.picture(ui.ctx(), plugin.id(), &tab.view, dark)
        else {
            return;
        };
        // Live: what the device sends now, also before it is taught, so every press shows.
        if tab
            .motion
            .as_ref()
            .is_none_or(|(id, _)| *id != device.info.guid)
        {
            tab.motion = Some((device.info.guid.clone(), Motion::new(device)));
        }
        let now = ui.input(|i| i.time);
        let moving = tab
            .motion
            .as_mut()
            .map(|(_, m)| m.update(device, now))
            .unwrap_or_default();
        let live = live_inputs(device, &moving);
        let teaching = tab.teach.as_ref().and_then(|t| t.queue.first()).cloned();
        // Per control part: what it shows; per control: (action, output) of its bindings.
        let mut menus: HashMap<String, Vec<(String, GameOutput)>> = HashMap::new();
        let built =
            crate::device_pictures::callouts(plugin, &tab.view, Some(i18n), true, |c, part| {
                let menu = menus.entry(c.id.clone()).or_default();
                let asked = teaching
                    .as_ref()
                    .is_some_and(|(tc, tp)| *tc == c.id && tp == part);
                let Some(input) = plugin.input_of(&c.id, part) else {
                    return PartLine {
                        text: i18n.tr("game-device-unknown"),
                        lit: asked,
                        empty: true,
                        value: None,
                    };
                };
                let value = match input {
                    Input::Axis(slot) => {
                        device.axes.get(usize::from(slot).wrapping_sub(1)).copied()
                    }
                    _ => None,
                };
                let held = asked
                    || match input {
                        Input::Axis(slot) => moving.contains(&slot),
                        _ => active(device, input),
                    };
                let p = pressed_of(device, input);
                let (_, bindings) = bindings_of(ctx, &p);
                let mut names: Vec<String> = Vec::new();
                for o in outputs_of(ctx, &p, &bindings) {
                    let mut route = None;
                    for action in drives.get(&o).into_iter().flatten() {
                        if !menu.iter().any(|(a, out)| a == action && *out == o) {
                            menu.push((action.to_string(), o.clone()));
                        }
                        // Several outputs of one control (short or long press, modes) say
                        // which is which.
                        let route = route.get_or_insert_with(|| route_of(ctx, i18n, &bindings, &o));
                        let name = if route.is_empty() {
                            name_of(action)
                        } else {
                            format!("{} ({route})", name_of(action))
                        };
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                names.sort();
                PartLine {
                    empty: names.is_empty(),
                    text: if names.is_empty() {
                        i18n.tr("game-device-drop")
                    } else {
                        names.join(", ")
                    },
                    lit: held,
                    value,
                }
            });
        let unknown: Vec<Input> = live
            .iter()
            .copied()
            .filter(|i| plugin.control_of(*i).is_none())
            .collect();
        self.live_strip(ui, i18n, plugin, &live);
        let tab = &mut self.device;
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        let response = style::DevicePicture::new(
            ("game-device", plugin.id(), &tab.view),
            egui::Image::new(&texture),
            size,
            &built.list,
        )
        .caption(plugin.manifest.credit.as_deref())
        .max_height((ui.available_height() - 24.0).max(240.0))
        .show(ui);

        let mut quick = None;
        for (i, r) in response.callouts.iter().enumerate() {
            let shown: Vec<&Control> = built
                .controls(i)
                .into_iter()
                .filter_map(|id| plugin.control(id))
                .collect();
            // Clicked while an input the picture does not know yet is held: it is this part.
            if r.clicked()
                && let [input] = unknown.as_slice()
            {
                let on_line = response
                    .line
                    .filter(|(c, _)| *c == i)
                    .and_then(|(c, n)| built.part(c, n));
                let single = (built.parts[i].len() == 1)
                    .then(|| built.part(i, 0))
                    .flatten();
                let by_direction = match input {
                    Input::HatDirection(_, d) => built.parts[i]
                        .iter()
                        .find(|(_, part)| part == d.name())
                        .map(|(c, p)| (c.as_str(), p.as_str())),
                    _ => None,
                };
                if let Some((control, part)) = on_line.or(single).or(by_direction) {
                    let mut taught = Inputs::default();
                    match *input {
                        Input::HatDirection(n, _)
                            if plugin.control(control).is_some_and(|c| c.kind == Kind::Hat)
                                && part.parse::<Direction>().is_ok() =>
                        {
                            set_hat(&mut taught, control, n);
                        }
                        input => taught.set(control, part, input),
                    }
                    let what =
                        plugin.control(control).map_or_else(String::new, |c| {
                            match part_label(Some(i18n), part) {
                                l if l.is_empty() => c.name.clone(),
                                l => format!("{} {l}", c.name),
                            }
                        });
                    keep_taught(tab, plugin, &taught);
                    quick = Some(i18n.trf(
                        "game-device-quick-taught",
                        &[
                            ("input", input_text(i18n, *input).into()),
                            ("control", what.into()),
                        ],
                    ));
                    break;
                }
            }
            if let Some(action) = r.dnd_release_payload::<String>() {
                // Dropped on a line, badge or hat arrow: that part; on a control with one part:
                // that one; else ask which.
                let on_line = response
                    .line
                    .filter(|(c, _)| *c == i)
                    .and_then(|(c, n)| built.part(c, n));
                let single = (built.parts[i].len() == 1)
                    .then(|| built.part(i, 0))
                    .flatten();
                match on_line.or(single) {
                    Some((control, part)) => match plugin.input_of(control, part) {
                        Some(input) => {
                            tab.pending = Some((action.to_string(), pressed_of(device, input)));
                        }
                        None => start_teach(tab, device, plugin, shown.clone()),
                    },
                    None => {
                        if let Some(c) = shown.first() {
                            tab.choose = Some((action.to_string(), c.id.clone()));
                        }
                    }
                }
            }
            let menu: Vec<(String, GameOutput)> = shown
                .iter()
                .flat_map(|c| menus.get(&c.id).cloned().unwrap_or_default())
                .collect();
            r.context_menu(|ui| {
                for (action, output) in &menu {
                    let text = i18n.trf(
                        "game-device-unbind",
                        &[
                            ("action", name_of(action).into()),
                            ("control", output_name(i18n, output).into()),
                        ],
                    );
                    if ui.button(format!("{}  {text}", icon::LINK_BREAK)).clicked() {
                        remove_ops(game, state, action, output, ops);
                        ui.close();
                    }
                }
                if !menu.is_empty() {
                    ui.separator();
                }
                if ui
                    .button(format!(
                        "{}  {}",
                        icon::HAND_TAP,
                        i18n.tr("game-device-teach")
                    ))
                    .clicked()
                {
                    start_teach(tab, device, plugin, shown.clone());
                    ui.close();
                }
            });
            r.clone().on_hover_text(i18n.tr("game-device-card-tip"));
        }
        if quick.is_some() {
            self.note = quick;
        }
    }

    /// What the device sends now, one chip per input: the control it is on the picture, or
    /// its number while the picture does not know it (hold it and click the control).
    fn live_strip(&self, ui: &mut egui::Ui, i18n: &I18n, plugin: &DevicePlugin, live: &[Input]) {
        let accent = style::accent(ui);
        // One row of fixed height, cut at the right edge: what it shows changes every frame and
        // must not move the picture below it.
        let height = ui.spacing().interact_size.y;
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        row.set_clip_rect(rect.intersect(ui.clip_rect()));
        let ui = &mut row;
        {
            ui.label(
                RichText::new(icon::HAND_POINTING).color(if live.is_empty() {
                    ui.visuals().weak_text_color()
                } else {
                    accent
                }),
            )
            .on_hover_text(i18n.tr("game-device-live-tip"));
            if live.is_empty() {
                ui.label(RichText::new(i18n.tr("game-device-live-none")).weak());
                return;
            }
            for input in live {
                let raw = input_text(i18n, *input);
                match plugin.control_of(*input) {
                    Some((c, part)) => {
                        let text = match part_label(Some(i18n), part) {
                            l if l.is_empty() => c.name.clone(),
                            l => format!("{} {l}", c.name),
                        };
                        ui.label(RichText::new(text).color(accent).strong())
                            .on_hover_text(raw);
                    }
                    None => {
                        ui.label(RichText::new(format!("{raw}  {}", icon::QUESTION)).weak())
                            .on_hover_text(i18n.tr("game-device-live-unknown"));
                    }
                }
            }
        }
    }

    /// While teaching: what to press, Skip and Cancel; takes the press and keeps it.
    fn teach_banner(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        device: &DeviceState,
        plugin: &DevicePlugin,
    ) {
        let tab = &mut self.device;
        let Some(teach) = &mut tab.teach else {
            ui.horizontal(|ui| {
                if ui
                    .button(format!(
                        "{}  {}",
                        icon::HAND_TAP,
                        i18n.tr("game-device-teach-all")
                    ))
                    .on_hover_text(i18n.tr("game-device-teach-all-tip"))
                    .clicked()
                {
                    let controls: Vec<&Control> = plugin
                        .manifest
                        .controls
                        .iter()
                        .filter(|c| c.view == tab.view)
                        .collect();
                    start_teach(tab, device, plugin, controls);
                }
            });
            return;
        };
        if teach.device != device.info.guid || teach.plugin != plugin.id() {
            tab.teach = None;
            return;
        }
        let Some((control, part)) = teach.queue.first().cloned() else {
            finish_teach(tab, plugin);
            return;
        };
        let Some(c) = plugin.control(&control) else {
            teach.queue.remove(0);
            return;
        };
        let rest = teach.rest.get_or_insert_with(|| Snapshot::of(device));
        let candidates = changed(rest, device, c.kind.is_axis_part(&part));
        // A mini stick may click its push while it is moved: what another part of the same
        // control already sends is not this part (pushes are asked for first).
        let mine = |input: &Input| {
            c.parts()
                .into_iter()
                .any(|p| p != part && teach.taught.get(&control, p).is_some_and(|t| t == *input))
        };
        let fresh: Vec<Input> = candidates.into_iter().filter(|i| !mine(i)).collect();
        let free = fresh
            .iter()
            .copied()
            .find(|i| taken_by(teach, plugin, &control, *i).is_none());
        let found = free.or(fresh.first().copied());
        let clash = match free {
            Some(_) => None,
            None => found.and_then(|input| taken_by(teach, plugin, &control, input)),
        };
        if let (Some(input), Some(other)) = (found, clash) {
            // Another control sends it already (a slip onto a neighbour, or a press still
            // held): ask again instead of keeping two controls on one input.
            teach.clash = Some((input, other));
            teach.rest = None;
        } else if let Some(input) = found {
            accept(teach, c, &part, input);
        }
        let what = match part_label(Some(i18n), &part) {
            l if l.is_empty() => c.name.clone(),
            l => format!("{} {l}", c.name),
        };
        let mut skip = false;
        let mut cancel = false;
        let mut take = false;
        egui::Frame::new()
            .fill(style::accent(ui).gamma_multiply(0.15))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::HAND_TAP).color(style::accent(ui)));
                    ui.label(i18n.trf(
                        "game-device-teach-press",
                        &[
                            ("control", what.into()),
                            ("device", device.info.name.clone().into()),
                        ],
                    ));
                    if let Some((input, other)) = &teach.clash {
                        ui.label(
                            RichText::new(i18n.trf(
                                "game-device-teach-taken",
                                &[
                                    ("input", input_text(i18n, *input).into()),
                                    ("control", other.clone().into()),
                                ],
                            ))
                            .color(ui.visuals().warn_fg_color),
                        );
                        take = ui
                            .button(i18n.tr("game-device-teach-take"))
                            .on_hover_text(i18n.tr("game-device-teach-take-tip"))
                            .clicked();
                    }
                    skip = ui.button(i18n.tr("game-device-teach-skip")).clicked();
                    cancel = ui.button(i18n.tr("cancel")).clicked();
                });
            });
        if let Some(teach) = &mut tab.teach {
            // The refused input belongs to this part after all: it moves here from the other
            // control when the numbering is kept.
            if take
                && let Some((input, _)) = teach.clash
                && let Some(c) = plugin.control(&control)
            {
                accept(teach, c, &part, input);
            }
            if skip && !teach.queue.is_empty() {
                teach.queue.remove(0);
                teach.rest = None;
            }
            if cancel {
                teach.queue.clear();
            }
        }
        if tab.teach.as_ref().is_some_and(|t| t.queue.is_empty()) {
            finish_teach(tab, plugin);
            self.note = Some(i18n.trf(
                "game-device-taught",
                &[("device", device.info.name.clone().into())],
            ));
        }
    }

    /// An action dropped on a control with several parts: which part.
    fn part_dialog(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        device: &DeviceState,
        plugin: &DevicePlugin,
    ) {
        let tab = &mut self.device;
        let Some((action, control)) = tab.choose.clone() else {
            return;
        };
        let Some(c) = plugin.control(&control) else {
            tab.choose = None;
            return;
        };
        let ctx = ui.ctx().clone();
        let mut close = false;
        style::modal(&ctx, "game-device-part").show(&ctx, |ui| {
            style::dialog_title(
                ui,
                Some((icon::JOYSTICK, None)),
                &i18n.trf(
                    "game-device-which-part",
                    &[("control", c.name.clone().into())],
                ),
            );
            ui.horizontal_wrapped(|ui| {
                for part in c.parts() {
                    let label = match part_label(Some(i18n), part) {
                        l if l.is_empty() => c.name.clone(),
                        l => l,
                    };
                    if ui.button(label).clicked() {
                        match plugin.input_of(&c.id, part) {
                            Some(input) => {
                                tab.pending = Some((action.clone(), pressed_of(device, input)));
                            }
                            None => start_teach(tab, device, plugin, vec![c]),
                        }
                        close = true;
                    }
                }
            });
            if style::dialog_buttons(ui, &[style::DialogButton::secondary(i18n.tr("cancel"))])
                .is_some()
            {
                close = true;
            }
        });
        if close {
            tab.choose = None;
        }
    }

    /// Binds an action dropped on a control, now that the page has what binding needs.
    pub(super) fn bind_dropped(&mut self, scene: &bind::Scene, ops: &mut Vec<Op>) {
        if let Some((action, pressed)) = self.device.pending.take() {
            let aim = Aim {
                action: Some(action),
                replace: None,
            };
            self.bind_pressed(scene, aim, pressed, ops);
        }
    }
}

/// Whether a joystick control drives the action (keyboard and mouse bindings do not count).
fn on_joystick(state: &State, action: &str) -> bool {
    state
        .of(action)
        .iter()
        .any(|b| b.mark.in_effect() && b.output.combo().is_none())
}

/// Keeps `input` for `part` of control `c` (the part asked for now) and moves on. A hat (or a
/// mini stick set up as one) reporting a POV direction is taught whole: its other directions
/// need no press.
fn accept(teach: &mut Teach, c: &Control, part: &str, input: Input) {
    teach.clash = None;
    match input {
        Input::HatDirection(n, _) if c.kind == Kind::Hat => {
            set_hat(&mut teach.taught, &c.id, n);
            teach
                .queue
                .retain(|(qc, qp)| *qc != c.id || qp.parse::<Direction>().is_err());
        }
        _ => {
            teach.taught.set(&c.id, part, input);
            teach.queue.remove(0);
        }
    }
    teach.rest = None;
}

/// All four directions of hat `control` come from POV hat `n` (its push stays as it is).
fn set_hat(taught: &mut Inputs, control: &str, n: u8) {
    for d in Direction::ALL {
        taught.set(control, d.name(), Input::HatDirection(n, d));
    }
}

/// The other control (its name and part) that already sends `input`: in what this round
/// taught, or in the numbering of a control this round does not teach again. Directions of one
/// POV hat count as one input.
fn taken_by(teach: &Teach, plugin: &DevicePlugin, control: &str, input: Input) -> Option<String> {
    let same = |a: Input| match (a, input) {
        (Input::HatDirection(n, _), Input::HatDirection(m, _)) => n == m,
        (a, b) => a == b,
    };
    let again =
        |id: &str| teach.queue.iter().any(|(c, _)| c == id) || teach.taught.0.contains_key(id);
    plugin
        .manifest
        .controls
        .iter()
        .filter(|c| c.id != control)
        .find_map(|c| {
            c.parts().into_iter().find_map(|part| {
                let known = if again(&c.id) {
                    teach.taught.get(&c.id, part)
                } else {
                    plugin.input_of(&c.id, part)
                };
                known
                    .filter(|i| same(*i))
                    .map(|_| match part_label(None, part) {
                        l if l.is_empty() => c.name.clone(),
                        l => format!("{} {l}", c.name),
                    })
            })
        })
}

fn start_teach(
    tab: &mut DeviceTab,
    device: &DeviceState,
    plugin: &DevicePlugin,
    controls: Vec<&Control>,
) {
    // A hat's push first: a mini stick that clicks its push while moved is then not taught
    // the push for a direction.
    let queue = controls
        .iter()
        .flat_map(|c| {
            let mut parts = c.parts();
            if c.kind == Kind::Hat {
                parts.sort_by_key(|p| p.parse::<Direction>().is_ok());
            }
            parts.into_iter().map(|p| (c.id.clone(), p.to_string()))
        })
        .collect();
    tab.teach = Some(Teach {
        device: device.info.guid.clone(),
        plugin: plugin.id().to_string(),
        queue,
        rest: None,
        taught: Inputs::default(),
        clash: None,
    });
}

/// Keeps what was taught in the user's numbering of the plugin and shows it at once.
fn finish_teach(tab: &mut DeviceTab, plugin: &DevicePlugin) {
    let Some(teach) = tab.teach.take() else {
        return;
    };
    keep_taught(tab, plugin, &teach.taught);
}

/// Adds `taught` to the user's numbering of the plugin and shows it at once.
fn keep_taught(tab: &mut DeviceTab, plugin: &DevicePlugin, taught: &Inputs) {
    if taught.0.is_empty() {
        return;
    }
    let Some(dir) = crate::device_pictures::user_dir() else {
        return;
    };
    let path = gob_devices::taught_path(&dir, plugin.id());
    let mut all = match Inputs::load(&path) {
        Ok(i) => i,
        Err(e) => {
            log::warn!("taught numbering {}: {e}", path.display());
            Inputs::default()
        }
    };
    // An input belongs to one control: what was just taught leaves every other control.
    let new: Vec<Input> = taught
        .0
        .values()
        .flat_map(|e| match e {
            gob_devices::Entry::One(i) => vec![*i],
            gob_devices::Entry::Parts(p) => p.values().copied().collect(),
        })
        .collect();
    let same = |a: &Input| {
        new.iter().any(|b| match (a, b) {
            (
                Input::HatDirection(n, _) | Input::Hat(n),
                Input::HatDirection(m, _) | Input::Hat(m),
            ) => n == m,
            (a, b) => a == b,
        })
    };
    all.0.retain(|control, entry| {
        if taught.0.contains_key(control) {
            return true;
        }
        match entry {
            gob_devices::Entry::One(i) => !same(i),
            gob_devices::Entry::Parts(parts) => {
                parts.retain(|_, i| !same(i));
                !parts.is_empty()
            }
        }
    });
    // Part by part: a skipped part keeps what was taught before.
    for (control, entry) in &taught.0 {
        match entry {
            gob_devices::Entry::One(input) => all.set(control, "", *input),
            gob_devices::Entry::Parts(parts) => {
                for (part, input) in parts {
                    all.set(control, part, *input);
                }
            }
        }
    }
    match all.save(&path) {
        Ok(()) => log::info!("taught numbering of {} saved", plugin.id()),
        Err(e) => log::error!("taught numbering {}: {e}", path.display()),
    }
    tab.pictures.reload();
}
