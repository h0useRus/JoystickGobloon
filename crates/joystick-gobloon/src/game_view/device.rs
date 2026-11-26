//! The Controller tab: a connected controller's picture (its device plugin, mapped in the Input
//! viewer) with the game actions each of its controls drives through the profile, lit as the
//! controls are worked: a preview to check the bindings on the real stick. The cards can be
//! dragged elsewhere; where they go is kept in the profile (`<picture-cards>`), so a shared
//! profile shows them the same.

use gob_core::{CardSide, PictureCard, Press};
use gob_devices::{Control, Direction, Input, Inputs};

use super::bind::{bindings_of, outputs_of};
use super::capture::Pressed;
use super::state::State;
use super::*;

#[derive(Default)]
pub(super) struct DeviceTab {
    /// The device plugins, loaded when the tab is first shown.
    pictures: Option<crate::mapping::Mapper>,
    /// The controller shown (its id on this computer).
    device: Option<String>,
    view: String,
}

/// The press of the profile's input a control part stands for.
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

/// A part's short name before its actions: arrows for directions, words for the rest.
fn part_label(i18n: &I18n, part: &str) -> String {
    match part {
        "" => String::new(),
        "up" => icon::ARROW_UP.into(),
        "right" => icon::ARROW_RIGHT.into(),
        "down" => icon::ARROW_DOWN.into(),
        "left" => icon::ARROW_LEFT.into(),
        "x" => "X".into(),
        "y" => "Y".into(),
        "push" => i18n.tr("map-part-push"),
        "twist" => i18n.tr("map-part-twist"),
        other => other.to_string(),
    }
}

/// What the tab draws of the plugin.
struct Shown {
    id: String,
    views: Vec<(String, String)>,
    controls: Vec<Control>,
    inputs: Inputs,
    credit: Option<String>,
}

