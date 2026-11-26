//! Device pictures and mapping a controller onto its picture, on the Input viewer's Picture view.
//!
//! [`Mapper`] holds the device plugins (pictures of controllers with their controls placed) and
//! edits them: every button, axis or hat the user works on the controller gets the next number,
//! waiting in the picture's top left corner until it is dragged to the spot of the picture where
//! that control is (`canvas`). A placed number is a control of the device plugin with that one
//! input; buttons gather into hats and axes into sticks. `panel` lists the placed controls,
//! `dialogs` makes a new device from a picture and copies the controls of another device, `share`
//! packs a device's picture and mapping into a file to share and takes such files in.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui;
use gob_devices::{Control, Detector, DevicePlugin, Input, Kind, Palette, Side};
use gob_i18n::I18n;
use gob_input::{DeviceInfo, DeviceState};

use crate::style::{self, icon};

mod canvas;
mod dialogs;
mod panel;
mod share;
mod store;
#[cfg(test)]
mod tests;

use dialogs::{CopyControls, NewDevice};
use store::Store;

pub use gob_devices::draw::HAT_PARTS;
/// How far around a button (a share of the picture's width) "Make it a hat" takes buttons in.
const HAT_GATHER: f32 = 0.05;
/// The parts of a stick made of axes.
pub const STICK_PARTS: [&str; 3] = ["x", "y", "twist"];

/// How long an axis' tag stays lit after the axis moved.
pub const AXIS_LIT: Duration = Duration::from_millis(700);

pub struct Mapper {
    pub store: Store,
    /// Mapping now (else only the pictures are shown).
    pub editing: bool,
    /// The device plugin edited.
    pub plugin: Option<String>,
    /// Its view shown.
    pub view: String,
    /// Connected controllers (not our virtual ones), as read this frame.
    pub devices: Vec<DeviceState>,
    /// The controller mapped (instance GUID).
    pub device: Option<String>,
    detector: Option<Detector>,
    /// Inputs worked but not placed yet, in the order they came: they get the next numbers.
    pub pending: Vec<Input>,
    /// When each input last counted (lights axes' tags for a moment).
    pub seen: HashMap<Input, Instant>,
    pub selected: Option<String>,
    /// The tag being dragged to a new place: (control id, its spot in picture units).
    pub moving: Option<(String, [f32; 2])>,
    /// The control a tag's menu is open for.
    pub menu: Option<String>,
    /// The selected control's name while it is typed.
    pub name_edit: Option<(String, String)>,
    /// (plugin id, view id, dark) -> texture and the picture's size in control units.
    textures: HashMap<(String, String, bool), (egui::TextureHandle, egui::Vec2)>,
    pub new_device: Option<NewDevice>,
    pub copy: Option<CopyControls>,
    /// A shared file unpacked that replaces the user's own mapping: asked first.
    import: Option<share::Unpacked>,
    pub error: Option<String>,
    /// Where things were drawn last frame (tests drag between them).
    pub layout: Layout,
}

/// Where the picture and the waiting numbers were drawn last frame.
#[derive(Debug, Default, Clone)]
pub struct Layout {
    /// The picture's rectangle on screen and its scale (screen points per picture unit).
    pub picture: Option<(egui::Rect, f32)>,
    pub pending: Vec<(Input, egui::Rect)>,
}

#[cfg(test)]
impl Layout {
    /// A spot of the picture on screen.
    pub fn to_screen(&self, at: [f32; 2]) -> Option<egui::Pos2> {
        let (rect, scale) = self.picture?;
        Some(rect.min + egui::vec2(at[0], at[1]) * scale)
    }

    pub fn pending(&self, input: Input) -> Option<egui::Pos2> {
        self.pending
            .iter()
            .find(|(i, _)| *i == input)
            .map(|(_, r)| r.center())
    }
}

impl Default for Mapper {
    fn default() -> Self {
        Self::with(Store::new())
    }
}

impl Mapper {
    pub fn with(store: Store) -> Self {
        Self {
            store,
            editing: false,
            plugin: None,
            view: gob_devices::plugin::MAIN_VIEW.into(),
            devices: Vec::new(),
            device: None,
            detector: None,
            pending: Vec::new(),
            seen: HashMap::new(),
            selected: None,
            moving: None,
            menu: None,
            name_edit: None,
            textures: HashMap::new(),
            new_device: None,
            copy: None,
            import: None,
            error: None,
            layout: Layout::default(),
        }
    }

