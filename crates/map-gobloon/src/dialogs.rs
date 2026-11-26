//! New device: brand, name, picture and the connected controller it is for.

use std::path::PathBuf;

use eframe::egui;
use gob_devices::edit::slug;
use gob_ui as style;
use style::icon;

use crate::app::MapApp;

pub struct NewDevice {
    brand: String,
    name: String,
    picture: Option<PathBuf>,
    /// The connected controller it is for (instance GUID).
    controller: Option<String>,
}

impl NewDevice {
    pub fn new(app: &MapApp) -> Self {
        Self {
            brand: app.brand.clone().unwrap_or_default(),
            name: app
                .device_state()
                .map(|d| d.info.name.clone())
                .unwrap_or_default(),
            picture: None,
            controller: app.device.clone(),
        }
    }
}

impl MapApp {
    pub fn new_device_dialog(&mut self, ctx: &egui::Context) {
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
                &self.i18n.tr("new-title"),
            );
            egui::Grid::new("new-device")
                .num_columns(2)
                .spacing([style::SPACE_M, style::SPACE_S])
                .show(ui, |ui| {
                    ui.label(self.i18n.tr("new-brand"));
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
                    ui.label(self.i18n.tr("new-name"));
                    ui.add(style::text_field(&mut form.name).desired_width(300.0));
                    ui.end_row();
                    ui.label(self.i18n.tr("new-picture"));
                    ui.horizontal(|ui| {
                        let shown = form
                            .picture
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ui.label(shown);
                        if ui.button(self.i18n.tr("new-pick")).clicked()
                            && let Some(file) = rfd::FileDialog::new()
                                .add_filter("PNG, JPEG, SVG", &["png", "jpg", "jpeg", "svg"])
                                .pick_file()
                        {
                            form.picture = Some(file);
                        }
                    });
                    ui.end_row();
                    ui.label(self.i18n.tr("new-controller"));
                    let none = self.i18n.tr("new-no-controller");
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
                egui::RichText::new(self.i18n.tr("new-picture-hint"))
                    .small()
                    .weak(),
            );
            let ready = !slug(&form.brand).is_empty()
                && !slug(&form.name).is_empty()
                && form.picture.is_some();
            result = style::dialog_buttons(
                ui,
                &[
                    style::DialogButton::primary(self.i18n.tr("new-create")).enabled(ready),
                    style::DialogButton::secondary(self.i18n.tr("cancel")),
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
                    Ok(id) => {
                        if let Some(guid) = form.controller.clone() {
                            self.choose_device(Some(guid));
                        }
                        self.open(&id);
                    }
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
}
