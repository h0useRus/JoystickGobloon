//! The window: toolbar (brand, device, connected controller), the picture with a numbered tag
//! per placed input (`canvas`) and the list of placed inputs (`panel`).
//!
//! Mapping works by the controller itself: every button, axis or hat the user works gets the
//! next number, waiting in the picture's top left corner until it is dragged to the spot of the
//! picture where that control is. A placed number is a control of the device plugin with that one
//! input.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32};
use gob_devices::{Control, Detector, DevicePlugin, Input, Kind, Palette, Side};
use gob_i18n::I18n;
use gob_input::{DeviceState, InputEvent, InputService};
use gob_ui as style;
use style::icon;

use crate::dialogs::NewDevice;
use crate::settings::{Own, Shared, Theme};
use crate::store::Store;

/// The parts of a hat made of buttons.
pub const HAT_PARTS: [&str; 5] = ["up", "right", "down", "left", "push"];
/// How far around a button (a share of the picture's width) "Make it a hat" takes buttons in.
const HAT_GATHER: f32 = 0.05;
/// The parts of a stick made of axes.
pub const STICK_PARTS: [&str; 3] = ["x", "y", "twist"];

/// How long an axis' tag stays lit after the axis moved.
pub const AXIS_LIT: Duration = Duration::from_millis(700);

const PALETTES: style::Palettes = style::Palettes {
    dark: style::Palette {
        accent: Color32::from_rgb(0x8c, 0xc8, 0x2c),
        panel: Color32::from_rgb(0x1b, 0x1d, 0x22),
        window: Color32::from_rgb(0x23, 0x26, 0x2c),
        faint: Color32::from_rgb(0x2a, 0x2d, 0x34),
    },
    light: style::Palette {
        accent: Color32::from_rgb(0x6f, 0x9e, 0x1e),
        panel: Color32::from_rgb(0xf4, 0xf5, 0xf7),
        window: Color32::from_rgb(0xff, 0xff, 0xff),
        faint: Color32::from_rgb(0xe8, 0xea, 0xee),
    },
};

