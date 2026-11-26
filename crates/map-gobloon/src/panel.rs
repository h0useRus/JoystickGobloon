//! The side panel: the device, how much of it is mapped, its controls, and the selected
//! control's editor (name, kind, extra parts, which input each part has).

use eframe::egui::{self, RichText};
use gob_devices::{Control, Kind, Look, Side};
use gob_ui as style;
use style::icon;

use crate::app::{KINDS, MapApp};

impl MapApp {
    pub fn panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(style::SPACE_M);
        let Some(plugin) = self.plugin() else {
            ui.label(RichText::new(self.i18n.tr("canvas-empty")).weak());
            return;
        };
        let id = plugin.id().to_string();
        let name = plugin.manifest.name.clone();
        let brand = plugin.manifest.brand.clone();
        let credit = plugin.manifest.credit.clone();
        let controls = plugin.manifest.controls.clone();
        let inputs = plugin.defaults.clone();
        let total: usize = controls.iter().map(|c| c.parts().len()).sum();
        let done: usize = controls
            .iter()
            .map(|c| {
                c.parts()
                    .iter()
                    .filter(|p| inputs.get(&c.id, p).is_some())
                    .count()
            })
            .sum();

        ui.heading(&name);
        ui.label(RichText::new(&brand).weak());
        if let Some(credit) = credit {
            ui.label(RichText::new(credit).small().weak());
        }
        ui.add_space(style::SPACE_S);
        use gob_i18n::FluentValue;
        ui.label(self.i18n.trf(
            "panel-mapped",
            &[
                ("done", FluentValue::from(done)),
                ("total", FluentValue::from(total)),
            ],
        ));
        let fraction = if total == 0 {
            0.0
        } else {
            done as f32 / total as f32
        };
        ui.add(egui::ProgressBar::new(fraction).desired_height(6.0));
        if !self.store.is_own(&id) {
            ui.add_space(style::SPACE_XS);
            ui.label(
                RichText::new(self.i18n.trf(
                    "panel-copy",
                    &[(
                        "path",
                        FluentValue::from(self.store.write_dir.display().to_string()),
                    )],
                ))
                .small()
                .weak(),
            );
        }
        ui.add_space(style::SPACE_M);
        ui.separator();

