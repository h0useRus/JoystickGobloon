//! Applications page: programs known to Joystick Gobloon (`apps.gob`) and their options. For now the only
//! option is access to hidden devices (the device hiding's application list, e.g. HidHide's);
//! profiles per program come later. Switching access off keeps the program on the page.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Align, Layout, RichText, Vec2};
use gob_hide::{Backend, same_path};
use gob_i18n::I18n;

use crate::settings::{AppEntry, AppList, Settings};
use crate::style::{self, icon};

const NAME_WIDTH: f32 = 220.0;

pub struct AppsView {
    hh: Result<Arc<dyn Backend>, String>,
    list: AppList,
    loaded: bool,
    /// The application list: programs that see hidden devices, or in inverse mode the ones
    /// that do not.
    listed: Vec<PathBuf>,
    inverse: bool,
    error: Option<String>,
    /// Program whose file is gone: asking whether to remove it.
    confirm_remove: Option<PathBuf>,
}

impl AppsView {
    pub fn new() -> Self {
        Self {
            hh: gob_hide::load().map_err(|e| e.to_string()),
            list: AppList::load(),
            loaded: false,
            listed: Vec::new(),
            inverse: false,
            error: None,
            confirm_remove: None,
        }
    }

    /// Lets Joystick Gobloon itself see hidden devices: adds it to the whitelist (not in inverse mode,
    /// where the list blocks programs instead).
    pub fn whitelist_self(&self) -> Option<Result<bool, String>> {
        let hh = self
            .hh
            .as_ref()
            .ok()
            .filter(|hh| hh.capabilities().app_list)?;
        let exe = std::env::current_exe().ok()?;
        let result = hh.apps_inverted().and_then(|inverse| {
            if inverse {
                Ok(false)
            } else {
                hh.ensure_app(&exe)
            }
        });
        Some(result.map_err(|e| e.to_string()))
    }

    /// Reads the application list; programs in it that are not on the page are added.
    fn refresh(&mut self) {
        self.loaded = true;
        let Ok(hh) = &self.hh else { return };
        let result = (|| {
            self.inverse = hh.apps_inverted()?;
            self.listed = hh.apps()?;
            Ok::<_, gob_hide::Error>(())
        })();
        if let Err(e) = &result {
            log::error!("reading the device hiding application list: {e}");
        }
        self.error = result.err().map(|e| e.to_string());
        let mut added = false;
        for path in &self.listed {
            if !self.list.apps.iter().any(|a| same_path(&a.path, path)) {
                self.list.apps.push(AppEntry { path: path.clone() });
                added = true;
            }
        }
        if added {
            self.list.save();
        }
    }

    /// Runs a change, then reloads the application list.
    fn apply(&mut self, change: impl FnOnce(&dyn Backend) -> Result<(), gob_hide::Error>) {
        if let Ok(hh) = &self.hh
            && let Err(e) = change(hh.as_ref())
        {
            log::error!("changing the device hiding application list: {e}");
            self.error = Some(e.to_string());
            return;
        }
        self.refresh();
    }

    fn is_listed(&self, exe: &Path) -> bool {
        self.listed.iter().any(|a| same_path(a, exe))
    }

    /// Takes a program out of the application list (its file may be gone already).
    fn unlist(&mut self, path: &Path) {
        if self.is_listed(path) {
            self.apply(|hh| hh.set_app(path, false));
        }
    }

    fn remove(&mut self, path: &Path) {
        self.unlist(path);
        self.list.apps.retain(|a| !same_path(&a.path, path));
        self.list.save();
    }

    /// Switches inverse mode, keeping Joystick Gobloon itself able to see hidden devices.
    fn set_inverse(&mut self, on: bool, settings: &Settings) {
        let own = std::env::current_exe().ok();
        self.apply(|hh| {
            if let Some(own) = &own
                && on
            {
                hh.set_app(own, false)?;
            }
            hh.set_apps_inverted(on)?;
            match &own {
                Some(own) if !on && settings.hidhide_whitelist_self => hh.ensure_app(own).map(drop),
                _ => Ok(()),
            }
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, settings: &Settings) {
        if !self.loaded {
            self.refresh();
        }
        ui.horizontal(|ui| {
            let add = format!("{}  {}", icon::PLUS, i18n.tr("apps-add"));
            if ui.button(add).clicked()
                && let Some(path) = program_dialog().pick_file()
            {
                if !self.list.apps.iter().any(|a| same_path(&a.path, &path)) {
                    self.list.apps.push(AppEntry { path: path.clone() });
                    self.list.save();
                }
                // A new program sees hidden devices: whitelisted, or not blocked when inverse.
                if self.hh.is_ok() && !self.inverse && !self.is_listed(&path) {
                    self.apply(|hh| hh.set_app(&path, true));
                }
            }
            if style::icon_button(ui, true, icon::ARROWS_CLOCKWISE, &i18n.tr("action-refresh"))
                .clicked()
            {
                self.list = AppList::load();
                self.refresh();
            }
            ui.add_space(12.0);
            let mut inverse = self.inverse;
            let response = ui
                .add_enabled(
                    self.hh.is_ok(),
                    style::switch(&mut inverse, i18n.tr("apps-inverse")),
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

        if self.list.apps.is_empty() {
            crate::app::empty_state(ui, icon::APP_WINDOW, &i18n.tr("apps-empty"));
            return;
        }
        let rows: Vec<PathBuf> = self.list.apps.iter().map(|a| a.path.clone()).collect();
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
                                let eye = style::icon_button(ui, self.hh.is_ok(), glyph, &tip);
                                if eye.clicked() {
                                    toggle = Some((path.clone(), !listed));
                                }
                                row(ui, path, own.as_deref(), i18n, &mut remove);
                            });
                        });
                }
            });
        if let Some((path, listed)) = toggle {
            if !path.exists() {
                self.confirm_remove = Some(path);
            } else if listed {
                self.apply(|hh| hh.set_app(&path, true));
            } else {
                self.unlist(&path);
            }
        }
        if let Some(path) = remove {
            self.remove(&path);
        }
        self.remove_dialog(ui.ctx(), i18n);
    }

    /// The program's file is gone: offer to remove it from the page.
    fn remove_dialog(&mut self, ctx: &egui::Context, i18n: &I18n) {
        let Some(path) = self.confirm_remove.clone() else {
            return;
        };
        let mut choice = None;
        egui::Modal::new(egui::Id::new("apps-remove")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading(i18n.tr("apps-missing"));
            ui.label(path.display().to_string());
            ui.add_space(4.0);
            ui.label(i18n.tr("apps-missing-text"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let remove = format!("{}  {}", icon::TRASH, i18n.tr("apps-remove"));
                if ui.button(remove).clicked() {
                    choice = Some(true);
                }
                if ui.button(i18n.tr("cancel")).clicked() {
                    choice = Some(false);
                }
            });
        });
        if let Some(remove) = choice {
            self.confirm_remove = None;
            if remove {
                self.remove(&path);
            }
        }
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

/// File picker for programs, filtered to the platform's program files (Windows: `.exe`).
fn program_dialog() -> rfd::FileDialog {
    let extensions = crate::platform::program_extensions();
    let dialog = rfd::FileDialog::new();
    if extensions.is_empty() {
        dialog
    } else {
        dialog.add_filter(extensions.join(", "), extensions)
    }
}
