//! New device (brand, name, picture and the connected controller it is for) and copying another
//! device's controls onto the one mapped.

use std::path::PathBuf;

use crate::style::{self, icon};
use eframe::egui;
use gob_devices::edit::slug;

use super::Mapper;

pub struct NewDevice {
    brand: String,
    name: String,
    picture: Option<PathBuf>,
    /// The connected controller it is for (instance GUID).
    controller: Option<String>,
}

impl NewDevice {
    /// For connected controller `guid`, named `name`.
    pub fn new(guid: &str, name: String) -> Self {
        Self {
            brand: String::new(),
            name,
            picture: None,
            controller: Some(guid.to_string()),
        }
    }
}

/// Copying another device's controls onto the one mapped.
#[derive(Default)]
pub struct CopyControls {
    source: Option<String>,
    /// For the other hand's device: left and right swap.
    mirror: bool,
}

impl Mapper {
    pub fn new_device_dialog(&mut self, ctx: &egui::Context, i18n: &gob_i18n::I18n) {
        let Some(mut form) = self.new_device.take() else {
            return;
        };
        let brands = self.store.brands();
        let mut result = None;
        style::modal(ctx, "new-device").show(ctx, |ui| {
            ui.set_width(460.0);
            style::dialog_title(
                ui,
                Some((icon::PLUS_CIRCLE, None)),
                &i18n.tr("map-new-title"),
            );
            egui::Grid::new("new-device")
                .num_columns(2)
                .spacing([style::SPACE_M, style::SPACE_S])
                .show(ui, |ui| {
                    ui.label(i18n.tr("map-new-brand"));
                    ui.horizontal(|ui| {
                        ui.add(style::text_field(&mut form.brand).desired_width(260.0));
                        ui.menu_button(icon::CARET_DOWN, |ui| {
                            for b in &brands {
                                if ui.button(b).clicked() {
                                    form.brand = b.clone();
                                    ui.close();
                                }
                            }
                        });
                    });
                    ui.end_row();
                    ui.label(i18n.tr("map-new-name"));
                    ui.add(style::text_field(&mut form.name).desired_width(300.0));
                    ui.end_row();
                    ui.label(i18n.tr("map-new-picture"));
                    ui.horizontal(|ui| {
                        let shown = form
                            .picture
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ui.label(shown);
                        if ui.button(i18n.tr("map-new-pick")).clicked()
                            && let Some(file) = rfd::FileDialog::new()
                                .add_filter("PNG, JPEG, SVG", &["png", "jpg", "jpeg", "svg"])
                                .pick_file()
                        {
                            form.picture = Some(file);
                        }
                    });
                    ui.end_row();
                    ui.label(i18n.tr("map-new-controller"));
                    let none = i18n.tr("map-new-no-controller");
                    let current = form
                        .controller
                        .as_deref()
                        .and_then(|g| self.devices.iter().find(|d| d.info.guid == g))
                        .map_or(none.clone(), |d| d.info.name.clone());
                    egui::ComboBox::from_id_salt("new-controller")
                        .selected_text(current)
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.controller, None, none);
                            for d in &self.devices {
                                ui.selectable_value(
                                    &mut form.controller,
                                    Some(d.info.guid.clone()),
                                    &d.info.name,
                                );
                            }
                        });
                    ui.end_row();
                });
            ui.label(
                egui::RichText::new(i18n.tr("map-new-picture-hint"))
                    .small()
                    .weak(),
            );
            let ready = !slug(&form.brand).is_empty()
                && !slug(&form.name).is_empty()
                && form.picture.is_some();
            result = style::dialog_buttons(
                ui,
                &[
                    style::DialogButton::primary(i18n.tr("map-new-create")).enabled(ready),
                    style::DialogButton::secondary(i18n.tr("cancel")),
                ],
            );
        });
        match result {
            Some(0) => {
                let device = form
                    .controller
                    .as_deref()
                    .and_then(|g| self.devices.iter().find(|d| d.info.guid == g))
                    .map(|d| (d.info.vendor_id, d.info.name.clone()));
                let created = self.store.create(
                    form.brand.trim(),
                    form.name.trim(),
                    form.picture.as_deref().expect("checked"),
                    device.as_ref().map(|(vid, name)| (*vid, name.as_str())),
                );
                match created {
                    Ok(id) => match form.controller.clone() {
                        Some(guid) => self.edit(&guid, &id),
                        None => self.open(&id),
                    },
                    Err(e) => {
                        self.error = Some(e);
                        self.new_device = Some(form);
                    }
                }
            }
            Some(_) => {}
            None => self.new_device = Some(form),
        }
    }

    pub fn copy_dialog(&mut self, ctx: &egui::Context, i18n: &gob_i18n::I18n) {
        let Some(mut form) = self.copy.take() else {
            return;
        };
        let Some(to) = self.plugin.clone() else {
            return;
        };
        let mut devices: Vec<(String, String)> = self
            .store
            .catalog
            .plugins
            .iter()
            .filter(|p| p.id() != to)
            .map(|p| {
                let name = format!("{} {}", p.manifest.brand, p.manifest.name);
                (p.id().to_string(), name)
            })
            .collect();
        devices.sort_by_key(|(_, name)| name.to_lowercase());
        let replaced = self.store.get(&to).map_or(0, |p| p.manifest.controls.len());
        let mut result = None;
        style::modal(ctx, "copy-controls").show(ctx, |ui| {
            ui.set_width(440.0);
            style::dialog_title(ui, Some((icon::COPY, None)), &i18n.tr("map-copy-title"));
            ui.horizontal(|ui| {
                ui.label(i18n.tr("map-copy-from"));
                let shown = form
                    .source
                    .as_deref()
                    .and_then(|id| devices.iter().find(|(d, _)| d == id))
                    .map_or_else(String::new, |(_, name)| name.clone());
                egui::ComboBox::from_id_salt("copy-source")
                    .selected_text(shown)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for (id, name) in &devices {
                            ui.selectable_value(&mut form.source, Some(id.clone()), name);
                        }
                    });
            });
            ui.add_space(style::SPACE_S);
            ui.add(style::switch(&mut form.mirror, i18n.tr("map-copy-mirror")));
            if replaced > 0 {
                use gob_i18n::FluentValue;
                ui.add_space(style::SPACE_S);
                ui.label(
                    egui::RichText::new(
                        i18n.trf("map-copy-replaces", &[("n", FluentValue::from(replaced))]),
                    )
                    .color(style::RED),
                );
            }
            result = style::dialog_buttons(
                ui,
                &[
                    style::DialogButton::primary(i18n.tr("map-copy-do"))
                        .enabled(form.source.is_some()),
                    style::DialogButton::secondary(i18n.tr("cancel")),
                ],
            );
        });
        match result {
            Some(0) => {
                let from = form.source.clone().expect("checked");
                match self.store.copy_controls(&from, &to, form.mirror) {
                    Ok(()) => self.open(&to),
                    Err(e) => self.error = Some(e),
                }
            }
            Some(_) => {}
            None => self.copy = Some(form),
        }
    }
}
