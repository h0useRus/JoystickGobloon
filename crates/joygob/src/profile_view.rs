//! Loaded profile: the Devices tab (bindings per device and mode) and the import report.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use gob_core::{Action, InputType, Profile};
use gob_gremlin::{ImportReport, Warning};
use gob_i18n::I18n;

pub struct LoadedProfile {
    pub profile: Profile,
    pub path: PathBuf,
    pub report: ImportReport,
    device: usize,
    mode: String,
}

impl LoadedProfile {
    pub fn new(profile: Profile, path: PathBuf) -> Self {
        let report = gob_gremlin::report(&profile);
        let mode = profile
            .startup_mode()
            .or_else(|| profile.modes.first().map(|m| m.name.clone()))
            .unwrap_or_default();
        Self {
            profile,
            path,
            report,
            device: 0,
            mode,
        }
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }

    pub fn devices_tab(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        let p = &self.profile;
        ui.horizontal(|ui| {
            ui.label(i18n.tr("label-mode"));
            egui::ComboBox::from_id_salt("mode")
                .selected_text(&self.mode)
                .show_ui(ui, |ui| {
                    for m in &p.modes {
                        ui.selectable_value(&mut self.mode, m.name.clone(), &m.name);
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            for (i, d) in p.devices.iter().enumerate() {
                ui.selectable_value(&mut self.device, i, d.name.trim());
            }
        });
        ui.separator();

        let Some(device) = p.devices.get(self.device) else {
            ui.label(i18n.tr("no-devices"));
            return;
        };
        let mut inputs: Vec<_> = p
            .inputs
            .iter()
            .filter(|b| b.device_id == device.id && b.mode == self.mode)
            .collect();
        inputs.sort_by_key(|b| {
            (
                type_order(b.input_type()),
                b.input_id.parse::<u32>().unwrap_or(u32::MAX),
            )
        });
        if inputs.is_empty() {
            ui.label(i18n.tr("no-bindings"));
            return;
        }

        let index = p.action_index();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for b in inputs {
                    let key = match b.input_type() {
                        InputType::Axis => "input-axis",
                        InputType::Button => "input-button",
                        InputType::Hat => "input-hat",
                        InputType::Other => "input-other",
                    };
                    let mut title = i18n.trf(key, &[("id", b.input_id.clone().into())]);
                    if let Some(summary) = summary(&index, &b.root_action) {
                        title = format!("{title}   ·   {summary}");
                    }
                    egui::CollapsingHeader::new(title)
                        .id_salt((&b.input_type, &b.input_id, &b.mode, &b.device_id))
                        .show(ui, |ui| action_tree(ui, &index, &b.root_action, 0));
                }
            });
    }
}

fn type_order(t: InputType) -> u8 {
    match t {
        InputType::Axis => 0,
        InputType::Button => 1,
        InputType::Hat => 2,
        InputType::Other => 3,
    }
}

/// Label of the first meaningful action under a root, shown on the collapsed input row.
fn summary(index: &HashMap<&str, &Action>, root: &str) -> Option<String> {
    let children: Vec<&Action> = index
        .get(root)?
        .child_ids()
        .into_iter()
        .filter_map(|id| index.get(id).copied())
        .collect();
    let first = children
        .iter()
        .find(|a| a.kind != "description")
        .or(children.first())?;
    Some(display_name(first))
}

fn display_name(action: &Action) -> String {
    match action.label().filter(|l| !l.is_empty() && *l != "None") {
        Some(label) => format!("{} — {label}", action.kind),
        None => action.kind.clone(),
    }
}

/// Shows an action and its children; root containers are transparent, as in Gremlin.
fn action_tree(ui: &mut egui::Ui, index: &HashMap<&str, &Action>, id: &str, depth: usize) {
    let Some(action) = index.get(id) else {
        ui.colored_label(ui.visuals().error_fg_color, format!("? {id}"));
        return;
    };
    let children = action.child_ids();
    if action.kind == "root" {
        children
            .into_iter()
            .for_each(|c| action_tree(ui, index, c, depth));
        return;
    }
    let text = display_name(action);
    if children.is_empty() || depth > 16 {
        ui.label(text);
    } else {
        egui::CollapsingHeader::new(text)
            .id_salt(&action.id)
            .default_open(true)
            .show(ui, |ui| {
                children
                    .into_iter()
                    .for_each(|c| action_tree(ui, index, c, depth + 1));
            });
    }
}

pub fn report_window(ctx: &egui::Context, i18n: &I18n, r: &ImportReport, open: &mut bool) {
    egui::Window::new(i18n.tr("import-title"))
        .open(open)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label(i18n.trf(
                "import-summary",
                &[
                    ("version", r.version.clone().into()),
                    ("inputs", r.inputs.into()),
                    ("actions", r.actions.into()),
                    ("devices", r.devices.into()),
                    ("modes", r.modes.into()),
                ],
            ));
            ui.add_space(6.0);
            ui.strong(i18n.tr("import-by-kind"));
            egui::Grid::new("by-kind").striped(true).show(ui, |ui| {
                for (kind, count) in &r.by_kind {
                    ui.label(kind);
                    ui.label(count.to_string());
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.strong(i18n.tr("import-warnings"));
            if r.warnings.is_empty() {
                ui.label(i18n.tr("import-no-warnings"));
            }
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for w in &r.warnings {
                        ui.label(warning_text(i18n, w));
                    }
                });
        });
}

fn warning_text(i18n: &I18n, w: &Warning) -> String {
    match w {
        Warning::DanglingRef { from, to } => i18n.trf(
            "warn-dangling",
            &[("from", from.clone().into()), ("to", to.clone().into())],
        ),
        Warning::UnknownMode(m) => i18n.trf("warn-unknown-mode", &[("mode", m.clone().into())]),
        Warning::NotExecutable { kind, count } => i18n.trf(
            "warn-not-executable",
            &[("kind", kind.clone().into()), ("count", (*count).into())],
        ),
        Warning::HasScripts => i18n.tr("warn-scripts"),
    }
}
