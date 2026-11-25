//! The Device tab: a picture of the player's own controller with the game actions on each of its
//! controls. Actions are dragged from the list onto a control; a control's menu unbinds them or
//! teaches the control (press it, its numbering is kept for this device). Binding goes the same
//! way as pressing the control: through the profile's output it writes, mapping it first when it
//! writes none.

use std::collections::HashMap;

use gob_devices::{Control, DevicePlugin, Direction, Input, Inputs, Kind};

use super::bind::{Aim, bindings_of, outputs_of, remove_ops};
use super::capture::Pressed;
use super::graph::Graph;
use super::state::State;
use super::*;

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
}

/// A control part's name on its card: arrows for directions, words for the rest.
fn part_label(i18n: &I18n, part: &str) -> String {
    match part {
        "" => String::new(),
        "up" => icon::ARROW_UP.into(),
        "right" => icon::ARROW_RIGHT.into(),
        "down" => icon::ARROW_DOWN.into(),
        "left" => icon::ARROW_LEFT.into(),
        "x" => "X".into(),
        "y" => "Y".into(),
        other => i18n.tr(&format!("device-part-{other}")),
    }
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

/// The first input that changed from `rest`: a button or hat direction that went on, or an
/// axis that moved far.
fn changed(rest: &Snapshot, now: &DeviceState, kind: Kind) -> Option<Input> {
    let axis_wanted = matches!(kind, Kind::Axis | Kind::Stick);
    if axis_wanted {
        for (i, (v, r)) in now.axes.iter().zip(&rest.axes).enumerate() {
            if (v - r).abs() > AXIS_TRAVEL {
                return Some(Input::Axis(i as u8 + 1));
            }
        }
        return None;
    }
    for (i, (on, was)) in now.buttons.iter().zip(&rest.buttons).enumerate() {
        if *on && !*was {
            return Some(Input::Button(i as u16 + 1));
        }
    }
    for (i, (h, was)) in now.hats.iter().zip(&rest.hats).enumerate() {
        if let (Some(angle), None) = (h, was) {
            return Some(Input::HatDirection(
                i as u8 + 1,
                Direction::of_angle(*angle),
            ));
        }
    }
    None
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
        let controls: Vec<&Control> = plugin
            .manifest
            .controls
            .iter()
            .filter(|c| c.view == tab.view)
            .collect();
        let teaching = tab
            .teach
            .as_ref()
            .and_then(|t| t.queue.first())
            .map(|(c, _)| c.clone());
        // Per control: its lines, and (action, output) of every binding for its menu.
        let mut menus: Vec<Vec<(String, GameOutput)>> = Vec::new();
        let callouts = crate::device_pictures::callouts(plugin, &tab.view, |c| {
            let mut lines = Vec::new();
            let mut menu = Vec::new();
            let mut lit = teaching.as_deref() == Some(c.id.as_str());
            for part in c.parts() {
                let label = part_label(i18n, part);
                let Some(input) = plugin.input_of(&c.id, part) else {
                    lines.push(format!("{label} {}", i18n.tr("game-device-unknown")));
                    continue;
                };
                lit |= active(device, input);
                let p = pressed_of(device, input);
                let (_, bindings) = bindings_of(ctx, &p);
                let mut names: Vec<String> = Vec::new();
                for o in outputs_of(ctx, &p, &bindings) {
                    for action in drives.get(&o).into_iter().flatten() {
                        if !menu
                            .iter()
                            .any(|(a, out): &(String, GameOutput)| a == action && *out == o)
                        {
                            menu.push((action.to_string(), o.clone()));
                        }
                        let name = name_of(action);
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                let what = if names.is_empty() {
                    i18n.tr("game-device-drop")
                } else {
                    names.join(", ")
                };
                lines.push(if label.is_empty() {
                    what
                } else {
                    format!("{label}  {what}")
                });
            }
            menus.push(menu);
            (lines, lit)
        });
        if tab.teach.is_some() || controls.iter().any(|c| active_any(device, plugin, c)) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        let response = style::DevicePicture::new(
            ("game-device", plugin.id(), &tab.view),
            egui::Image::new(&texture),
            size,
            &callouts,
        )
        .caption(plugin.manifest.credit.as_deref())
        .max_height((ui.available_height() - 24.0).max(240.0))
        .show(ui);

        for (i, r) in response.callouts.iter().enumerate() {
            let control = controls[i];
            if let Some(action) = r.dnd_release_payload::<String>() {
                let parts = control.parts();
                if parts.len() == 1 {
                    match plugin.input_of(&control.id, parts[0]) {
                        Some(input) => {
                            tab.pending = Some((action.to_string(), pressed_of(device, input)));
                        }
                        None => start_teach(tab, device, plugin, vec![control]),
                    }
                } else {
                    tab.choose = Some((action.to_string(), control.id.clone()));
                }
            }
            let menu = &menus[i];
            r.context_menu(|ui| {
                for (action, output) in menu {
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
                    start_teach(tab, device, plugin, vec![control]);
                    ui.close();
                }
            });
            r.clone().on_hover_text(i18n.tr("game-device-card-tip"));
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
        if let Some(input) = changed(rest, device, c.kind) {
            teach.taught.set(&control, &part, input);
            teach.queue.remove(0);
            teach.rest = None;
        }
        let what = match part_label(i18n, &part) {
            l if l.is_empty() => c.name.clone(),
            l => format!("{} {l}", c.name),
        };
        let mut skip = false;
        let mut cancel = false;
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
                    skip = ui.button(i18n.tr("game-device-teach-skip")).clicked();
                    cancel = ui.button(i18n.tr("cancel")).clicked();
                });
            });
        if let Some(teach) = &mut tab.teach {
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
                    let label = match part_label(i18n, part) {
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

fn active_any(device: &DeviceState, plugin: &DevicePlugin, c: &Control) -> bool {
    c.parts()
        .into_iter()
        .filter_map(|p| plugin.input_of(&c.id, p))
        .any(|i| active(device, i))
}

fn start_teach(
    tab: &mut DeviceTab,
    device: &DeviceState,
    plugin: &DevicePlugin,
    controls: Vec<&Control>,
) {
    let queue = controls
        .iter()
        .flat_map(|c| c.parts().into_iter().map(|p| (c.id.clone(), p.to_string())))
        .collect();
    tab.teach = Some(Teach {
        device: device.info.guid.clone(),
        plugin: plugin.id().to_string(),
        queue,
        rest: None,
        taught: Inputs::default(),
    });
}

/// Keeps what was taught in the user's numbering of the plugin and shows it at once.
fn finish_teach(tab: &mut DeviceTab, plugin: &DevicePlugin) {
    let Some(teach) = tab.teach.take() else {
        return;
    };
    if teach.taught.0.is_empty() {
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
    // Part by part: a skipped part keeps what was taught before.
    for (control, entry) in &teach.taught.0 {
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
