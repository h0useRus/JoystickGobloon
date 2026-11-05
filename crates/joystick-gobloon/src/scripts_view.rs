//! Scripts page: the profile's Lua scripts (stored in it or external files) and Joystick
//! Gremlin's Python scripts (kept, not run). The selected script shows its code (or file), its
//! error with the line, and a form for the variables it declares.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{self, RichText};
use gob_core::{
    InputType, Key, Profile, ScriptInstance, ScriptLanguage, ScriptSource, ScriptValue, script_path,
};
use gob_i18n::I18n;
use gob_input::DeviceState;
use gob_script::{ScriptError, VarKind, VariableDecl};

use crate::action_editor::Op;
use crate::style::{self, icon};

/// Code of a new script: one variable of each common kind and a handler.
const TEMPLATE: &str = r#"-- Variables appear on the Scripts page, where you pick their values.
local button = script.physical_input{ name = "Button", types = { "button" } }
local output = script.virtual_output{ name = "Output", types = { "button" } }

-- Runs on every press and release of the button.
button:on(function(e)
    output:set(e.pressed)
end)
"#;

/// What `describe` found for one version of a script's code.
struct Described {
    code_hash: u64,
    result: Result<Vec<VariableDecl>, ScriptError>,
}

/// An external file's code, read again when it changes.
struct FileCode {
    modified: Option<SystemTime>,
    code: Result<String, String>,
}

#[derive(Default)]
pub struct ScriptsView {
    selected: Option<String>,
    described: HashMap<String, Described>,
    files: HashMap<PathBuf, FileCode>,
    /// Modification times of the running profile's script files.
    watched: Vec<(PathBuf, Option<SystemTime>)>,
    /// JOYSTICK_GOBLOON_EXPAND was handled (debug builds).
    debug_selected: bool,
}

fn hash(code: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut h);
    h.finish()
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn type_key(t: InputType) -> &'static str {
    match t {
        InputType::Axis => "input-axis-kind",
        InputType::Hat => "input-hat-kind",
        _ => "input-button-kind",
    }
}

impl ScriptsView {
    /// The code of a script: its own, or its file's (read again when the file changes).
    fn code(&mut self, s: &ScriptInstance, base: Option<&Path>) -> Result<String, String> {
        match &s.source {
            ScriptSource::Code(code) => Ok(code.clone()),
            ScriptSource::File(path) => {
                let path = script_path(path, base);
                let now = modified(&path);
                let stale = self.files.get(&path).is_none_or(|f| f.modified != now);
                if stale {
                    let code = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    self.files.insert(
                        path.clone(),
                        FileCode {
                            modified: now,
                            code,
                        },
                    );
                }
                self.files[&path].code.clone()
            }
        }
    }

    fn describe(
        &mut self,
        s: &ScriptInstance,
        code: &str,
    ) -> &Result<Vec<VariableDecl>, ScriptError> {
        let code_hash = hash(code);
        let fresh = self
            .described
            .get(&s.id)
            .is_some_and(|d| d.code_hash == code_hash);
        if !fresh {
            let result = gob_script::describe(&s.name, code);
            self.described
                .insert(s.id.clone(), Described { code_hash, result });
        }
        &self.described[&s.id].result
    }

