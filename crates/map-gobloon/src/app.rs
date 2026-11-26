//! The window: toolbar (brand, device, connected controller), the strip of inputs the controller
//! sends (`strip`), the picture with the controls' tags (`canvas`) and the controls' list and
//! editor (`panel`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32};
use gob_devices::{Control, Detector, DevicePlugin, Direction, Input, Inputs, Kind, Palette};
use gob_i18n::I18n;
use gob_input::{DeviceState, InputEvent, InputService};
use gob_ui as style;
use style::icon;

use crate::dialogs::{NewControl, NewDevice};
use crate::settings::{Own, Shared, Theme};
use crate::store::Store;

/// How long an axis' control stays lit after the axis moved.
pub const AXIS_LIT: Duration = Duration::from_millis(700);
/// The most inputs the strip keeps.
const RECENT: usize = 24;

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
    pub input: InputService,
    /// Connected controllers (not our virtual ones), read each frame.
    pub devices: Vec<DeviceState>,
    /// The controller whose inputs the strip shows (instance GUID).
    pub device: Option<String>,
    detector: Option<Detector>,
    /// Inputs the controller sent, newest first.
    pub recent: Vec<Input>,
    /// When each input last counted (lights axes' controls for a moment).
    pub seen: HashMap<Input, Instant>,
    /// The strip shows every input of the controller, not only the ones worked.
    pub show_all: bool,
    pub selected: Option<String>,
    /// The tag being dragged to a new place: (control id, its spot in picture units).
    pub moving: Option<(String, [f32; 2])>,
    /// The selected control's name while it is typed.
    pub name_edit: Option<(String, String)>,
    textures: HashMap<(String, String, bool), (egui::TextureHandle, egui::Vec2)>,
    pub new_device: Option<NewDevice>,
    pub new_control: Option<NewControl>,
    pub error: Option<String>,
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
        let store = Store::new();
        let own = Own::load();
        let mut app = Self {
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
            recent: Vec::new(),
            seen: HashMap::new(),
            // Debug builds: JOYSTICK_GOBLOON_MAP_ALL=1 shows every input at start.
            show_all: cfg!(debug_assertions)
                && std::env::var_os("JOYSTICK_GOBLOON_MAP_ALL").is_some(),
            selected: None,
            moving: None,
            name_edit: None,
            textures: HashMap::new(),
            new_device: None,
            new_control: None,
            error: None,
        };
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
        self.recent.clear();
        self.seen.clear();
        self.detector = self.device_state().map(Detector::new);
    }

    pub fn device_state(&self) -> Option<&DeviceState> {
        let guid = self.device.as_deref()?;
        self.devices.iter().find(|d| d.info.guid == guid)
    }

    /// Reads the controllers and what they sent since the last frame.
    fn read_input(&mut self) {
        let mut changed = false;
        let events: Vec<InputEvent> = self.input.events().collect();
        self.devices = self
            .input
            .devices()
            .into_iter()
            .filter(|d| !d.info.is_own())
            .collect();
        for event in &events {
            if matches!(event, InputEvent::DevicesChanged) {
                changed = true;
            }
            if let Some(input) = self.detector.as_mut().and_then(|d| d.feed(event)) {
                self.seen.insert(input, Instant::now());
                self.recent.retain(|i| *i != input);
                self.recent.insert(0, input);
                self.recent.truncate(RECENT);
            }
        }
        let gone = self.device.is_some() && self.device_state().is_none();
        if gone {
            self.detector = None;
        } else if self.detector.is_none() && self.device_state().is_some() {
            self.detector = self.device_state().map(Detector::new);
        }
        // A controller appears: read it when it fits the open device, or when it is the only one.
        if (changed || self.device.is_none()) && self.device.is_none() {
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

    /// Runs a change of the open plugin, showing what failed.
    pub fn report(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.error = Some(e);
        }
    }

    /// Puts `input` on `part` of `control` (and off wherever else it was).
    pub fn assign(&mut self, control: &str, part: &str, input: Input) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let id = plugin.id().to_string();
        let kind = plugin.control(control).map(|c| c.kind);
        let mut inputs = plugin.defaults.clone();
        take_off(&mut inputs, input);
        inputs.set(control, part, input);
        if kind == Some(Kind::Hat) {
            inputs.compact_hat(control);
        }
        let result = self.store.save_inputs(&id, &inputs);
        self.report(result);
        self.selected = Some(control.to_string());
    }

    /// Puts `input` on the part of `control` it fits best: a hat direction on a hat takes the
    /// whole hat, an axis the first free axis part, a button the first free other part.
    /// Returns whether it went anywhere.
    pub fn assign_best(&mut self, control: &str, input: Input) -> bool {
        let Some(plugin) = self.plugin() else {
            return false;
        };
        let Some(c) = plugin.control(control).cloned() else {
            return false;
        };
        let parts = c.parts();
        let inputs = plugin.defaults.clone();
        if parts == [""] {
            if c.kind.is_axis_part("") == matches!(input, Input::Axis(_)) {
                self.assign(control, "", input);
                return true;
            }
            return false;
        }
        if let Input::HatDirection(n, _) = input
            && Direction::ALL.iter().all(|d| parts.contains(&d.name()))
        {
            for d in Direction::ALL {
                self.assign(control, d.name(), Input::HatDirection(n, d));
            }
            return true;
        }
        let axis = matches!(input, Input::Axis(_));
        let fitting: Vec<&str> = parts
            .iter()
            .copied()
            .filter(|p| c.kind.is_axis_part(p) == axis)
            .collect();
        let named = match input {
            Input::HatDirection(_, d) => fitting.iter().copied().find(|p| *p == d.name()),
            _ => None,
        };
        let free = fitting
            .iter()
            .copied()
            .find(|p| inputs.get(control, p).is_none());
        match named.or(free).or(fitting.first().copied()) {
            Some(part) => {
                self.assign(control, part, input);
                true
            }
            None => false,
        }
    }

    /// Takes the input off `part` of `control`.
    pub fn unassign(&mut self, control: &str, part: &str) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let id = plugin.id().to_string();
        let mut inputs = plugin.defaults.clone();
        inputs.clear(control, part);
        let result = self.store.save_inputs(&id, &inputs);
        self.report(result);
    }

    /// Writes `control` in place of control `old_id` (or as a new one).
    pub fn put_control(&mut self, old_id: &str, control: Control) {
        let Some(id) = self.plugin.clone() else {
            return;
        };
        let result = self.store.edit_manifest(&id, |text| {
            gob_devices::edit::set_control(text, old_id, &control)
        });
        self.report(result);
    }

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

    /// An input's caption: "Button 12", "Axis X", "Hat 1 ↑".
    pub fn input_label(&self, input: Input) -> String {
        use gob_i18n::FluentValue;
        match input {
            Input::Button(n) => self
                .i18n
                .trf("input-button", &[("n", FluentValue::from(n))]),
            Input::Axis(slot) => {
                let name = gob_devices::input::AXES[usize::from(slot - 1)];
                self.i18n.trf(
                    "input-axis",
                    &[("name", FluentValue::from(axis_name(name)))],
                )
            }
            Input::Hat(n) => self.i18n.trf("input-hat", &[("n", FluentValue::from(n))]),
            Input::HatDirection(n, d) => format!(
                "{} {}",
                self.i18n.trf("input-hat", &[("n", FluentValue::from(n))]),
                direction_glyph(d)
            ),
        }
    }

    /// A part's short name: arrows for directions, words for the rest.
    pub fn part_label(&self, part: &str) -> String {
        match part {
            "" => String::new(),
            "up" => icon::ARROW_UP.into(),
            "right" => icon::ARROW_RIGHT.into(),
            "down" => icon::ARROW_DOWN.into(),
            "left" => icon::ARROW_LEFT.into(),
            "x" => "X".into(),
            "y" => "Y".into(),
            other => self.i18n.tr(&format!("part-{other}")),
        }
    }

    pub fn kind_label(&self, kind: Kind) -> String {
        self.i18n.tr(&format!("kind-{}", kind_name(kind)))
    }

    /// Where `input` is: "Trigger · 1", or `None` when it is on no control.
    pub fn mapped_to(&self, input: Input) -> Option<String> {
        let plugin = self.plugin()?;
        let (control, part) = plugin.defaults.parts_with(input).into_iter().next()?;
        let name = plugin
            .control(&control)
            .map_or(control.clone(), |c| c.name.clone());
        Some(match self.part_label(&part) {
            p if p.is_empty() => name,
            p => format!("{name} {p}"),
        })
    }
}