impl GameView {
    pub(super) fn device_tab(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        ctx: &Context,
        catalog: &Catalog,
        state: &State,
        ops: &mut Vec<Op>,
    ) {
        let tab = &mut self.device;
        let pictures = tab.pictures.get_or_insert_with(Default::default);
        pictures.update(ctx.devices);
        // The connected controllers with a picture.
        let fitting: Vec<(&DeviceState, Shown)> = ctx
            .devices
            .iter()
            .filter_map(|d| {
                let p = pictures.plugin_for(&d.info)?;
                let shown = Shown {
                    id: p.id().to_string(),
                    views: p
                        .manifest
                        .views
                        .iter()
                        .map(|v| (v.id.clone(), v.name.clone()))
                        .collect(),
                    controls: p.manifest.controls.clone(),
                    inputs: p.inputs.clone(),
                    credit: p.manifest.credit.clone(),
                };
                Some((d, shown))
            })
            .collect();
        if fitting.is_empty() {
            crate::app::empty_state(ui, icon::JOYSTICK, &i18n.tr("game-device-none"));
            return;
        }
        if !fitting
            .iter()
            .any(|(d, _)| tab.device.as_deref() == Some(d.info.guid.as_str()))
        {
            tab.device = Some(fitting[0].0.info.guid.clone());
        }
        let Some((device, shown)) = fitting
            .iter()
            .find(|(d, _)| tab.device.as_deref() == Some(d.info.guid.as_str()))
        else {
            return;
        };
        if !shown.views.iter().any(|(v, _)| *v == tab.view)
            && let Some((v, _)) = shown.views.first()
        {
            tab.view = v.clone();
        }
        let placed = ctx
            .profile
            .picture_cards
            .iter()
            .any(|c| c.plugin == shown.id && c.view == tab.view);
        if fitting.len() > 1 || shown.views.len() > 1 || placed {
            ui.horizontal_wrapped(|ui| {
                for (d, _) in &fitting {
                    let on = tab.device.as_deref() == Some(d.info.guid.as_str());
                    if style::tab(ui, on, icon::JOYSTICK, d.info.name.trim()).clicked() {
                        tab.device = Some(d.info.guid.clone());
                    }
                }
                if shown.views.len() > 1 {
                    ui.add_space(style::SPACE_L);
                    for (id, name) in &shown.views {
                        if style::tab(ui, tab.view == *id, icon::IMAGE, name).clicked() {
                            tab.view = id.clone();
                        }
                    }
                }
                if placed {
                    ui.add_space(style::SPACE_L);
                    if style::icon_button(
                        ui,
                        true,
                        icon::MAGIC_WAND,
                        &i18n.tr("game-device-arrange"),
                    )
                    .clicked()
                    {
                        ops.push(Op::ResetPictureCards {
                            plugin: shown.id.clone(),
                            view: tab.view.clone(),
                        });
                    }
                }
            });
            ui.add_space(style::SPACE_S);
        }

        // What each output drives in the game, presses apart: what the game has after the next
        // export, or while the profile holds no bindings for it, what the game has now.
        let held = state
            .all()
            .any(|(_, list)| list.iter().any(|b| b.mark.profile()));
        let mut drives: HashMap<GameOutput, Vec<(&str, Option<Press>)>> = HashMap::new();
        for (action, bindings) in state.all() {
            for b in bindings.iter().filter(|b| b.mark.in_effect() || !held) {
                drives
                    .entry(b.output.base().clone())
                    .or_default()
                    .push((action, b.output.press()));
            }
        }
        for list in drives.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        let name_of = |id: &str| {
            catalog
                .action(id)
                .map_or_else(|| id.to_string(), |a| a.name.clone())
        };
        // The actions of one input of the controller.
        let actions_of = |input: Input| -> Vec<String> {
            let p = pressed_of(device, input);
            let (_, bindings) = bindings_of(ctx, &p);
            let mut names: Vec<String> = Vec::new();
            for o in outputs_of(ctx, &p, &bindings) {
                for (action, press) in drives.get(&o).into_iter().flatten() {
                    let name = match press {
                        Some(press) => {
                            format!("{} ({})", name_of(action), press_name(i18n, *press))
                        }
                        None => name_of(action),
                    };
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
            names
        };
        let live = |input: Input| gob_devices::draw::Live {
            lit: gob_devices::is_active(device, input),
            value: gob_devices::axis_value(device, input),
        };
        let controls: Vec<&Control> = shown
            .controls
            .iter()
            .filter(|c| c.view == tab.view)
            .collect();
        let callouts: Vec<style::Callout> = controls
            .iter()
            .map(|c| {
                let mut lines = Vec::new();
                let mut lit = false;
                for part in c.parts() {
                    let Some(input) = shown.inputs.get(&c.id, part) else {
                        continue;
                    };
                    lit |= live(input).lit;
                    let actions = actions_of(input);
                    if actions.is_empty() {
                        continue;
                    }
                    let label = part_label(i18n, part);
                    let text = actions.join(", ");
                    lines.push(if label.is_empty() {
                        text
                    } else {
                        format!("{label}  {text}")
                    });
                }
                if lines.is_empty() {
                    lines.push("—".into());
                }
                let placed = ctx.profile.picture_card(&shown.id, &tab.view, &c.id);
                style::Callout {
                    // The card points at the control as drawn (off its spot when moved out).
                    at: Pos2::new(c.drawn()[0], c.drawn()[1]),
                    side: match (placed.map(|p| p.side), c.side) {
                        (Some(CardSide::Left), _) | (None, gob_devices::Side::Left) => {
                            style::CalloutSide::Left
                        }
                        (Some(CardSide::Right), _) | (None, gob_devices::Side::Right) => {
                            style::CalloutSide::Right
                        }
                    },
                    title: c.name.clone(),
                    lines,
                    group: None,
                    lit,
                    place: placed.map(|p| p.y),
                }
            })
            .collect();
        let Some((texture, size)) = pictures.picture(ui.ctx(), &shown.id, &tab.view) else {
            return;
        };
        let pal = gob_devices::draw::Pal::new(ui.visuals(), style::accent(ui));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        egui::ScrollArea::vertical()
            .id_salt("game-device")
            .auto_shrink(false)
            .show(ui, |ui| {
                let r = style::DevicePicture::new(
                    ("game-device", &shown.id, &tab.view),
                    egui::Image::new(&texture),
                    size,
                    &callouts,
                )
                .caption(shown.credit.as_deref())
                // The same size as in the Input viewer (smaller only when it does not fit).
                .max_height(size.y * crate::mapping::picture_scale(size))
                .movable_cards(true)
                .show(ui);
                // A card dropped elsewhere stays there (in the profile).
                if let Some((i, side, y)) = r.moved
                    && let Some(c) = controls.get(i)
                {
                    ops.push(Op::PictureCard(PictureCard {
                        plugin: shown.id.clone(),
                        view: tab.view.clone(),
                        control: c.id.clone(),
                        side: match side {
                            style::CalloutSide::Left => CardSide::Left,
                            style::CalloutSide::Right => CardSide::Right,
                        },
                        y,
                    }));
                }
                // A card's whole text when it does not fit.
                for (c, response) in callouts.iter().zip(&r.callouts) {
                    if c.lines.len() > 1 || c.lines.first().is_some_and(|l| l.chars().count() > 24)
                    {
                        response.clone().on_hover_text(format!(
                            "{}
{}",
                            c.title,
                            c.lines.join(
                                "
"
                            )
                        ));
                    }
                }
                // The controls drawn over their spots, as in the Input viewer.
                let painter = ui.painter_at(r.response.rect.expand(48.0));
                let to_screen = |at: [f32; 2]| r.picture.min + Vec2::new(at[0], at[1]) * r.scale;
                for c in &controls {
                    gob_devices::draw::leader_of(
                        &painter,
                        &pal,
                        c,
                        &shown.inputs,
                        &to_screen,
                        &live,
                    );
                }
                for (n, c) in controls.iter().enumerate() {
                    let at = to_screen(c.drawn());
                    gob_devices::draw::control(
                        &painter,
                        &pal,
                        c,
                        &shown.inputs,
                        at,
                        None,
                        r.hovered == Some(n),
                        None,
                        &live,
                    );
                }
            });
    }
}