pub struct MapApp {
    pub i18n: I18n,
    pub store: Store,
    own: Own,
    pub brand: Option<String>,
    /// The device plugin open.
    pub plugin: Option<String>,
    /// Its view shown.
    pub view: String,
    /// The controllers; `None` in tests.
    pub input: Option<InputService>,
    /// Connected controllers (not our virtual ones), read each frame.
    pub devices: Vec<DeviceState>,
    /// The controller read (instance GUID).
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
    textures: HashMap<(String, String, bool), (egui::TextureHandle, egui::Vec2)>,
    pub new_device: Option<NewDevice>,
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

impl MapApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let shared = Shared::load();
        let lang_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("lang").join("map-gobloon")));
        let mut i18n = I18n::new(crate::LANGUAGES, lang_dir.as_deref());
        if !shared.language.is_empty() {
            i18n.set_language(&shared.language);
        }
        install_fonts(&cc.egui_ctx, matches!(i18n.language(), "ko" | "zh"));
        let preference = match shared.theme {
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::System => egui::ThemePreference::System,
        };
        style::apply(&cc.egui_ctx, &PALETTES, preference);
        let ctx = cc.egui_ctx.clone();
        let input = InputService::start(move || ctx.request_repaint());
        let mut app = Self::with(i18n, Store::new(), Some(input), Own::load());
        // Debug builds open a device at start for screenshots: JOYSTICK_GOBLOON_MAP_DEVICE=<id>.
        let start = std::env::var("JOYSTICK_GOBLOON_MAP_DEVICE")
            .ok()
            .filter(|_| cfg!(debug_assertions));
        if let Some(id) = start.or_else(|| app.own.last_device.clone())
            && app.store.get(&id).is_some()
        {
            app.open(&id);
        }
        app
    }

    pub fn with(i18n: I18n, store: Store, input: Option<InputService>, own: Own) -> Self {
        Self {
            i18n,
            store,
            own,
            brand: None,
            plugin: None,
            view: gob_devices::plugin::MAIN_VIEW.into(),
            input,
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
            error: None,
            layout: Layout::default(),
        }
    }

    pub fn plugin(&self) -> Option<&DevicePlugin> {
        self.store.get(self.plugin.as_deref()?)
    }

    /// Opens device plugin `id`.
    pub fn open(&mut self, id: &str) {
        let Some(plugin) = self.store.get(id) else {
            return;
        };
        self.brand = Some(plugin.manifest.brand.clone());
        self.view = plugin
            .manifest
            .views
            .first()
            .map_or_else(|| gob_devices::plugin::MAIN_VIEW.into(), |v| v.id.clone());
        self.plugin = Some(id.to_string());
        self.selected = None;
        self.name_edit = None;
        self.pending.clear();
        if self.own.last_device.as_deref() != Some(id) {
            self.own.last_device = Some(id.to_string());
            self.own.save();
        }
        // A connected controller this plugin fits is the one to read.
        let fits = self.devices.iter().find(|d| {
            let i = &d.info;
            self.store
                .get(id)
                .and_then(|p| p.score(i.vendor_id, i.product_id, &i.name))
                .is_some()
        });
        if let Some(guid) = fits.map(|d| d.info.guid.clone()) {
            self.choose_device(Some(guid));
        }
    }

    /// Reads controller `guid` from now on.
    pub fn choose_device(&mut self, guid: Option<String>) {
        if self.device == guid {
            return;
        }
        self.device = guid;
        self.pending.clear();
        self.seen.clear();
        self.detector = self.device_state().map(Detector::new);
    }

    pub fn device_state(&self) -> Option<&DeviceState> {
        let guid = self.device.as_deref()?;
        self.devices.iter().find(|d| d.info.guid == guid)
    }

    /// Reads the controllers and what they sent since the last frame.
    fn read_input(&mut self) {
        let Some(service) = &self.input else {
            return;
        };
        let events: Vec<InputEvent> = service.events().collect();
        self.devices = service
            .devices()
            .into_iter()
            .filter(|d| !d.info.is_own())
            .collect();
        let mut changed = false;
        for event in &events {
            changed |= matches!(event, InputEvent::DevicesChanged);
            if let Some(input) = self.detector.as_mut().and_then(|d| d.feed(event)) {
                self.note(input);
            }
        }
        if self.device.is_some() && self.device_state().is_none() {
            self.detector = None;
        } else if self.detector.is_none() && self.device_state().is_some() {
            self.detector = self.device_state().map(Detector::new);
        }
        // A controller appears: read it when it fits the open device, or when it is the only one.
        if (changed || self.detector.is_none()) && self.device.is_none() {
            let id = self.plugin.clone();
            let fits = self.devices.iter().find(|d| {
                let i = &d.info;
                id.as_deref()
                    .and_then(|id| self.store.get(id))
                    .and_then(|p| p.score(i.vendor_id, i.product_id, &i.name))
                    .is_some()
            });
            let pick = fits.or(match self.devices.as_slice() {
                [only] => Some(only),
                _ => None,
            });
            if let Some(guid) = pick.map(|d| d.info.guid.clone()) {
                self.choose_device(Some(guid));
            }
        }
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

    /// The open view's picture as a texture and its size in hotspot units.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<(egui::TextureHandle, egui::Vec2)> {
        let id = self.plugin.clone()?;
        let dark = ctx.global_style().visuals.dark_mode;
        let key = (id.clone(), self.view.clone(), dark);
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let picture = self.store.get(&id)?.pictures.get(&self.view)?;
        let [_, _, w, h] = picture.bounds()?;
        let palette = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mut rgba = match picture.rasterize(&palette, 2.0) {
            Ok(rgba) => rgba,
            Err(e) => {
                self.error = Some(e);
                return None;
            }
        };
        rgba.lift_blacks();
        let [pw, ph] = rgba.size;
        let image =
            egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], &rgba.pixels);
        let texture = ctx.load_texture(
            format!("device-{id}-{}-{dark}", self.view),
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

    /// An input's caption in the user's language: "Button 12", "Axis X", "Hat 1".
    pub fn input_label(&self, input: Input) -> String {
        use gob_i18n::FluentValue;
        match input {
            Input::Button(n) => self
                .i18n
                .trf("input-button", &[("n", FluentValue::from(n))]),
            Input::Axis(slot) => self.i18n.trf(
                "input-axis",
                &[("name", FluentValue::from(axis_name(slot)))],
            ),
            Input::Hat(n) | Input::HatDirection(n, _) => {
                self.i18n.trf("input-hat", &[("n", FluentValue::from(n))])
            }
        }
    }
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

pub fn install_fonts(ctx: &egui::Context, cjk: bool) {
    let mut fonts = egui::FontDefinitions::default();
    style::add_icon_font(&mut fonts);
    if cjk {
        for (name, file) in crate::platform::cjk_fonts() {
            let Ok(bytes) = std::fs::read(&file) else {
                continue;
            };
            fonts.font_data.insert(
                name.into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(name.into());
            }
        }
    }
    ctx.set_fonts(fonts);
}

impl eframe::App for MapApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(root);
    }
}