/// Takes `input` off every control part it is on (a whole hat: each of its directions).
fn take_off(inputs: &mut Inputs, input: Input) {
    inputs.remove_input(input);
    if let Input::Hat(n) = input {
        for d in Direction::ALL {
            inputs.remove_input(Input::HatDirection(n, d));
        }
    }
}

pub fn direction_glyph(d: Direction) -> &'static str {
    match d {
        Direction::Up => icon::ARROW_UP,
        Direction::Right => icon::ARROW_RIGHT,
        Direction::Down => icon::ARROW_DOWN,
        Direction::Left => icon::ARROW_LEFT,
    }
}

/// DirectInput's axis names as players know them from controller tools.
fn axis_name(name: &str) -> String {
    match name {
        "x" | "y" | "z" => name.to_uppercase(),
        "rx" => "RX".into(),
        "ry" => "RY".into(),
        "rz" => "RZ".into(),
        "slider" => "Slider".into(),
        "dial" => "Dial".into(),
        other => other.into(),
    }
}

pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Button => "button",
        Kind::Trigger => "trigger",
        Kind::Hat => "hat",
        Kind::Encoder => "encoder",
        Kind::Switch => "switch",
        Kind::Lever => "lever",
        Kind::Axis => "axis",
        Kind::Stick => "stick",
    }
}

pub const KINDS: [Kind; 8] = [
    Kind::Button,
    Kind::Trigger,
    Kind::Hat,
    Kind::Encoder,
    Kind::Switch,
    Kind::Lever,
    Kind::Axis,
    Kind::Stick,
];

fn install_fonts(ctx: &egui::Context, cjk: bool) {
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
        self.read_input();
        // Axes' controls go dark again after a moment.
        if self.seen.values().any(|t| t.elapsed() < AXIS_LIT) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let margin = egui::Margin::symmetric(16, 4);
        egui::Panel::top("toolbar")
            .frame(egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(margin))
            .show(root, |ui| self.toolbar(ui));
        egui::Panel::top("strip")
            .frame(
                egui::Frame::side_top_panel(&ctx.global_style())
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(root, |ui| self.strip(ui));
        egui::Panel::right("panel")
            .resizable(true)
            .default_size(320.0)
            .min_size(260.0)
            .show(root, |ui| self.panel(ui));
        let frame = egui::Frame::central_panel(&ctx.global_style()).inner_margin(16);
        egui::CentralPanel::default()
            .frame(frame)
            .show(root, |ui| self.canvas(ui));
        self.dialogs(&ctx);
    }
}

impl MapApp {
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
        self.new_control_dialog(ctx);
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