    /// Every frame: the controllers' readings, what the mapped one did since the last frame,
    /// and the plugins again when their files changed.
    pub fn update(&mut self, devices: &[DeviceState]) {
        self.devices = devices
            .iter()
            .filter(|d| !d.info.is_own() && d.info.guid != gob_core::LOGICAL_DEVICE_GUID)
            .cloned()
            .collect();
        if self.store.reload_if_changed() {
            self.textures.clear();
        }
        if !self.editing {
            return;
        }
        let Some(state) = self.device_state().cloned() else {
            self.detector = None;
            return;
        };
        let detector = self.detector.get_or_insert_with(|| Detector::new(&state));
        for input in detector.poll(&state) {
            self.note(input);
        }
    }

    /// The plugin for a connected device, if there is one.
    pub fn plugin_for(&self, info: &DeviceInfo) -> Option<&DevicePlugin> {
        if info.is_own() {
            return None;
        }
        self.store
            .catalog
            .find(info.vendor_id, info.product_id, &info.name)
    }

    pub fn plugin(&self) -> Option<&DevicePlugin> {
        self.store.get(self.plugin.as_deref()?)
    }

    /// Starts mapping controller `guid` onto plugin `id`.
    pub fn edit(&mut self, guid: &str, id: &str) {
        self.device = Some(guid.to_string());
        self.detector = self.device_state().map(Detector::new);
        self.seen.clear();
        self.open(id);
        self.editing = true;
    }

    /// Mapping is done.
    pub fn done(&mut self) {
        self.editing = false;
        self.detector = None;
        self.pending.clear();
        self.moving = None;
        self.menu = None;
    }

    /// Opens device plugin `id`.
    pub fn open(&mut self, id: &str) {
        let Some(plugin) = self.store.get(id) else {
            return;
        };
        if self.plugin.as_deref() != Some(id)
            || !plugin.manifest.views.iter().any(|v| v.id == self.view)
        {
            self.view = plugin
                .manifest
                .views
                .first()
                .map_or_else(|| gob_devices::plugin::MAIN_VIEW.into(), |v| v.id.clone());
        }
        self.plugin = Some(id.to_string());
        self.selected = None;
        self.name_edit = None;
        self.pending.clear();
    }

    pub fn device_state(&self) -> Option<&DeviceState> {
        let guid = self.device.as_deref()?;
        self.devices.iter().find(|d| d.info.guid == guid)
    }

    /// The user worked `input`: a new one gets the next number (a hat is one number for all its
    /// directions); one already placed or waiting only lights up.
    pub fn note(&mut self, input: Input) {
        let input = match input {
            Input::HatDirection(n, _) => Input::Hat(n),
            other => other,
        };
        self.seen.insert(input, Instant::now());
        if self.plugin.is_none() || self.pending.contains(&input) || self.placed(input).is_some() {
            return;
        }
        self.pending.push(input);
    }

    /// The control `input` is placed as.
    pub fn placed(&self, input: Input) -> Option<String> {
        let plugin = self.plugin()?;
        plugin
            .manifest
            .controls
            .iter()
            .find(|c| {
                // A hat placed by its directions counts as the whole hat.
                self.inputs_of(c).iter().any(|i| match (*i, input) {
                    (Input::HatDirection(m, _), Input::Hat(n)) => m == n,
                    (i, input) => i == input,
                })
            })
            .map(|c| c.id.clone())
    }

    /// The inputs control `id` has.
    pub fn inputs_of(&self, control: &Control) -> Vec<Input> {
        let Some(plugin) = self.plugin() else {
            return Vec::new();
        };
        if let Some(gob_devices::Entry::One(input)) = plugin.defaults.0.get(&control.id) {
            return vec![*input];
        }
        control
            .parts()
            .iter()
            .filter_map(|p| plugin.defaults.get(&control.id, p))
            .collect()
    }

    /// Whether `input` is worked now: held, or (an axis) moved a moment ago.
    pub fn is_lit(&self, input: Input) -> bool {
        let held = self
            .device_state()
            .is_some_and(|s| gob_devices::is_active(s, input));
        held || matches!(input, Input::Axis(_))
            && self
                .seen
                .get(&input)
                .is_some_and(|t| t.elapsed() < AXIS_LIT)
    }

