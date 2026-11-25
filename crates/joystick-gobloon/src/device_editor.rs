//! Debug builds only: places a device plugin's hotspots on its picture. Drag a spot, then save:
//! only the `at = [x, y]` lines of `device.gob` change. A developer tool, so not translated
//! (`JOYSTICK_GOBLOON_START_PAGE=device-editor`).

use std::collections::BTreeMap;

use eframe::egui;

use crate::device_pictures::{DevicePictures, PartLine, callouts};
use crate::style;

#[derive(Default)]
pub struct DeviceEditor {
    plugin: String,
    view: String,
    /// Moved spots not saved yet: control id -> position.
    moved: BTreeMap<String, [f32; 2]>,
    status: String,
}

impl DeviceEditor {
    /// The editor's content; false = close it.
    pub fn show(&mut self, ui: &mut egui::Ui, pictures: &mut DevicePictures) -> bool {
        let mut open = true;
        let ids: Vec<(String, String)> = pictures
            .catalog()
            .plugins
            .iter()
            .map(|p| (p.id().to_string(), p.manifest.name.clone()))
            .collect();
        if ids.is_empty() {
            ui.label("No device plugins found.");
            return !ui.button("Close").clicked();
        }
        if !ids.iter().any(|(id, _)| *id == self.plugin) {
            self.plugin = ids[0].0.clone();
            self.view.clear();
            self.moved.clear();
        }
        let views: Vec<(String, String)> = pictures
            .catalog()
            .get(&self.plugin)
            .map(|p| {
                p.manifest
                    .views
                    .iter()
                    .map(|v| (v.id.clone(), v.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if !views.iter().any(|(id, _)| *id == self.view)
            && let Some((id, _)) = views.first()
        {
            self.view = id.clone();
        }

        ui.horizontal(|ui| {
            let name = ids
                .iter()
                .find(|(id, _)| *id == self.plugin)
                .map_or("", |(_, n)| n.as_str());
            egui::ComboBox::from_id_salt("device-editor-plugin")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for (id, name) in &ids {
                        if ui.selectable_label(*id == self.plugin, name).clicked() {
                            self.plugin = id.clone();
                            self.view.clear();
                            self.moved.clear();
                        }
                    }
                });
            for (id, name) in &views {
                if ui.selectable_label(*id == self.view, name).clicked() {
                    self.view = id.clone();
                }
            }
            ui.separator();
            let dirty = !self.moved.is_empty();
            if style::icon_button(ui, dirty, style::icon::FLOPPY_DISK, "Save device.gob").clicked()
            {
                self.save(pictures);
            }
            if style::icon_button(
                ui,
                dirty,
                style::icon::ARROW_COUNTER_CLOCKWISE,
                "Undo moves",
            )
            .clicked()
            {
                self.moved.clear();
            }
            if style::icon_button(ui, true, style::icon::ARROWS_CLOCKWISE, "Reload plugins")
                .clicked()
            {
                pictures.reload();
                self.moved.clear();
            }
            if style::icon_button(ui, true, style::icon::X, "Close").clicked() {
                open = false;
            }
            ui.label(egui::RichText::new(&self.status).weak());
        });
        ui.separator();

        let dark = ui.visuals().dark_mode;
        let Some((texture, size)) = pictures.picture(ui.ctx(), &self.plugin, &self.view, dark)
        else {
            ui.label("This view has no picture.");
            return open;
        };
        let Some(plugin) = pictures.catalog().get(&self.plugin) else {
            return open;
        };
        // One callout per control (groups not merged): each spot can be dragged on its own.
        let built = callouts(plugin, &self.view, None, false, |c, part| {
            let input = plugin
                .input_of(&c.id, part)
                .map_or("?".to_string(), |i| i.to_string());
            PartLine {
                text: format!("{}  {input}", c.id),
                ..Default::default()
            }
        });
        let mut list = built.list.clone();
        let ids: Vec<String> = (0..list.len())
            .map(|i| {
                built
                    .controls(i)
                    .first()
                    .map_or(String::new(), |c| c.to_string())
            })
            .collect();
        for (callout, id) in list.iter_mut().zip(&ids) {
            if let Some([x, y]) = self.moved.get(id) {
                callout.at = egui::pos2(*x, *y);
                callout.lit = true;
            }
        }
        let credit = plugin.manifest.credit.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let height = ui.available_height().max(300.0);
            let response =
                style::DevicePicture::new("device-editor", egui::Image::new(&texture), size, &list)
                    .caption(credit.as_deref())
                    .max_height(height)
                    .draggable(true)
                    .show(ui);
            for (i, r) in response.callouts.iter().enumerate() {
                if r.dragged()
                    && let Some(pos) = r.interact_pointer_pos()
                {
                    let at = response.to_picture(pos);
                    let at = [at.x.clamp(0.0, size.x), at.y.clamp(0.0, size.y)];
                    self.moved.insert(ids[i].clone(), at);
                }
            }
            if let Some(i) = response.hovered {
                let at = list[i].at;
                self.status = format!("{} at {:.0}, {:.0}", ids[i], at.x, at.y);
            }
        });
        open
    }

    fn save(&mut self, pictures: &mut DevicePictures) {
        let Some(plugin) = pictures.catalog().get(&self.plugin) else {
            return;
        };
        let path = plugin.dir.join(gob_devices::plugin::MANIFEST);
        let result = std::fs::read_to_string(&path).and_then(|text| {
            let at: Vec<(&str, [f32; 2])> = self
                .moved
                .iter()
                .map(|(id, at)| (id.as_str(), *at))
                .collect();
            gob_core::file::write_whole(&path, gob_devices::with_hotspots(&text, &at).as_bytes())
        });
        self.status = match result {
            Ok(()) => {
                self.moved.clear();
                pictures.reload();
                format!("saved {}", path.display())
            }
            Err(e) => format!("{}: {e}", path.display()),
        };
    }
}
