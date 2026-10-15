//! Loaded profile, its import report and the action tree widgets.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use gob_core::{Action, Profile};
use gob_gremlin::{ImportReport, Warning};
use gob_i18n::I18n;

use crate::style::{self, icon};

pub struct LoadedProfile {
    pub profile: Profile,
    /// `None` for a new profile that was never saved.
    pub path: Option<PathBuf>,
    pub report: ImportReport,
    /// Changed since it was loaded or saved.
    pub dirty: bool,
    undo: Vec<Profile>,
    redo: Vec<Profile>,
    last_checkpoint: Option<Instant>,
}

/// Edits closer together than this form one undo step (typing, dragging).
const COALESCE: Duration = Duration::from_millis(700);
const UNDO_LIMIT: usize = 100;

impl LoadedProfile {
    pub fn new(profile: Profile, path: Option<PathBuf>) -> Self {
        let report = gob_gremlin::report(&profile);
        Self {
            profile,
            path,
            report,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            last_checkpoint: None,
        }
    }

    /// Call before editing: remembers the current state for undo.
    pub fn checkpoint(&mut self) {
        if self.last_checkpoint.is_none_or(|t| t.elapsed() > COALESCE) {
            self.undo.push(self.profile.clone());
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_checkpoint = Some(Instant::now());
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.profile, previous));
        self.last_checkpoint = None;
        self.refresh();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.profile, next));
        self.last_checkpoint = None;
        self.refresh();
        true
    }

    /// Call after editing the profile: re-checks it and marks it unsaved.
    pub fn refresh(&mut self) {
        self.report = gob_gremlin::report(&self.profile);
        self.dirty = true;
    }

    pub fn file_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or_else(
            || "untitled.xml".into(),
            |n| n.to_string_lossy().into_owned(),
        )
    }
}

/// Label of the first meaningful action under a root, shown on the collapsed input row.
pub fn summary(index: &HashMap<&str, &Action>, root: &str) -> Option<String> {
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

/// Kind icon plus the user's label (or the Gremlin type name when there is none).
fn display_name(action: &Action) -> String {
    let text = action
        .label()
        .filter(|l| !l.is_empty() && *l != "None")
        .unwrap_or(&action.kind);
    format!("{}  {text}", kind_icon(&action.kind))
}

fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "map-to-vjoy" => icon::GAME_CONTROLLER,
        "response-curve" => icon::CHART_LINE,
        "change-mode" => icon::STACK,
        "tempo" => icon::TIMER,
        "description" => icon::NOTE,
        "macro" => icon::KEYBOARD,
        "map-to-mouse" => icon::MOUSE,
        _ => icon::PUZZLE_PIECE,
    }
}

/// Shows an action and its children; root containers are transparent, as in Gremlin.
pub fn action_tree(ui: &mut egui::Ui, index: &HashMap<&str, &Action>, id: &str, depth: usize) {
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
            ui.label(RichText::new(icon::LIST_BULLETS).size(style::ICON_SIZE));
            egui::Grid::new("by-kind").striped(true).show(ui, |ui| {
                for (kind, count) in &r.by_kind {
                    ui.label(format!("{}  {kind}", kind_icon(kind)));
                    ui.label(count.to_string());
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new(icon::WARNING).size(style::ICON_SIZE));
            if r.warnings.is_empty() {
                ui.label(format!(
                    "{}  {}",
                    icon::CHECK_CIRCLE,
                    i18n.tr("import-no-warnings")
                ));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(lp: &mut LoadedProfile, name: &str) {
        lp.checkpoint();
        lp.profile.add_mode(name, None);
        lp.refresh();
    }

    #[test]
    fn undo_redo_and_coalescing() {
        let mut lp = LoadedProfile::new(Profile::default(), None);
        assert!(!lp.can_undo());
        edit(&mut lp, "A");
        edit(&mut lp, "B"); // within the coalescing window: same step as "A"
        assert_eq!(lp.profile.modes.len(), 3);
        assert!(lp.undo());
        assert_eq!(lp.profile.modes.len(), 1, "both quick edits undone together");
        assert!(!lp.can_undo());
        assert!(lp.redo());
        assert_eq!(lp.profile.modes.len(), 3);

        lp.last_checkpoint = None; // as if time passed
        edit(&mut lp, "C");
        assert!(!lp.can_redo(), "a new edit clears redo");
        assert!(lp.undo());
        assert_eq!(lp.profile.modes.len(), 3);
    }
}