    /// Where an axis input is now, -1 to 1.
    pub fn axis_value(&self, input: Input) -> Option<f32> {
        gob_devices::axis_value(self.device_state()?, input)
    }

    /// The edited view's picture as a texture and its size in control units.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<(egui::TextureHandle, egui::Vec2)> {
        let id = self.plugin.clone()?;
        let view = self.view.clone();
        self.picture(ctx, &id, &view)
    }

    /// A view's picture for the current theme and its size in control units.
    pub fn picture(
        &mut self,
        ctx: &egui::Context,
        id: &str,
        view: &str,
    ) -> Option<(egui::TextureHandle, egui::Vec2)> {
        let dark = ctx.global_style().visuals.dark_mode;
        let key = (id.to_string(), view.to_string(), dark);
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let picture = self.store.get(id)?.pictures.get(view)?;
        let [_, _, w, h] = picture.bounds()?;
        let palette = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mut rgba = match picture.rasterize(&palette, 2.0) {
            Ok(rgba) => rgba,
            Err(e) => {
                log::warn!("device plugin {id}: picture {view}: {e}");
                return None;
            }
        };
        // The window is transparent: near-black pixels would show what is behind it.
        rgba.lift_blacks();
        let [pw, ph] = rgba.size;
        let image =
            egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], &rgba.pixels);
        let texture = ctx.load_texture(
            format!("device-{id}-{view}-{dark}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        let entry = (texture, egui::vec2(w, h));
        self.textures.insert(key, entry.clone());
        Some(entry)
    }

    /// Shows what failed.
    pub fn report(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            log::error!("device mapping: {e}");
            self.error = Some(e);
        }
    }

    /// Puts waiting `input` on the picture at `at` (picture units, `width` wide): a new control
    /// named after the input, with that input.
    pub fn place(&mut self, input: Input, at: [f32; 2], width: f32) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let plugin_id = plugin.id().to_string();
        let base = gob_devices::edit::slug(&input.to_string()).replace('-', "_");
        let mut id = base.clone();
        let mut n = 2;
        while plugin.control(&id).is_some() {
            id = format!("{base}_{n}");
            n += 1;
        }
        let kind = match input {
            Input::Axis(_) => Kind::Axis,
            Input::Hat(_) | Input::HatDirection(..) => Kind::Hat,
            Input::Button(_) => Kind::Button,
        };
        let control = Control {
            id: id.clone(),
            name: plain_label(input),
            kind,
            parts: None,
            view: self.view.clone(),
            at: [at[0].round(), at[1].round()],
            side: if at[0] > width / 2.0 {
                Side::Right
            } else {
                Side::Left
            },
            group: None,
            look: None,
        };
        let mut inputs = plugin.defaults.clone();
        inputs.remove_input(input);
        inputs.set(&id, "", input);
        let result = self
            .store
            .edit_manifest(&plugin_id, |text| {
                gob_devices::edit::set_control(text, &id, &control)
            })
            .and_then(|()| self.store.save_inputs(&plugin_id, &inputs));
        self.report(result);
        self.pending.retain(|i| *i != input);
        self.selected = Some(id);
    }

    /// Turns control `id` (a button) into a hat of buttons: up, right, down, left and push.
    /// Buttons placed right around it go into the direction they sit in on the picture, and the
    /// button itself, then in the middle, is the push; without such neighbours its button waits
    /// to be dragged onto its direction.
    pub fn make_hat(&mut self, id: &str) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let Some(old) = plugin.control(id).cloned() else {
            return;
        };
        let plugin_id = plugin.id().to_string();
        let own = self.inputs_of(&old);
        let width = plugin
            .pictures
            .get(&old.view)
            .and_then(|p| p.bounds())
            .map_or(500.0, |b| b[2]);
        let reach = width * HAT_GATHER;
        let neighbours: Vec<(String, Input, [f32; 2])> = plugin
            .manifest
            .controls
            .iter()
            .filter(|c| c.id != id && c.view == old.view && c.kind == Kind::Button)
            .filter(|c| (c.at[0] - old.at[0]).hypot(c.at[1] - old.at[1]) <= reach)
            .filter_map(|c| match self.inputs_of(c)[..] {
                [input @ Input::Button(_)] => Some((c.id.clone(), input, c.at)),
                _ => None,
            })
            .collect();
        let directions = directions_of(old.at, &neighbours);
        let mut new_id = "hat".to_string();
        let mut n = 2;
        while plugin.control(&new_id).is_some() {
            new_id = format!("hat_{n}");
            n += 1;
        }
        let hat = Control {
            id: new_id.clone(),
            name: format!("Hat {}", n - 1),
            kind: Kind::Hat,
            parts: Some(HAT_PARTS.map(String::from).to_vec()),
            look: None,
            ..old
        };
        let mut inputs = plugin.defaults.clone();
        inputs.clear(id, "");
        let mut text_change: Vec<String> = Vec::new();
        for (part, (control, input)) in &directions {
            inputs.clear(control, "");
            inputs.set(&new_id, part, *input);
            text_change.push(control.clone());
        }
        let gathered = !directions.is_empty();
        if gathered && let [push] = own[..] {
            inputs.set(&new_id, "push", push);
        }
        let result = self
            .store
            .edit_manifest(&plugin_id, |text| {
                let mut text = gob_devices::edit::set_control(text, id, &hat);
                for control in &text_change {
                    text = gob_devices::edit::remove_control(&text, control);
                }
                text
            })
            .and_then(|()| self.store.save_inputs(&plugin_id, &inputs));
        self.report(result);
        if !gathered {
            for input in own.into_iter().rev() {
                self.pending.retain(|i| *i != input);
                self.pending.insert(0, input);
            }
        }
        self.selected = Some(new_id);
    }

    /// Takes a group (a hat of buttons, a stick of axes) apart: its inputs wait to be placed
    /// again, the group goes.
    pub fn split(&mut self, id: &str) {
        let Some(control) = self.plugin().and_then(|p| p.control(id)).cloned() else {
            return;
        };
        let own = self.inputs_of(&control);
        self.delete_control(id);
        for input in own.into_iter().rev() {
            self.pending.retain(|i| *i != input);
            self.pending.insert(0, input);
        }
    }

    /// Puts `input` on `part` of group `group` (off wherever it was before). A single axis
    /// becomes a stick: its own axis takes the first other part. An input that was on that part
    /// waits to be placed again.
    pub fn place_part(&mut self, input: Input, group: &str, part: &str) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let Some(control) = plugin.control(group).cloned() else {
            return;
        };
        let plugin_id = plugin.id().to_string();
        let mut inputs = plugin.defaults.clone();
        let mut back = Vec::new();
        if control.kind == Kind::Axis {
            let own = self.inputs_of(&control);
            inputs.clear(group, "");
            let stick = Control {
                kind: Kind::Stick,
                parts: Some(STICK_PARTS.map(String::from).to_vec()),
                name: if control.name.starts_with("Axis ") {
                    "Stick".into()
                } else {
                    control.name.clone()
                },
                ..control.clone()
            };
            let result = self.store.edit_manifest(&plugin_id, |text| {
                gob_devices::edit::set_control(text, group, &stick)
            });
            self.report(result);
            if let Some(old) = own.first().filter(|old| **old != input)
                && let Some(free) = STICK_PARTS.into_iter().find(|p| *p != part)
            {
                inputs.set(group, free, *old);
            }
        } else if let Some(previous) = inputs.get(group, part).filter(|p| *p != input) {
            back.push(previous);
        }
        inputs.remove_input(input);
        inputs.set(group, part, input);
        let result = self.store.save_inputs(&plugin_id, &inputs);
        self.report(result);
        self.pending.retain(|i| *i != input);
        for previous in back {
            self.pending.insert(0, previous);
        }
        self.selected = Some(group.to_string());
    }

    /// Moves placed control `from` (one input) into `part` of group `group`; `from` goes.
    pub fn merge_into(&mut self, from: &str, group: &str, part: &str) {
        let Some(control) = self.plugin().and_then(|p| p.control(from)).cloned() else {
            return;
        };
        let [input] = self.inputs_of(&control)[..] else {
            return;
        };
        self.delete_control(from);
        self.place_part(input, group, part);
    }

    /// Whether control `c` is a hat made of buttons (its directions filled one by one).
    pub fn is_button_hat(&self, c: &Control) -> bool {
        c.kind == Kind::Hat
            && c.parts().len() > 1
            && !matches!(
                self.plugin().and_then(|p| p.defaults.0.get(&c.id)),
                Some(gob_devices::Entry::One(_))
            )
    }

    /// The parts of control `c` that `input` can be dropped into: a button into a hat of
    /// buttons, an axis into another axis or a stick.
    pub fn group_parts(&self, c: &Control, input: Input) -> Option<&'static [&'static str]> {
        match input {
            Input::Button(_) if self.is_button_hat(c) => Some(&HAT_PARTS),
            Input::Axis(_) if matches!(c.kind, Kind::Axis | Kind::Stick) => Some(&STICK_PARTS),
            _ => None,
        }
    }

    /// Writes `control` in place of control `old_id`.
    pub fn put_control(&mut self, old_id: &str, control: Control) {
        let Some(id) = self.plugin.clone() else {
            return;
        };
        let result = self.store.edit_manifest(&id, |text| {
            gob_devices::edit::set_control(text, old_id, &control)
        });
        self.report(result);
    }

    /// Takes control `control` off the picture (its input can be placed again).
    pub fn delete_control(&mut self, control: &str) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let id = plugin.id().to_string();
        let mut inputs = plugin.defaults.clone();
        inputs.clear(control, "");
        let result = self.store.save_inputs(&id, &inputs).and_then(|()| {
            self.store
                .edit_manifest(&id, |text| gob_devices::edit::remove_control(text, control))
        });
        self.report(result);
        if self.selected.as_deref() == Some(control) {
            self.selected = None;
        }
    }

    /// While mapping: the bar (done, the device's views, copying another device's controls),
    /// the list of placed controls and the picture.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        ui.horizontal(|ui| {
            if ui
                .button(format!("{}  {}", icon::CHECK, i18n.tr("map-done")))
                .clicked()
            {
                self.done();
            }
            if let Some(name) = self.device_state().map(|d| d.info.name.trim().to_string()) {
                ui.label(egui::RichText::new(name).strong());
            }
            let can_copy = self.store.catalog.plugins.len() > 1;
            if style::icon_button(ui, can_copy, icon::COPY, &i18n.tr("map-copy")).clicked() {
                self.copy = Some(CopyControls::default());
            }
            if style::icon_button(
                ui,
                self.plugin.is_some(),
                icon::EXPORT,
                &i18n.tr("map-export"),
            )
            .clicked()
            {
                self.export_file();
            }
        });
        ui.separator();
        egui::Panel::right("mapping-panel")
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .show(ui, |ui| self.panel(ui, i18n));
        self.canvas(ui, i18n);
        // Axes' tags go dark again after a moment.
        if self.seen.values().any(|t| t.elapsed() < AXIS_LIT) {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Saves the mapped device's picture and mapping as one file to share.
    pub fn export_file(&mut self) {
        let Some(id) = self.plugin.clone() else {
            return;
        };
        let Some(to) = rfd::FileDialog::new()
            .set_file_name(format!("{id}.zip"))
            .add_filter("ZIP", &["zip"])
            .save_file()
        else {
            return;
        };
        let result = share::export(&self.store, &id, &to);
        if result.is_ok() {
            log::info!("device plugin {id} exported to {}", to.display());
        }
        self.report(result);
    }

    /// Takes in a shared file: a device's picture and mapping (asks first when it replaces the
    /// user's own mapping of that device).
    pub fn import_file(&mut self) {
        let Some(from) = rfd::FileDialog::new()
            .add_filter("ZIP", &["zip"])
            .pick_file()
        else {
            return;
        };
        match share::unpack(&self.store, &from) {
            Ok(unpacked) if unpacked.replaces.is_some() => self.import = Some(unpacked),
            Ok(unpacked) => {
                let result = unpacked.install(&mut self.store).map(|_| ());
                self.textures.clear();
                self.report(result);
            }
            Err(e) => self.report(Err(e)),
        }
    }

    /// Asks for a picture of controller `guid` (a device without a plugin).
    pub fn add_picture(&mut self, guid: &str) {
        let name = self
            .devices
            .iter()
            .find(|d| d.info.guid == guid)
            .map(|d| d.info.name.trim().to_string())
            .unwrap_or_default();
        self.new_device = Some(NewDevice::new(guid, name));
    }

    /// The dialogs, shown on top of whatever the page shows.
    pub fn dialogs(&mut self, ctx: &egui::Context, i18n: &I18n) {
        self.new_device_dialog(ctx, i18n);
        self.copy_dialog(ctx, i18n);
        self.import_dialog(ctx, i18n);
        if let Some(error) = self.error.clone() {
            let mut close = false;
            style::modal(ctx, "mapping-error").show(ctx, |ui| {
                ui.set_max_width(480.0);
                style::dialog_title(
                    ui,
                    Some((icon::WARNING, Some(style::RED))),
                    &i18n.tr("map-error"),
                );
                ui.label(&error);
                close = style::dialog_buttons(ui, &[style::DialogButton::primary(i18n.tr("ok"))])
                    .is_some();
            });
            if close {
                self.error = None;
            }
        }
    }
}