        let editor_height = if self.selected.is_some() { 330.0 } else { 0.0 };
        let list_height = (ui.available_height() - editor_height).max(120.0);
        ui.label(RichText::new(self.i18n.tr("panel-controls")).strong());
        egui::ScrollArea::vertical()
            .id_salt("controls")
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (n, c) in controls.iter().enumerate() {
                    let parts = c.parts();
                    let mapped = parts
                        .iter()
                        .filter(|p| inputs.get(&c.id, p).is_some())
                        .count();
                    let selected = self.selected.as_deref() == Some(c.id.as_str());
                    let text = format!("{}  {}", n + 1, c.name);
                    let r = ui
                        .horizontal(|ui| {
                            let r = ui.selectable_label(selected, text);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let done = mapped == parts.len();
                                    let mark = if done { icon::CHECK } else { "" };
                                    ui.label(
                                        RichText::new(format!("{mark} {mapped}/{}", parts.len()))
                                            .weak(),
                                    );
                                },
                            );
                            r
                        })
                        .inner;
                    if r.clicked() {
                        self.selected = Some(c.id.clone());
                        if c.view != self.view {
                            self.view = c.view.clone();
                        }
                    }
                }
            });

        let Some(selected) = self.selected.clone() else {
            return;
        };
        let Some(control) = controls.iter().find(|c| c.id == selected).cloned() else {
            self.selected = None;
            return;
        };
        ui.separator();
        self.editor(ui, control);
    }

    fn editor(&mut self, ui: &mut egui::Ui, control: Control) {
        let id = control.id.clone();
        let mut changed: Option<Control> = None;

        // Name: written when the field is left.
        ui.horizontal(|ui| {
            ui.label(self.i18n.tr("control-name"));
            let text = match &mut self.name_edit {
                Some((for_id, text)) if *for_id == id => text,
                other => {
                    *other = Some((id.clone(), control.name.clone()));
                    &mut other.as_mut().expect("just set").1
                }
            };
            let r = ui.add(style::text_field(text).desired_width(f32::INFINITY));
            if r.lost_focus() && !text.trim().is_empty() && text.trim() != control.name {
                let mut c = control.clone();
                c.name = text.trim().to_string();
                changed = Some(c);
            }
        });

        // Kind.
        ui.horizontal(|ui| {
            ui.label(self.i18n.tr("control-kind"));
            let mut kind = control.kind;
            egui::ComboBox::from_id_salt(("kind", &id))
                .selected_text(self.kind_label(kind))
                .show_ui(ui, |ui| {
                    for k in KINDS {
                        ui.selectable_value(&mut kind, k, self.kind_label(k));
                    }
                });
            if kind != control.kind {
                let mut c = control.clone();
                c.kind = kind;
                c.parts = None;
                c.look = None;
                changed = Some(c);
            }
        });

        // Extra parts and looks the kind may have.
        let parts = control.parts();
        match control.kind {
            Kind::Hat => {
                let mut push = parts.contains(&"push");
                if ui
                    .add(style::switch(&mut push, self.i18n.tr("control-push")))
                    .changed()
                {
                    let mut c = control.clone();
                    c.parts = push.then(|| {
                        ["up", "right", "down", "left", "push"]
                            .map(String::from)
                            .to_vec()
                    });
                    changed = Some(c);
                }
                let mut mini = control.look == Some(Look::Stick);
                if ui
                    .add(style::switch(&mut mini, self.i18n.tr("control-mini-stick")))
                    .changed()
                {
                    let mut c = control.clone();
                    c.look = mini.then_some(Look::Stick);
                    changed = Some(c);
                }
            }
            Kind::Stick => {
                let mut twist = parts.contains(&"twist");
                if ui
                    .add(style::switch(&mut twist, self.i18n.tr("control-twist")))
                    .changed()
                {
                    let mut c = control.clone();
                    c.parts = twist.then(|| ["x", "y", "twist"].map(String::from).to_vec());
                    changed = Some(c);
                }
            }
            _ => {}
        }
        ui.horizontal(|ui| {
            ui.label(self.i18n.tr("control-side"));
            let mut side = control.side;
            ui.selectable_value(&mut side, Side::Left, self.i18n.tr("side-left"));
            ui.selectable_value(&mut side, Side::Right, self.i18n.tr("side-right"));
            if side != control.side {
                let mut c = control.clone();
                c.side = side;
                changed = Some(c);
            }
        });

        // Each part's input.
        ui.add_space(style::SPACE_S);
        let plugin_inputs = self
            .plugin()
            .map(|p| p.defaults.clone())
            .unwrap_or_default();
        let mut clear: Option<String> = None;
        egui::Grid::new(("parts", &id))
            .num_columns(3)
            .spacing([style::SPACE_M, style::SPACE_XS])
            .show(ui, |ui| {
                for part in &parts {
                    let label = match self.part_label(part) {
                        l if l.is_empty() => self.kind_label(control.kind),
                        l => l,
                    };
                    ui.label(label);
                    match plugin_inputs.get(&id, part) {
                        Some(input) => {
                            let text = RichText::new(self.input_label(input));
                            let text = if self.is_lit(input) {
                                text.color(style::accent(ui)).strong()
                            } else {
                                text
                            };
                            ui.label(text);
                            if style::icon_button(ui, true, icon::X, &self.i18n.tr("part-clear"))
                                .clicked()
                            {
                                clear = Some(part.to_string());
                            }
                        }
                        None => {
                            ui.label(RichText::new(self.i18n.tr("part-empty")).weak());
                            ui.label("");
                        }
                    }
                    ui.end_row();
                }
            });
        if let Some(part) = clear {
            self.unassign(&id, &part);
        }

        ui.add_space(style::SPACE_M);
        if style::icon_button_colored(
            ui,
            true,
            icon::TRASH,
            style::RED,
            &self.i18n.tr("control-delete"),
        )
        .clicked()
        {
            self.delete_control(&id);
            return;
        }

        if let Some(c) = changed {
            // Parts the new kind lacks lose their inputs.
            let keep: Vec<String> = c.parts().iter().map(|p| p.to_string()).collect();
            for part in &parts {
                if !keep.iter().any(|k| k == part) && plugin_inputs.get(&id, part).is_some() {
                    self.unassign(&id, part);
                }
            }
            self.put_control(&id, c);
            self.name_edit = None;
        }
    }
}
