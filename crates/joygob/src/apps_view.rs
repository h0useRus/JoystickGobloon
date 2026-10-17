//! Applications page: programs known to JoyGob and their options. For now the only option is
//! access to devices hidden by HidHide (its whitelist); profiles per program come later.

use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText};
use gob_hidhide::{same_path, HidHide};
use gob_i18n::I18n;

use crate::settings::{AppEntry, Settings};
use crate::style::{self, icon};

const LIST_WIDTH: f32 = 260.0;

pub struct AppsView {
    hh: Result<HidHide, String>,
    loaded: bool,
    /// HidHide whitelist.
    allowed: Vec<PathBuf>,
    selected: Option<PathBuf>,
    error: Option<String>,
}

impl AppsView {
    pub fn new() -> Self {
        Self {
            hh: HidHide::find().map_err(|e| e.to_string()),
            loaded: false,
            allowed: Vec::new(),
            selected: None,
            error: None,
        }
    }

    /// Adds JoyGob itself to the whitelist so it can read hidden devices.
    pub fn whitelist_self(&self) -> Option<Result<bool, String>> {
        let hh = self.hh.as_ref().ok()?;
        let exe = std::env::current_exe().ok()?;
        Some(hh.ensure_app(&exe).map_err(|e| e.to_string()))
    }

    fn refresh(&mut self) {
        self.loaded = true;
        let Ok(hh) = &self.hh else { return };
        match hh.apps() {
            Ok(apps) => {
                self.allowed = apps;
                self.error = None;
            }
            Err(e) => {
                log::error!("reading the HidHide whitelist: {e}");
                self.error = Some(e.to_string());
            }
        }
    }

    fn set_allowed(&mut self, exe: &Path, allowed: bool) {
        if let Ok(hh) = &self.hh {
            if let Err(e) = hh.set_app(exe, allowed) {
                log::error!("changing the HidHide whitelist: {e}");
                self.error = Some(e.to_string());
                return;
            }
        }
        self.refresh();
    }

    fn is_allowed(&self, exe: &Path) -> bool {
        self.allowed.iter().any(|a| same_path(a, exe))
    }

    /// Our list, then whitelisted programs that are not in it.
    fn rows(&self, apps: &[AppEntry]) -> Vec<PathBuf> {
        let mut rows: Vec<PathBuf> = apps.iter().map(|a| a.path.clone()).collect();
        for a in &self.allowed {
            if !rows.iter().any(|r| same_path(r, a)) {
                rows.push(a.clone());
            }
        }
        rows
    }

    /// Shows the page; returns whether `settings` changed.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, settings: &mut Settings) -> bool {
        if !self.loaded {
            self.refresh();
        }
        let mut changed = false;
        ui.horizontal(|ui| {
            let add = format!("{}  {}", icon::PLUS, i18n.tr("apps-add"));
            if ui.button(add).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("exe", &["exe"])
                    .pick_file()
                {
                    if !settings.apps.iter().any(|a| same_path(&a.path, &path)) {
                        settings.apps.push(AppEntry { path: path.clone() });
                        changed = true;
                    }
                    if self.hh.is_ok() && !self.is_allowed(&path) {
                        self.set_allowed(&path, true);
                    }
                    self.selected = Some(path);
                }
            }
            if style::icon_button(ui, true, icon::ARROWS_CLOCKWISE, &i18n.tr("action-refresh"))
                .clicked()
            {
                self.refresh();
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        ui.add_space(4.0);

        let rows = self.rows(&settings.apps);
        if rows.is_empty() {
            crate::app::empty_state(ui, icon::APP_WINDOW, &i18n.tr("apps-empty"));
            return changed;
        }
        if !self
            .selected
            .as_ref()
            .is_some_and(|s| rows.iter().any(|r| same_path(r, s)))
        {
            self.selected = rows.first().cloned();
        }
        let own = std::env::current_exe().ok();
        let is_own = |p: &Path| own.as_deref().is_some_and(|o| same_path(o, p));

        egui::SidePanel::left("apps-list")
            .resizable(false)
            .exact_width(LIST_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for path in &rows {
                            let selected = self.selected.as_deref() == Some(path.as_path());
                            let mut text = file_name(path);
                            // Same file name in several folders: add the folder name.
                            let twins = rows.iter().filter(|r| file_name(r) == text).count();
                            if let Some(dir) = path.parent().and_then(Path::file_name) {
                                if twins > 1 {
                                    text = format!("{text}  ·  {}", dir.to_string_lossy());
                                }
                            }
                            if is_own(path) {
                                text = format!("{}  {text}", icon::STAR);
                            }
                            if self.is_allowed(path) {
                                text = format!("{text}  {}", icon::EYE);
                            }
                            let text = if path.exists() {
                                RichText::new(text)
                            } else {
                                RichText::new(text).weak()
                            };
                            if ui
                                .selectable_label(selected, text)
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                self.selected = Some(path.clone());
                            }
                        }
                    });
            });

        let Some(path) = self.selected.clone() else {
            return changed;
        };
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading(file_name(&path));
            if is_own(&path) {
                ui.label(RichText::new(icon::STAR).color(style::accent(ui)))
                    .on_hover_text(crate::APP_NAME);
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new(path.display().to_string()).weak());
            if !path.exists() {
                ui.label(RichText::new(icon::FILE_X).weak())
                    .on_hover_text(i18n.tr("apps-missing"));
            }
        });
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            let allowed = self.is_allowed(&path);
            let tip = match &self.hh {
                Ok(_) => i18n.tr("apps-access-tip"),
                Err(_) => i18n.tr("hidhide-missing"),
            };
            let glyph = if allowed { icon::EYE } else { icon::EYE_CLOSED };
            let response = ui
                .add_enabled_ui(self.hh.is_ok(), |ui| {
                    style::icon_toggle(ui, allowed, glyph, &tip)
                })
                .inner;
            if response.clicked() {
                self.set_allowed(&path, !allowed);
            }
            ui.label(i18n.tr("apps-access"));
        });
        ui.add_space(12.0);

        let remove = format!("{}  {}", icon::TRASH, i18n.tr("apps-remove"));
        if ui.button(remove).clicked() {
            let before = settings.apps.len();
            settings.apps.retain(|a| !same_path(&a.path, &path));
            changed |= settings.apps.len() != before;
            if self.is_allowed(&path) {
                self.set_allowed(&path, false);
            }
            self.selected = None;
        }
        changed
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
