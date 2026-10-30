//! New profile wizard, one screen: a name and which virtual joystick each attached controller
//! maps to; "Create" automaps every input (gob-core's `automap`).

use eframe::egui::{self, RichText};
use gob_core::InputLayout;
use gob_i18n::I18n;
use gob_input::DeviceState;

use crate::style::{self, icon};

pub struct Wizard {
    name: String,
    /// Per attached controller (GUID): the vJoy device it maps to.
    targets: Vec<(String, Option<u32>)>,
}

/// What "Create" asks for: the profile name, and each controller with its vJoy device.
pub struct Plan {
    pub name: String,
    pub maps: Vec<Mapping>,
}

pub struct Mapping {
    pub device: String,
    pub device_name: String,
    pub from: InputLayout,
    pub vjoy: u32,
    pub to: InputLayout,
}

fn layout_of(d: &DeviceState) -> InputLayout {
    InputLayout {
        axes: d.info.axes.clone(),
        buttons: d.info.buttons as u32,
        hats: d.info.hats as u32,
    }
}

fn vjoy_layout(d: &gob_virtual::DeviceInfo) -> InputLayout {
    InputLayout {
        axes: d.axes.clone(),
        buttons: d.buttons,
        hats: d.hats(),
    }
}

/// Axis, button and hat counts as icons, like the vJoy device tabs.
fn counts(l: &InputLayout) -> String {
    format!(
        "{} {}   {} {}   {} {}",
        icon::ARROWS_OUT_LINE_HORIZONTAL,
        l.axes.len(),
        icon::RADIO_BUTTON,
        l.buttons,
        icon::ARROWS_OUT_CARDINAL,
        l.hats
    )
}

