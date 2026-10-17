//! Applications page: programs known to JoyGob and their options. For now the only option is
//! access to devices hidden by HidHide (its application list); profiles per program come later.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Layout, RichText, Vec2};
use gob_hidhide::{same_path, HidHide};
use gob_i18n::I18n;

use crate::settings::{AppEntry, Settings};
use crate::style::{self, icon};

const NAME_WIDTH: f32 = 220.0;

pub struct AppsView {
    hh: Result<HidHide, String>,
    loaded: bool,
    /// HidHide's application list: whitelist, or blacklist in inverse mode.
    listed: Vec<PathBuf>,
    inverse: bool,
    error: Option<String>,
}

impl AppsView {
    pub fn new() -> Self {
        Self {
            hh: HidHide::find().map_err(|e| e.to_string()),
            loaded: false,
            listed: Vec::new(),
            inverse: false,
            error: None,
        }
    }

    /// Lets JoyGob itself see hidden devices: adds it to the whitelist (not in inverse mode,
    /// where the list blocks programs instead).
    pub fn whitelist_self(&self) -> Option<Result<bool, String>> {
        let hh = self.hh.as_ref().ok()?;
        let exe = std::env::current_exe().ok()?;
        let result = hh.inverse().and_then(|inverse| {
            if inverse {
                Ok(false)
            } else {
                hh.ensure_app(&exe)
            }
        });
        Some(result.map_err(|e| e.to_string()))
    }

    fn refresh(&mut self) {
        self.loaded = true;
        let Ok(hh) = &self.hh else { return };
        let result = (|| {
            self.inverse = hh.inverse()?;
            self.listed = hh.apps()?;
            Ok::<_, gob_hidhide::Error>(())
        })();
        if let Err(e) = &result {
            log::error!("reading the HidHide application list: {e}");
        }
        self.error = result.err().map(|e| e.to_string());
    }

    /// Runs a change, then reloads the state from HidHide.
    fn apply(&mut self, change: impl FnOnce(&HidHide) -> Result<(), gob_hidhide::Error>) {
        if let Ok(hh) = &self.hh {
            if let Err(e) = change(hh) {
                log::error!("changing the HidHide application list: {e}");
                self.error = Some(e.to_string());
                return;
            }
        }
        self.refresh();
    }

    fn is_listed(&self, exe: &Path) -> bool {
        self.listed.iter().any(|a| same_path(a, exe))
    }

    /// Our list, then programs from HidHide's list that are not in it.
    fn rows(&self, apps: &[AppEntry]) -> Vec<PathBuf> {
        let mut rows: Vec<PathBuf> = apps.iter().map(|a| a.path.clone()).collect();
        for a in &self.listed {
            if !rows.iter().any(|r| same_path(r, a)) {
                rows.push(a.clone());
            }
        }
        rows
    }

    /// Switches inverse mode, keeping JoyGob itself able to see hidden devices.
    fn set_inverse(&mut self, on: bool, settings: &Settings) {
        let own = std::env::current_exe().ok();
        self.apply(|hh| {
            if let Some(own) = &own {
                if on {
                    hh.set_app(own, false)?;
                }
            }
            hh.set_inverse(on)?;
            match &own {
                Some(own) if !on && settings.hidhide_whitelist_self => hh.ensure_app(own).map(drop),
                _ => Ok(()),
            }
        });
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
                    // A new program sees hidden devices: whitelisted, or not blocked when inverse.
                    if self.hh.is_ok() && !self.inverse && !self.is_listed(&path) {
                        self.apply(|hh| hh.set_app(&path, true));
                    }
                }
            }
            if style::icon_button(ui, true, icon::ARROWS_CLOCKWISE, &i18n.tr("action-refresh"))
                .clicked()
            {
                self.refresh();
            }
            ui.add_space(12.0);
            let mut inverse = self.inverse;
            let response = ui
                .add_enabled(
                    self.hh.is_ok(),
                    egui::Checkbox::new(&mut inverse, i18n.tr("apps-inverse")),
                )
                .on_hover_text(i18n.tr("apps-inverse-tip"));
            if response.changed() {
                self.set_inverse(inverse, settings);
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        ui.add_space(6.0);

        let rows = self.rows(&settings.apps);
        if rows.is_empty() {
            crate::app::empty_state(ui, icon::APP_WINDOW, &i18n.tr("apps-empty"));
            return changed;
        }
        let own = std::env::current_exe().ok();
        let mut toggle = None;
        let mut remove = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (i, path) in rows.iter().enumerate() {
                    let fill = if i % 2 == 1 {
                        ui.visuals().faint_bg_color
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    egui::Frame::new()
                        .fill(fill)
                        .corner_radius(4)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let listed = self.is_listed(path);
                                let sees = listed != self.inverse;
                                let (glyph, tip) = if sees {
                                    (icon::EYE, i18n.tr("apps-sees"))
                                } else {
                                    (icon::EYE_CLOSED, i18n.tr("apps-blind"))
                                };
                                let tip = if self.hh.is_ok() {
                                    tip
                                } else {
                                    i18n.tr("hidhide-missing")
                                };
                                let eye = ui
                                    .add_enabled_ui(self.hh.is_ok(), |ui| {
                                        style::icon_toggle(ui, sees, glyph, &tip)
                                    })
                                    .inner;
                                if eye.clicked() {
                                    toggle = Some((path.clone(), !listed));
                                }
                                row(ui, path, own.as_deref(), i18n, &mut remove);
                            });
                        });
                }
            });
        if let Some((path, listed)) = toggle {
            self.apply(|hh| hh.set_app(&path, listed));
        }
        if let Some(path) = remove {
            let before = settings.apps.len();
            settings.apps.retain(|a| !same_path(&a.path, &path));
            changed |= settings.apps.len() != before;
            if self.is_listed(&path) {
                self.apply(|hh| hh.set_app(&path, false));
            }
        }
        changed
    }
}

/// Name, path (shortened, full in the tooltip) and the remove button of one program.
fn row(
    ui: &mut egui::Ui,
    path: &Path,
    own: Option<&Path>,
    i18n: &I18n,
    remove: &mut Option<PathBuf>,
) {
    let h = ui.spacing().interact_size.y;
    let exists = path.exists();
    let full = path.display().to_string();
    let mut name = path
        .file_name()
        .map_or_else(|| full.clone(), |n| n.to_string_lossy().into_owned());
    if own.is_some_and(|o| same_path(o, path)) {
        name = format!("{}  {name}", icon::STAR);
    }
    let name = if exists {
        RichText::new(name)
    } else {
        RichText::new(name).weak()
    };
    ui.allocate_ui_with_layout(
        Vec2::new(NAME_WIDTH, h),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(NAME_WIDTH);
            ui.add(egui::Label::new(name).truncate())
                .on_hover_text(&full);
            if !exists {
                ui.label(RichText::new(icon::FILE_X).weak())
                    .on_hover_text(i18n.tr("apps-missing"));
            }
        },
    );
    let button = h + ui.spacing().item_spacing.x;
    let width = (ui.available_width() - button).max(0.0);
    ui.allocate_ui_with_layout(
        Vec2::new(width, h),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.add(egui::Label::new(RichText::new(&full).weak()).truncate())
                .on_hover_text(&full);
        },
    );
    if style::icon_button(ui, true, icon::TRASH, &i18n.tr("apps-remove")).clicked() {
        *remove = Some(path.to_path_buf());
    }
}