    /// The running profile's script files changed since the last call (none on the first).
    pub fn files_changed(&mut self, profile: &Profile, base: Option<&Path>) -> bool {
        let now: Vec<(PathBuf, Option<SystemTime>)> = profile
            .script_instances()
            .into_iter()
            .filter(|s| s.language == ScriptLanguage::Lua)
            .filter_map(|s| match s.source {
                ScriptSource::File(p) => Some(script_path(&p, base)),
                ScriptSource::Code(_) => None,
            })
            .map(|p| {
                let m = modified(&p);
                (p, m)
            })
            .collect();
        let changed = !self.watched.is_empty() && self.watched != now;
        self.watched = now;
        changed
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: &Profile,
        base: Option<&Path>,
        devices: &[DeviceState],
        vjoy: &[gob_virtual::DeviceInfo],
        ops: &mut Vec<Op>,
    ) {
        let scripts = profile.script_instances();
        // Debug builds: JOYSTICK_GOBLOON_EXPAND=script2 selects the second script (screenshots).
        if cfg!(debug_assertions) && !self.debug_selected {
            self.debug_selected = true;
            let n = std::env::var("JOYSTICK_GOBLOON_EXPAND")
                .ok()
                .and_then(|v| v.strip_prefix("script")?.parse::<usize>().ok());
            if let Some(s) = n.and_then(|n| scripts.get(n.saturating_sub(1))) {
                self.selected = Some(s.id.clone());
            }
        }
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !scripts.iter().any(|s| &s.id == id))
        {
            self.selected = scripts.first().map(|s| s.id.clone());
        }
        // Every Lua script is checked (cached by its code), so the list marks broken ones.
        for sc in scripts.iter().filter(|x| x.language == ScriptLanguage::Lua) {
            if let Ok(code) = self.code(sc, base) {
                self.describe(sc, &code);
            }
        }
        let list_width = 280.0;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(list_width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.list(ui, i18n, profile, &scripts, ops),
            );
            ui.separator();
            ui.vertical(|ui| {
                let selected = scripts
                    .iter()
                    .find(|s| Some(&s.id) == self.selected.as_ref());
                match selected {
                    Some(s) => {
                        egui::ScrollArea::vertical()
                            .id_salt("script-detail")
                            .show(ui, |ui| {
                                self.detail(ui, i18n, profile, s, base, devices, vjoy, ops)
                            });
                    }
                    None => {
                        crate::app::empty_state(ui, icon::CODE, &i18n.tr("scripts-empty"));
                    }
                }
            });
        });
    }

    fn list(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: &Profile,
        scripts: &[ScriptInstance],
        ops: &mut Vec<Op>,
    ) {
        ui.horizontal_wrapped(|ui| {
            let new = format!("{} {}", icon::PLUS, i18n.tr("scripts-new"));
            if ui.button(new).clicked() {
                let n = scripts.len() + 1;
                let name = i18n.trf("scripts-new-name", &[("n", n.to_string().into())]);
                ops.push(Op::AddScript {
                    name,
                    source: ScriptSource::Code(TEMPLATE.into()),
                });
                self.selected = None;
            }
            let file = format!("{} {}", icon::FILE_PLUS, i18n.tr("scripts-add-file"));
            if ui.button(file).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Lua", &["lua", "luau"])
                    .pick_file()
            {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ops.push(Op::AddScript {
                    name,
                    source: ScriptSource::File(path.display().to_string()),
                });
                self.selected = None;
            }
        });
        ui.add_space(style::SPACE_S);
        egui::ScrollArea::vertical()
            .id_salt("script-list")
            .show(ui, |ui| {
                for s in scripts {
                    let selected = Some(&s.id) == self.selected.as_ref();
                    let (glyph, caption) = match (&s.language, &s.source) {
                        (ScriptLanguage::Python, _) => (icon::WARNING, i18n.tr("scripts-python")),
                        (_, ScriptSource::Code(_)) => (icon::CODE, i18n.tr("scripts-in-profile")),
                        (_, ScriptSource::File(p)) => (icon::FILE_CODE, p.clone()),
                    };
                    let failed = self.described.get(&s.id).is_some_and(|d| d.result.is_err());
                    let frame = egui::Frame::new()
                        .fill(if selected {
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                        } else {
                            ui.visuals().faint_bg_color
                        })
                        .corner_radius(style::RADIUS)
                        .inner_margin(8);
                    let r = frame
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(glyph).size(style::ICON_SIZE));
                                let name = if s.name.is_empty() { "—" } else { &s.name };
                                ui.label(RichText::new(name).strong());
                                if failed {
                                    ui.label(RichText::new(icon::WARNING_CIRCLE).color(style::RED));
                                }
                            });
                            ui.add(egui::Label::new(RichText::new(caption).weak()).truncate());
                        })
                        .response
                        .interact(egui::Sense::click());
                    if r.clicked() {
                        self.selected = Some(s.id.clone());
                    }
                    ui.add_space(style::SPACE_XS);
                }
            });
        let _ = profile;
    }

    #[allow(clippy::too_many_arguments)]
    fn detail(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: &Profile,
        s: &ScriptInstance,
        base: Option<&Path>,
        devices: &[DeviceState],
        vjoy: &[gob_virtual::DeviceInfo],
        ops: &mut Vec<Op>,
    ) {
        let id = s.id.clone();
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if style::icon_button(ui, true, icon::TRASH, &i18n.tr("scripts-remove")).clicked() {
                    ops.push(Op::RemoveScript(id.clone()));
                }
                let list = profile.script_instances();
                let at = list.iter().position(|x| x.id == id).unwrap_or(0);
                if style::icon_button(
                    ui,
                    at + 1 < list.len(),
                    icon::CARET_DOWN,
                    &i18n.tr("edit-down"),
                )
                .clicked()
                {
                    ops.push(Op::MoveScript {
                        id: id.clone(),
                        up: false,
                    });
                }
                if style::icon_button(ui, at > 0, icon::CARET_UP, &i18n.tr("edit-up")).clicked() {
                    ops.push(Op::MoveScript {
                        id: id.clone(),
                        up: true,
                    });
                }
                let mut name = s.name.clone();
                let title = style::EditableText::new(&mut name, ("script-name", &id))
                    .hint(i18n.tr("scripts-name"))
                    .strong();
                if ui
                    .add(title)
                    .on_hover_text(i18n.tr("edit-rename"))
                    .changed()
                {
                    ops.push(Op::SetScriptName {
                        id: id.clone(),
                        name,
                    });
                }
            });
        });
        ui.add_space(style::SPACE_S);
        if s.language == ScriptLanguage::Python {
            ui.label(RichText::new(i18n.tr("scripts-python-help")).weak());
            if let ScriptSource::File(p) = &s.source {
                ui.label(RichText::new(p).weak());
            }
            return;
        }

        if let ScriptSource::File(p) = &s.source {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::FILE_CODE).size(style::ICON_SIZE));
                ui.add(egui::Label::new(p.as_str()).truncate());
                if style::icon_button(ui, true, icon::FOLDER_OPEN, &i18n.tr("scripts-choose-file"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Lua", &["lua", "luau"])
                        .pick_file()
                {
                    ops.push(Op::SetScriptSource {
                        id: id.clone(),
                        source: ScriptSource::File(path.display().to_string()),
                    });
                }
                if style::icon_button(
                    ui,
                    true,
                    icon::PENCIL_SIMPLE,
                    &i18n.tr("scripts-open-editor"),
                )
                .clicked()
                {
                    crate::platform::open_text_file(&script_path(p, base));
                }
            });
        }
        let code = self.code(s, base);
        let code = match code {
            Ok(code) => code,
            Err(e) => {
                let text = i18n.trf("scripts-file-missing", &[("error", e.into())]);
                ui.colored_label(style::RED, text);
                return;
            }
        };
        let result = self.describe(s, &code).clone();
        let error_line = result.as_ref().err().and_then(|e| e.line);
        if let ScriptSource::Code(_) = &s.source {
            let mut edited = code.clone();
            let editor = style::CodeEditor::new(&mut edited, ("script-code", &id))
                .desired_rows(14)
                .error_line(error_line);
            if ui.add(editor).changed() {
                ops.push(Op::SetScriptSource {
                    id: id.clone(),
                    source: ScriptSource::Code(edited),
                });
            }
        }
        ui.add_space(style::SPACE_S);
        let decls = match result {
            Ok(decls) => decls,
            Err(e) => {
                let text = match e.line {
                    Some(line) => i18n.trf(
                        "scripts-error",
                        &[
                            ("line", line.to_string().into()),
                            ("message", e.message.into()),
                        ],
                    ),
                    None => e.message,
                };
                ui.colored_label(style::RED, format!("{}  {text}", icon::WARNING_CIRCLE));
                return;
            }
        };
        ui.label(RichText::new(i18n.tr("scripts-variables")).strong());
        if decls.is_empty() {
            ui.label(RichText::new(i18n.tr("scripts-no-variables")).weak());
            return;
        }
        egui::Grid::new(("script-vars", &id))
            .num_columns(2)
            .spacing([style::SPACE_M, style::SPACE_S])
            .show(ui, |ui| {
                for d in &decls {
                    let stored = s
                        .variables
                        .iter()
                        .find(|(n, v)| *n == d.name && gob_script::fits(&d.kind, v))
                        .map(|(_, v)| v.clone());
                    let label = ui.label(&d.name);
                    if !d.description.is_empty() {
                        label.on_hover_text(&d.description);
                    }
                    ui.horizontal(|ui| {
                        let current = stored.clone().or_else(|| d.default.clone());
                        if let Some(v) =
                            value_editor(ui, i18n, profile, devices, vjoy, &id, d, current)
                        {
                            ops.push(Op::SetScriptVariable {
                                id: id.clone(),
                                name: d.name.clone(),
                                value: Some(v),
                            });
                        }
                        if stored.is_none() && d.default.is_none() && !d.optional {
                            ui.colored_label(style::RED, i18n.tr("scripts-needs-value"));
                        }
                    });
                    ui.end_row();
                }
            });
    }
}