impl Mapper {
    /// A shared file would replace the user's own mapping of that device: replace or keep.
    fn import_dialog(&mut self, ctx: &egui::Context, i18n: &I18n) {
        let Some(unpacked) = self.import.take() else {
            return;
        };
        let mut result = None;
        style::modal(ctx, "mapping-import").show(ctx, |ui| {
            ui.set_max_width(480.0);
            style::dialog_title(
                ui,
                Some((icon::DOWNLOAD_SIMPLE, None)),
                &i18n.tr("map-import"),
            );
            ui.label(i18n.trf(
                "map-import-replaces",
                &[("name", unpacked.name.clone().into())],
            ));
            result = style::dialog_buttons(
                ui,
                &[
                    style::DialogButton::primary(i18n.tr("map-import-do")),
                    style::DialogButton::secondary(i18n.tr("cancel")),
                ],
            );
        });
        match result {
            Some(0) => {
                let result = unpacked.install(&mut self.store).map(|_| ());
                self.textures.clear();
                self.report(result);
            }
            Some(_) => unpacked.discard(),
            None => self.import = Some(unpacked),
        }
    }
}

/// An input's caption in the user's language, as the Devices page names it.
pub fn input_label(i18n: &I18n, input: Input) -> String {
    let (kind, n) = match input {
        Input::Button(n) => ("button", u32::from(n)),
        Input::Axis(slot) => ("axis", u32::from(slot)),
        Input::Hat(n) | Input::HatDirection(n, _) => ("hat", u32::from(n)),
    };
    crate::output_marks::input_name(i18n, kind, &n.to_string())
}

