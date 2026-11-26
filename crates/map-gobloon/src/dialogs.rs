//! New device (brand, name, picture, the connected controller it is for) and new control
//! (name and kind of a control dropped on an empty spot of the picture).

use std::path::PathBuf;

use eframe::egui;
use gob_devices::edit::slug;
use gob_devices::{Control, Input, Kind, Side};
use gob_ui as style;
use style::icon;

use crate::app::{KINDS, MapApp};

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

pub struct NewControl {
    at: [f32; 2],
    view: String,
    /// Picture width: controls on its right half get their callout on the right.
    width: f32,
    name: String,
    kind: Kind,
    input: Option<Input>,
}

impl NewControl {
    pub fn new(app: &MapApp, at: [f32; 2], width: f32, input: Option<Input>) -> Self {
        let kind = match input {
            Some(Input::Axis(_)) => Kind::Axis,
            Some(Input::Hat(_) | Input::HatDirection(..)) => Kind::Hat,
            _ => Kind::Button,
        };
        Self {
            at,
            view: app.view.clone(),
            width,
            name: input.map(|i| app.input_label(i)).unwrap_or_default(),
            kind,
            input,
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

    pub fn new_control_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.new_control.take() else {
            return;
        };
        let mut result = None;
        style::modal(ctx, "new-control").show(ctx, |ui| {
            ui.set_width(380.0);
            style::dialog_title(ui, Some((icon::PLUS, None)), &self.i18n.tr("newc-title"));
            egui::Grid::new("new-control")
                .num_columns(2)
                .spacing([style::SPACE_M, style::SPACE_S])
                .show(ui, |ui| {
                    ui.label(self.i18n.tr("control-name"));
                    let r = ui.add(style::text_field(&mut form.name).desired_width(240.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        result = Some(0);
                    }
                    ui.end_row();
                    ui.label(self.i18n.tr("control-kind"));
                    egui::ComboBox::from_id_salt("new-control-kind")
                        .selected_text(self.kind_label(form.kind))
                        .show_ui(ui, |ui| {
                            for k in KINDS {
                                ui.selectable_value(&mut form.kind, k, self.kind_label(k));
                            }
                        });
                    ui.end_row();
                });
            let ready = !slug(&form.name).is_empty();
            if !ready && result == Some(0) {
                result = None;
            }
            if let Some(clicked) = style::dialog_buttons(
                ui,
                &[
                    style::DialogButton::primary(self.i18n.tr("newc-create")).enabled(ready),
                    style::DialogButton::secondary(self.i18n.tr("cancel")),
                ],
            ) {
                result = Some(clicked);
            }
        });
        match result {
            Some(0) => self.add_control(form),
            Some(_) => {}
            None => self.new_control = Some(form),
        }
    }

    fn add_control(&mut self, form: NewControl) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let base = slug(&form.name).replace('-', "_");
        let mut id = base.clone();
        let mut n = 2;
        while plugin.control(&id).is_some() {
            id = format!("{base}_{n}");
            n += 1;
        }
        let control = Control {
            id: id.clone(),
            name: form.name.trim().to_string(),
            kind: form.kind,
            parts: None,
            view: form.view,
            at: [form.at[0].round(), form.at[1].round()],
            side: if form.at[0] > form.width / 2.0 {
                Side::Right
            } else {
                Side::Left
            },
            group: None,
            look: None,
        };
        self.put_control(&id, control);
        if let Some(input) = form.input {
            self.assign_best(&id, input);
        }
        self.selected = Some(id);
    }
}