impl Wizard {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            targets: Vec::new(),
        }
    }

    /// Shows the wizard; `Some` when "Create" was clicked. `open` turns false on Cancel.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &I18n,
        devices: &[DeviceState],
        vjoy: &[gob_virtual::DeviceInfo],
        open: &mut bool,
    ) -> Option<Plan> {
        let physical: Vec<&DeviceState> = devices.iter().filter(|d| !d.info.is_own()).collect();
        // New controllers get the next vJoy device nobody has yet; unplugged ones drop out.
        self.targets
            .retain(|(g, _)| physical.iter().any(|d| &d.info.guid == g));
        for d in &physical {
            if !self.targets.iter().any(|(g, _)| *g == d.info.guid) {
                let free = vjoy
                    .iter()
                    .map(|v| v.id)
                    .find(|id| !self.targets.iter().any(|(_, t)| *t == Some(*id)));
                self.targets.push((d.info.guid.clone(), free));
            }
        }

        let mut plan = None;
        style::modal(ctx, "wizard").show(ctx, |ui| {
            ui.set_width(640.0);
            style::dialog_title(ui, Some((icon::MAGIC_WAND, None)), &i18n.tr("wizard-title"));
            ui.label(i18n.tr("wizard-name"));
            let r = ui.add(
                style::text_field(&mut self.name)
                    .desired_width(f32::INFINITY)
                    .hint_text(i18n.tr("wizard-name-hint")),
            );
            // The name field has the focus when the wizard opens.
            if ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            ui.add_space(style::SPACE_M);
            plan = self.devices(ui, i18n, &physical, vjoy, open);
        });
        plan
    }

    fn devices(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        physical: &[&DeviceState],
        vjoy: &[gob_virtual::DeviceInfo],
        open: &mut bool,
    ) -> Option<Plan> {
        ui.label(i18n.tr("wizard-devices"));
        ui.add_space(style::SPACE_XS);
        if physical.is_empty() {
            ui.label(RichText::new(i18n.tr("no-devices-attached")).weak());
        }
        if vjoy.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, i18n.tr("wizard-no-vjoy"));
        }
        let none = i18n.tr("wizard-none");
        let vjoy_name = |v: &gob_virtual::DeviceInfo| crate::vjoy_view::device_name(i18n, v);
        // What does not fit, per row: shown as ⚠ there and in words under the list.
        let mut warnings = Vec::new();
        egui::Grid::new("wizard-devices")
            .num_columns(4)
            .spacing([style::SPACE_M, style::SPACE_S])
            .show(ui, |ui| {
                for d in physical {
                    let from = layout_of(d);
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        let name = format!("{}  {}", icon::JOYSTICK, d.info.name.trim());
                        ui.add(egui::Label::new(name).truncate())
                            .on_hover_text(d.info.name.trim());
                        ui.label(RichText::new(counts(&from)).small().weak());
                    });
                    ui.label(RichText::new(icon::ARROW_RIGHT).size(style::ICON_SIZE));
                    let taken: Vec<u32> = self
                        .targets
                        .iter()
                        .filter(|(g, _)| *g != d.info.guid)
                        .filter_map(|(_, t)| *t)
                        .collect();
                    let Some((_, target)) =
                        self.targets.iter_mut().find(|(g, _)| *g == d.info.guid)
                    else {
                        continue;
                    };
                    let chosen = target.and_then(|t| vjoy.iter().find(|v| v.id == t));
                    let selected = chosen.map_or(none.clone(), |v| {
                        format!("{}  {}", icon::GAME_CONTROLLER, vjoy_name(v))
                    });
                    ui.vertical(|ui| {
                        egui::ComboBox::from_id_salt(("wizard-target", &d.info.guid))
                            .selected_text(selected)
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(target, None, &none);
                                for v in vjoy {
                                    // Each virtual joystick takes one controller: two would
                                    // fight over its outputs.
                                    let text = format!(
                                        "{}  {}    {}",
                                        icon::GAME_CONTROLLER,
                                        vjoy_name(v),
                                        counts(&vjoy_layout(v))
                                    );
                                    let free = !taken.contains(&v.id);
                                    ui.add_enabled_ui(free, |ui| {
                                        ui.selectable_value(target, Some(v.id), text)
                                    })
                                    .inner
                                    .on_disabled_hover_text(i18n.tr("wizard-taken"));
                                }
                            });
                        if let Some(v) = chosen {
                            ui.label(RichText::new(counts(&vjoy_layout(v))).small().weak());
                        }
                    });
                    let missing = chosen
                        .map(|v| (v, fewer(i18n, &from.missing_in(&vjoy_layout(v)))))
                        .filter(|(_, m)| !m.is_empty());
                    match missing {
                        Some((v, missing)) => {
                            ui.label(
                                RichText::new(icon::WARNING)
                                    .size(style::ICON_SIZE)
                                    .color(ui.visuals().warn_fg_color),
                            )
                            .on_hover_text(&missing);
                            warnings.push(i18n.trf(
                                "wizard-fewer",
                                &[
                                    ("device", d.info.name.trim().to_string().into()),
                                    ("vjoy", vjoy_name(v).into()),
                                    ("missing", missing.into()),
                                ],
                            ));
                        }
                        None => {
                            ui.label("");
                        }
                    }
                    ui.end_row();
                }
            });
        for text in warnings {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{}  {text}", icon::WARNING),
            );
        }
        let ready = !self.name.trim().is_empty() && self.targets.iter().any(|(_, t)| t.is_some());
        let buttons = [
            style::DialogButton::primary(i18n.tr("wizard-create")).enabled(ready),
            style::DialogButton::secondary(i18n.tr("cancel")),
        ];
        let clicked = style::dialog_buttons(ui, &buttons);
        if clicked == Some(1) {
            *open = false;
        }
        let create = clicked == Some(0);
        (create && ready).then(|| Plan {
            name: self.name.trim().to_string(),
            maps: self
                .targets
                .iter()
                .filter_map(|(g, t)| {
                    let d = physical.iter().find(|d| &d.info.guid == g)?;
                    let v = vjoy.iter().find(|v| Some(v.id) == *t)?;
                    Some(Mapping {
                        device: g.clone(),
                        device_name: d.info.name.clone(),
                        from: layout_of(d),
                        vjoy: v.id,
                        to: vjoy_layout(v),
                    })
                })
                .collect(),
        })
    }
}

/// "2 axes, 8 buttons" that do not fit.
fn fewer(i18n: &I18n, missing: &InputLayout) -> String {
    let mut parts = Vec::new();
    if !missing.axes.is_empty() {
        parts.push(i18n.trf("wizard-axes", &[("n", missing.axes.len().into())]));
    }
    if missing.buttons > 0 {
        parts.push(i18n.trf("wizard-buttons", &[("n", missing.buttons.into())]));
    }
    if missing.hats > 0 {
        parts.push(i18n.trf("wizard-hats", &[("n", missing.hats.into())]));
    }
    parts.join(", ")
}