/// The editor of one variable's value; returns a changed value.
#[allow(clippy::too_many_arguments)]
fn value_editor(
    ui: &mut egui::Ui,
    i18n: &I18n,
    profile: &Profile,
    devices: &[DeviceState],
    vjoy: &[gob_virtual::DeviceInfo],
    script: &str,
    d: &VariableDecl,
    current: Option<ScriptValue>,
) -> Option<ScriptValue> {
    let salt = (script.to_string(), d.name.clone());
    match &d.kind {
        VarKind::Bool => {
            let mut on = matches!(current, Some(ScriptValue::Bool(true)));
            ui.add(style::switch(&mut on, ""))
                .changed()
                .then_some(ScriptValue::Bool(on))
        }
        VarKind::Int { min, max } => {
            let mut v = match current {
                Some(ScriptValue::Int(i)) => i,
                _ => 0,
            };
            let (lo, hi) = ((*min).max(-1_000_000), (*max).min(1_000_000));
            ui.add(style::Spin::new(&mut v).range(lo..=hi))
                .changed()
                .then_some(ScriptValue::Int(v))
        }
        VarKind::Float { min, max } => {
            let mut v = match current {
                Some(ScriptValue::Float(f)) => f,
                _ => 0.0,
            };
            let (lo, hi) = ((*min).max(-1e6), (*max).min(1e6));
            ui.add(
                style::Spin::new(&mut v)
                    .range(lo..=hi)
                    .step(0.01)
                    .decimals(2),
            )
            .changed()
            .then_some(ScriptValue::Float(v))
        }
        VarKind::String => {
            let mut v = match current {
                Some(ScriptValue::String(s)) => s,
                _ => String::new(),
            };
            ui.add(style::text_field(&mut v).desired_width(240.0))
                .changed()
                .then_some(ScriptValue::String(v))
        }
        VarKind::Selection(options) => {
            let index = match current {
                Some(ScriptValue::Selection(i)) => Some(i),
                _ => None,
            };
            let mut picked = None;
            egui::ComboBox::from_id_salt(("script-sel", &salt))
                .selected_text(
                    index
                        .and_then(|i| options.get(i))
                        .cloned()
                        .unwrap_or_default(),
                )
                .show_ui(ui, |ui| {
                    for (i, o) in options.iter().enumerate() {
                        if ui.selectable_label(index == Some(i), o).clicked() {
                            picked = Some(ScriptValue::Selection(i));
                        }
                    }
                });
            picked
        }
        VarKind::Mode => {
            let mode = match current {
                Some(ScriptValue::Mode(m)) => m,
                _ => String::new(),
            };
            let mut picked = None;
            egui::ComboBox::from_id_salt(("script-mode", &salt))
                .selected_text(mode.as_str())
                .show_ui(ui, |ui| {
                    for m in &profile.modes {
                        if ui.selectable_label(m.name == mode, &m.name).clicked() {
                            picked = Some(ScriptValue::Mode(m.name.clone()));
                        }
                    }
                });
            picked
        }
        VarKind::Keyboard => {
            let keys: Vec<Key> = match current {
                Some(ScriptValue::Keyboard(k)) => vec![k],
                _ => Vec::new(),
            };
            let field = egui::Id::unique(("script-key", &salt));
            crate::key_capture::keys_field(ui, field, &keys, i18n)
                .and_then(|k| k.last().copied())
                .map(ScriptValue::Keyboard)
        }
        VarKind::PhysicalInput(types) => {
            let (device, input_type, id) = match current {
                Some(ScriptValue::PhysicalInput {
                    device,
                    input_type,
                    id,
                }) => (device, input_type, id),
                _ => (String::new(), types[0], 1),
            };
            let mut picked = None;
            let name = devices
                .iter()
                .find(|s| s.info.guid.eq_ignore_ascii_case(&device))
                .map(|s| s.info.name.trim().to_string())
                .or_else(|| profile.device_name(&device).map(String::from))
                .unwrap_or_else(|| {
                    if device.is_empty() {
                        i18n.tr("scripts-device")
                    } else {
                        device.chars().take(8).collect()
                    }
                });
            egui::ComboBox::from_id_salt(("script-dev", &salt))
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for s in devices.iter().filter(|s| !s.info.is_own()) {
                        let on = s.info.guid.eq_ignore_ascii_case(&device);
                        if ui.selectable_label(on, s.info.name.trim()).clicked() {
                            picked = Some(ScriptValue::PhysicalInput {
                                device: s.info.guid.clone(),
                                input_type,
                                id,
                            });
                        }
                    }
                });
            let (t, n) = type_and_id(ui, i18n, &salt, types, input_type, id);
            picked.or_else(|| {
                (t != input_type || n != id).then(|| ScriptValue::PhysicalInput {
                    device: device.clone(),
                    input_type: t,
                    id: n,
                })
            })
        }
        VarKind::VirtualOutput(types) => {
            let (device, input_type, id) = match current {
                Some(ScriptValue::Vjoy {
                    device,
                    input_type,
                    id,
                }) => (device, input_type, id),
                _ => (vjoy.first().map_or(1, |v| v.id), types[0], 1),
            };
            let mut picked = None;
            let name = |id: u32| {
                vjoy.iter()
                    .find(|v| v.id == id)
                    .map(|v| crate::vjoy_view::device_name(i18n, v))
                    .unwrap_or_else(|| format!("#{id}"))
            };
            egui::ComboBox::from_id_salt(("script-vjoy", &salt))
                .selected_text(name(device))
                .show_ui(ui, |ui| {
                    for v in vjoy {
                        if ui.selectable_label(v.id == device, name(v.id)).clicked() {
                            picked = Some(ScriptValue::Vjoy {
                                device: v.id,
                                input_type,
                                id,
                            });
                        }
                    }
                });
            let (t, n) = type_and_id(ui, i18n, &salt, types, input_type, id);
            picked.or_else(|| {
                (t != input_type || n != id).then_some(ScriptValue::Vjoy {
                    device,
                    input_type: t,
                    id: n,
                })
            })
        }
        VarKind::LogicalInput(types) => {
            let (input_type, id) = match current {
                Some(ScriptValue::LogicalDevice { input_type, id }) => (input_type, id),
                _ => (types[0], 1),
            };
            let (t, n) = type_and_id(ui, i18n, &salt, types, input_type, id);
            (t != input_type || n != id).then_some(ScriptValue::LogicalDevice {
                input_type: t,
                id: n,
            })
        }
    }
}

/// Input type (when there is a choice) and its 1-based number.
fn type_and_id(
    ui: &mut egui::Ui,
    i18n: &I18n,
    salt: &(String, String),
    types: &[InputType],
    input_type: InputType,
    id: u32,
) -> (InputType, u32) {
    let mut t = input_type;
    if types.len() > 1 {
        egui::ComboBox::from_id_salt(("script-type", salt))
            .selected_text(i18n.tr(type_key(t)))
            .show_ui(ui, |ui| {
                for &option in types {
                    ui.selectable_value(&mut t, option, i18n.tr(type_key(option)));
                }
            });
    } else {
        ui.label(i18n.tr(type_key(t)));
    }
    let max = match t {
        InputType::Axis => 8,
        InputType::Hat => 4,
        _ => 128,
    };
    let mut n = id.clamp(1, max);
    ui.add(style::Spin::new(&mut n).range(1..=max));
    (t, n)
}