/// Which hat direction each neighbour of a hat at `center` sits in on the picture: the
/// closest fits first, one neighbour per direction.
fn directions_of(
    center: [f32; 2],
    neighbours: &[(String, Input, [f32; 2])],
) -> Vec<(&'static str, (String, Input))> {
    let angle_of = |part: &str| match part {
        "up" => -std::f32::consts::FRAC_PI_2,
        "right" => 0.0,
        "down" => std::f32::consts::FRAC_PI_2,
        _ => std::f32::consts::PI,
    };
    let mut pairs: Vec<(f32, &'static str, usize)> = Vec::new();
    for (k, (_, _, at)) in neighbours.iter().enumerate() {
        let a = (at[1] - center[1]).atan2(at[0] - center[0]);
        for part in &HAT_PARTS[..4] {
            let d = (a - angle_of(part)).rem_euclid(std::f32::consts::TAU);
            pairs.push((d.min(std::f32::consts::TAU - d), part, k));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(&'static str, (String, Input))> = Vec::new();
    let mut used = Vec::new();
    for (d, part, k) in pairs {
        // More than 60 degrees off: not that direction.
        if d > std::f32::consts::FRAC_PI_3
            || used.contains(&k)
            || out.iter().any(|(p, _)| *p == part)
        {
            continue;
        }
        used.push(k);
        out.push((part, (neighbours[k].0.clone(), neighbours[k].1)));
    }
    out
}

/// A placed control's first name, kept in the plugin (product wording is not translated).
fn plain_label(input: Input) -> String {
    match input {
        Input::Button(n) => format!("Button {n}"),
        Input::Axis(slot) => format!("Axis {}", axis_name(slot)),
        Input::Hat(n) | Input::HatDirection(n, _) => format!("Hat {n}"),
    }
}

/// DirectInput's axis names as players know them from controller tools.
fn axis_name(slot: u8) -> String {
    match gob_devices::input::AXES[usize::from(slot.clamp(1, 8) - 1)] {
        "slider" => "Slider".into(),
        "dial" => "Dial".into(),
        other => other.to_uppercase(),
    }
}