impl MapApp {
    /// Every frame, drawn or not: reads the controllers.
    pub fn tick(&mut self, ctx: &egui::Context) {
        self.read_input();
        // Axes' tags go dark again after a moment.
        if self.seen.values().any(|t| t.elapsed() < AXIS_LIT) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub fn show(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        let margin = egui::Margin::symmetric(16, 4);
        egui::Panel::top("toolbar")
            .frame(egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(margin))
            .show(root, |ui| self.toolbar(ui));
        egui::Panel::right("panel")
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .show(root, |ui| self.panel(ui));
        let frame = egui::Frame::central_panel(&ctx.global_style()).inner_margin(16);
        egui::CentralPanel::default()
            .frame(frame)
            .show(root, |ui| self.canvas(ui));
        self.dialogs(&ctx);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.set_min_height(style::TOOLBAR_BUTTON);
            let brands = self.store.brands();
            let brand_text = self
                .brand
                .clone()
                .unwrap_or_else(|| self.i18n.tr("toolbar-brand"));
            egui::ComboBox::from_id_salt("brand")
                .selected_text(brand_text)
                .width(140.0)
                .show_ui(ui, |ui| {
                    for b in &brands {
                        if ui
                            .selectable_label(self.brand.as_deref() == Some(b), b)
                            .clicked()
                        {
                            self.brand = Some(b.clone());
                        }
                    }
                })
                .response
                .on_hover_text(self.i18n.tr("toolbar-brand"));
            let devices = self
                .brand
                .as_deref()
                .map(|b| self.store.devices_of(b))
                .unwrap_or_default();
            let device_text = self
                .plugin()
                .filter(|p| Some(&p.manifest.brand) == self.brand.as_ref())
                .map(|p| p.manifest.name.clone())
                .unwrap_or_else(|| self.i18n.tr("toolbar-device"));
            let mut open = None;
            egui::ComboBox::from_id_salt("device")
                .selected_text(device_text)
                .width(280.0)
                .show_ui(ui, |ui| {
                    for (id, name) in &devices {
                        if ui
                            .selectable_label(self.plugin.as_deref() == Some(id), name)
                            .clicked()
                        {
                            open = Some(id.clone());
                        }
                    }
                })
                .response
                .on_hover_text(self.i18n.tr("toolbar-device"));
            if let Some(id) = open {
                self.open(&id);
            }
            if style::toolbar_button(ui, true, icon::PLUS_CIRCLE, &self.i18n.tr("toolbar-new"))
                .clicked()
            {
                self.new_device = Some(NewDevice::new(self));
            }
            ui.separator();
            ui.label(egui::RichText::new(icon::JOYSTICK).size(style::LARGE_ICON_SIZE))
                .on_hover_text(self.i18n.tr("toolbar-controller"));
            let current = self
                .device_state()
                .map(|d| d.info.name.clone())
                .unwrap_or_else(|| self.i18n.tr("toolbar-no-controller"));
            let mut pick = None;
            egui::ComboBox::from_id_salt("controller")
                .selected_text(current)
                .width(260.0)
                .show_ui(ui, |ui| {
                    for d in &self.devices {
                        let on = self.device.as_deref() == Some(d.info.guid.as_str());
                        if ui.selectable_label(on, &d.info.name).clicked() {
                            pick = Some(d.info.guid.clone());
                        }
                    }
                })
                .response
                .on_hover_text(self.i18n.tr("toolbar-controller"));
            if let Some(guid) = pick {
                self.choose_device(Some(guid));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if style::toolbar_button(
                    ui,
                    true,
                    icon::ARROWS_CLOCKWISE,
                    &self.i18n.tr("toolbar-reload"),
                )
                .clicked()
                {
                    self.store.reload();
                    self.textures.clear();
                }
                if style::toolbar_button(
                    ui,
                    true,
                    icon::FOLDER_OPEN,
                    &self.i18n.tr("toolbar-folder"),
                )
                .clicked()
                {
                    let dir = self.store.folder_of(self.plugin.as_deref());
                    let _ = std::fs::create_dir_all(&dir);
                    crate::platform::open_folder(&dir);
                }
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        self.new_device_dialog(ctx);
        if let Some(error) = self.error.clone() {
            let mut close = false;
            style::modal(ctx, "error").show(ctx, |ui| {
                ui.set_max_width(480.0);
                style::dialog_title(
                    ui,
                    Some((icon::WARNING, Some(style::RED))),
                    &self.i18n.tr("error-title"),
                );
                ui.label(&error);
                close =
                    style::dialog_buttons(ui, &[style::DialogButton::primary(self.i18n.tr("ok"))])
                        .is_some();
            });
            if close {
                self.error = None;
            }
        }
    }
}
