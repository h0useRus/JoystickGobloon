//! The side panel: the device, its placed inputs by number, and the selected one's name.

use crate::style::{self, icon};
use eframe::egui::{self, RichText};

use super::{Mapper, input_label};

impl Mapper {
    pub fn panel(&mut self, ui: &mut egui::Ui, i18n: &gob_i18n::I18n) {
        ui.add_space(style::SPACE_M);
        let Some(plugin) = self.plugin() else {
            ui.label(RichText::new(i18n.tr("map-canvas-empty")).weak());
            return;
        };
        let id = plugin.id().to_string();
        let name = plugin.manifest.name.clone();
        let brand = plugin.manifest.brand.clone();
        let credit = plugin.manifest.credit.clone();
        let controls = plugin.manifest.controls.clone();

        ui.heading(&name);
        ui.label(RichText::new(&brand).weak());
        if let Some(credit) = credit {
            ui.label(RichText::new(credit).small().weak());
        }
        if !self.store.is_own(&id) {
            use gob_i18n::FluentValue;
            ui.add_space(style::SPACE_XS);
            ui.label(
                RichText::new(i18n.trf(
                    "map-panel-copy",
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
        if controls.is_empty() {
            ui.label(RichText::new(i18n.tr("map-panel-empty")).weak());
            return;
        }

        let editor_height = if self.selected.is_some() { 120.0 } else { 0.0 };
        let list_height = (ui.available_height() - editor_height).max(120.0);
        let accent = style::accent(ui);
        egui::ScrollArea::vertical()
            .id_salt("controls")
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("controls")
                    .num_columns(3)
                    .spacing([style::SPACE_M, style::SPACE_XS])
                    .show(ui, |ui| {
                        for (n, c) in controls.iter().enumerate() {
                            let inputs = self.inputs_of(c);
                            let lit = inputs.iter().any(|i| self.is_lit(*i));
                            let number = RichText::new(format!("{}", n + 1)).strong();
                            ui.label(if lit { number.color(accent) } else { number });
                            let selected = self.selected.as_deref() == Some(c.id.as_str());
                            if ui.selectable_label(selected, &c.name).clicked() {
                                // A name being typed for the control selected before is kept:
                                // its field is not drawn again to lose its focus.
                                self.commit_name();
                                self.selected = Some(c.id.clone());
                                self.view = c.view.clone();
                            }
                            let shown: Vec<String> =
                                inputs.iter().map(|i| input_label(i18n, *i)).collect();
                            let shown = shown.join(", ");
                            // A name that only repeats the input shows it once.
                            let shown = if shown == c.name {
                                String::new()
                            } else {
                                shown
                            };
                            ui.label(RichText::new(shown).weak());
                            ui.end_row();
                        }
                    });
            });

        let Some(selected) = self.selected.clone() else {
            return;
        };
        let Some(control) = controls.iter().find(|c| c.id == selected).cloned() else {
            self.selected = None;
            return;
        };
        ui.separator();
        // The name: written when the field is left.
        ui.horizontal(|ui| {
            let label = ui.label(i18n.tr("map-control-name"));
            let text = match &mut self.name_edit {
                Some((for_id, text)) if *for_id == control.id => text,
                other => {
                    *other = Some((control.id.clone(), control.name.clone()));
                    &mut other.as_mut().expect("just set").1
                }
            };
            let r = ui
                .add(style::text_field(text).desired_width(f32::INFINITY))
                .labelled_by(label.id);
            if r.lost_focus() && !text.trim().is_empty() && text.trim() != control.name {
                let mut c = control.clone();
                c.name = text.trim().to_string();
                self.put_control(&control.id, c);
                self.name_edit = None;
            }
        });
        ui.add_space(style::SPACE_S);
        ui.horizontal(|ui| {
            if control.kind == gob_devices::Kind::Button
                && ui.button(i18n.tr("map-tag-make-hat")).clicked()
            {
                self.make_hat(&control.id);
            }
            if (self.is_button_hat(&control) || control.kind == gob_devices::Kind::Stick)
                && ui.button(i18n.tr("map-tag-split")).clicked()
            {
                self.split(&control.id);
            }
        });
        if style::icon_button_colored(
            ui,
            true,
            icon::TRASH,
            style::RED,
            &i18n.tr("map-tag-remove"),
        )
        .clicked()
        {
            self.delete_control(&control.id);
        }
    }
}

impl Mapper {
    /// Writes the name typed for a control, if it changed.
    fn commit_name(&mut self) {
        let Some((id, text)) = self.name_edit.take() else {
            return;
        };
        let name = text.trim();
        let Some(mut c) = self.plugin().and_then(|p| p.control(&id)).cloned() else {
            return;
        };
        if !name.is_empty() && name != c.name {
            c.name = name.to_string();
            self.put_control(&id, c);
        }
    }
}
