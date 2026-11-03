//! Applications page: programs known to Joystick Gobloon (`apps.gob`) and their options: access
//! to hidden devices (the device hiding's application list, e.g. HidHide's, where the system has
//! one) and the profile (and mode) that runs while the program is in front. Switching access off
//! keeps the program on the page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Align, Layout, RichText, Vec2};
use gob_hide::{Backend, same_path};
use gob_i18n::I18n;

use crate::settings::{AppEntry, AppList, Settings};
use crate::style::{self, icon};

const NAME_WIDTH: f32 = 220.0;
const PROFILE_WIDTH: f32 = 190.0;
const MODE_WIDTH: f32 = 150.0;

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
    /// Modes of the profiles chosen here, read once per file (empty: unreadable).
    modes: HashMap<PathBuf, Vec<String>>,
    /// "Add a running program" while its menu is open: read once when it opens.
    running: Option<Vec<crate::platform::Program>>,
}

/// A change to one program's row.
enum Edit {
    Hiding(PathBuf, bool),
    Remove(PathBuf),
    Profile(usize, Option<PathBuf>),
    ChooseProfile(usize),
    Mode(usize, Option<String>),
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
            modes: HashMap::new(),
            running: None,
        }
    }

    /// The system lets programs see hidden devices one by one (Linux without its system part
    /// hides from every program alike).
    fn hiding(&self) -> bool {
        self.hh.as_ref().is_ok_and(|hh| hh.capabilities().app_list)
    }

    /// Adds a program to the page; it sees hidden devices (whitelisted, or not blocked when
    /// inverse).
    fn add(&mut self, path: PathBuf) {
        if !self.list.apps.iter().any(|a| same_path(&a.path, &path)) {
            self.list.apps.push(AppEntry::new(path.clone()));
            self.list.save();
        }
        if self.hiding() && !self.inverse && !self.is_listed(&path) {
            self.apply(|hh| hh.set_app(&path, true));
        }
    }

    /// Mode names of a profile file, read once.
    fn modes_of(&mut self, profile: &Path) -> &[String] {
        self.modes.entry(profile.to_path_buf()).or_insert_with(|| {
            gob_gremlin::load(profile)
                .map(|p| p.modes.into_iter().map(|m| m.name).collect())
                .unwrap_or_else(|e| {
                    log::warn!("{}: {e}", profile.display());
                    Vec::new()
                })
        })
    }

    /// The programs of the page.
    pub fn apps(&self) -> &[AppEntry] {
        &self.list.apps
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
        if !self.hiding() {
            return;
        }
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
                self.list.apps.push(AppEntry::new(path.clone()));
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

    /// `running`: [`AppEntry::key`] of the program whose profile runs now (highlighted);
    /// `current`: the profile open in the window, offered in the profile lists.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        settings: &Settings,
        running: Option<&str>,
        current: Option<&Path>,
    ) {
        if !self.loaded {
            self.refresh();
        }
        let hiding = self.hiding();
        ui.horizontal(|ui| {
            let add = format!("{}  {}", icon::PLUS, i18n.tr("apps-add"));
            if ui.button(add).clicked()
                && let Some(path) = program_dialog().pick_file()
            {
                self.add(path);
            }
            self.running_menu(ui, i18n);
            if style::icon_button(ui, true, icon::ARROWS_CLOCKWISE, &i18n.tr("action-refresh"))
                .clicked()
            {
                self.list = AppList::load();
                self.modes.clear();
                self.refresh();
            }
            if hiding {
                ui.add_space(12.0);
                let mut inverse = self.inverse;
                let response = ui
                    .add(style::switch(&mut inverse, i18n.tr("apps-inverse")))
                    .on_hover_text(i18n.tr("apps-inverse-tip"));
                if response.changed() {
                    self.set_inverse(inverse, settings);
                }
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
        // Profiles to offer: the open one and the ones other programs use.
        let mut known: Vec<PathBuf> = current.map(Path::to_path_buf).into_iter().collect();
        for p in self.list.apps.iter().filter_map(|a| a.profile.clone()) {
            if !known.iter().any(|k| same_path(k, &p)) {
                known.push(p);
            }
        }
        let own = std::env::current_exe().ok();
        let mut edit = None;
        for i in 0..self.list.apps.len() {
            if let Some(profile) = self.list.apps[i].profile.clone() {
                self.modes_of(&profile);
            }
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (i, app) in self.list.apps.iter().enumerate() {
                    let is_running = running == Some(app.key().as_str());
                    let fill = if is_running {
                        ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                    } else if i % 2 == 1 {
                        ui.visuals().faint_bg_color
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let is_own = own.as_deref().is_some_and(|o| same_path(o, &app.path));
                    let modes = app
                        .profile
                        .as_ref()
                        .and_then(|p| self.modes.get(p))
                        .map_or(&[][..], Vec::as_slice);
                    let frame = egui::Frame::new()
                        .fill(fill)
                        .corner_radius(4)
                        .inner_margin(egui::Margin::symmetric(6, 2));
                    let shown = frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if hiding {
                                let listed = self.is_listed(&app.path);
                                let sees = listed != self.inverse;
                                let (glyph, tip) = if sees {
                                    (icon::EYE, i18n.tr("apps-sees"))
                                } else {
                                    (icon::EYE_CLOSED, i18n.tr("apps-blind"))
                                };
                                if style::icon_button(ui, true, glyph, &tip).clicked() {
                                    edit = Some(Edit::Hiding(app.path.clone(), !listed));
                                }
                            }
                            row(ui, i, app, is_own, &known, modes, i18n, &mut edit);
                        });
                    });
                    if is_running {
                        shown.response.on_hover_text(i18n.tr("apps-running-tip"));
                    }
                }
            });
        match edit {
            Some(Edit::Hiding(path, listed)) => {
                if !path.exists() {
                    self.confirm_remove = Some(path);
                } else if listed {
                    self.apply(|hh| hh.set_app(&path, true));
                } else {
                    self.unlist(&path);
                }
            }
            Some(Edit::Remove(path)) => self.remove(&path),
            Some(Edit::Profile(i, profile)) => self.set_profile(i, profile),
            Some(Edit::ChooseProfile(i)) => {
                let mut dialog =
                    rfd::FileDialog::new().add_filter(i18n.tr("file-filter-gremlin"), &["xml"]);
                if let Some(dir) = settings.profile_dir.as_ref().filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(path) = dialog.pick_file() {
                    self.set_profile(i, Some(path));
                }
            }
            Some(Edit::Mode(i, mode)) => {
                self.list.apps[i].mode = mode;
                self.list.save();
            }
            None => {}
        }
        self.remove_dialog(ui.ctx(), i18n);
    }

    fn set_profile(&mut self, i: usize, profile: Option<PathBuf>) {
        let app = &mut self.list.apps[i];
        if app.profile != profile {
            app.profile = profile;
            app.mode = None;
            self.list.save();
        }
    }

    /// "Add a running program": the programs with a window that are not on the page yet.
    fn running_menu(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        let text = format!("{}  {}", icon::LIST_PLUS, i18n.tr("apps-add-running"));
        let mut picked = None;
        let menu = ui.menu_button(text, |ui| {
            let list = &self.list;
            let programs = self.running.get_or_insert_with(|| {
                let own = std::env::current_exe().ok();
                let mut programs = crate::platform::running_programs();
                programs.retain(|p| {
                    !own.as_deref().is_some_and(|o| same_path(o, &p.path))
                        && !list.apps.iter().any(|a| same_path(&a.path, &p.path))
                });
                programs.sort_by_key(|p| p.name.to_lowercase());
                programs.dedup_by(|a, b| same_path(&a.path, &b.path));
                programs
            });
            if programs.is_empty() {
                ui.label(RichText::new(i18n.tr("apps-add-running-none")).weak());
            }
            for p in programs.iter() {
                let r = ui
                    .button(&p.name)
                    .on_hover_text(p.path.display().to_string());
                if r.clicked() {
                    picked = Some(p.path.clone());
                    ui.close();
                }
            }
        });
        if menu.inner.is_none() || picked.is_some() {
            self.running = None;
        }
        if let Some(path) = picked {
            self.add(path);
        }
    }

    /// The program's file is gone: offer to remove it from the page.
    fn remove_dialog(&mut self, ctx: &egui::Context, i18n: &I18n) {
        let Some(path) = self.confirm_remove.clone() else {
            return;
        };
        let mut choice = None;
        style::modal(ctx, "apps-remove").show(ctx, |ui| {
            ui.set_width(420.0);
            style::dialog_title(ui, Some((icon::TRASH, None)), &i18n.tr("apps-missing"));
            ui.label(path.display().to_string());
            ui.add_space(style::SPACE_XS);
            ui.label(i18n.tr("apps-missing-text"));
            let remove = format!("{}  {}", icon::TRASH, i18n.tr("apps-remove"));
            let buttons = [
                style::DialogButton::primary(remove),
                style::DialogButton::secondary(i18n.tr("cancel")),
            ];
            if let Some(i) = style::dialog_buttons(ui, &buttons) {
                choice = Some(i == 0);
            }
        });
        if let Some(remove) = choice {
            self.confirm_remove = None;
            if remove {
                self.remove(&path);
            }
        }
    }
}

/// Name, path (shortened, full in the tooltip), profile, mode and the remove button of one
/// program.
#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    i: usize,
    app: &AppEntry,
    own: bool,
    known: &[PathBuf],
    modes: &[String],
    i18n: &I18n,
    edit: &mut Option<Edit>,
) {
    let path = &app.path;
    let h = ui.spacing().interact_size.y;
    let exists = path.exists();
    let full = path.display().to_string();
    let mut name = path
        .file_name()
        .map_or_else(|| full.clone(), |n| n.to_string_lossy().into_owned());
    if own {
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
    let file_name = |p: &Path| {
        p.file_name().map_or_else(
            || p.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    let none = i18n.tr("apps-profile-none");
    let selected = app
        .profile
        .as_deref()
        .map_or_else(|| none.clone(), file_name);
    let start = i18n.tr("apps-mode-start");
    let mode = app.mode.clone().unwrap_or_else(|| start.clone());
    // From the right: remove, mode, profile; the path gets what is left.
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if style::icon_button(ui, true, icon::TRASH, &i18n.tr("apps-remove")).clicked() {
            *edit = Some(Edit::Remove(path.to_path_buf()));
        }
        ui.add_enabled_ui(app.profile.is_some() && !modes.is_empty(), |ui| {
            let combo = egui::ComboBox::from_id_salt(("app-mode", i))
                .width(MODE_WIDTH)
                .truncate()
                .selected_text(format!("{}  {mode}", icon::STACK))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(app.mode.is_none(), &start).clicked() {
                        *edit = Some(Edit::Mode(i, None));
                    }
                    for m in modes {
                        if ui
                            .selectable_label(app.mode.as_ref() == Some(m), m)
                            .clicked()
                        {
                            *edit = Some(Edit::Mode(i, Some(m.clone())));
                        }
                    }
                });
            combo.response.on_hover_text(i18n.tr("apps-mode-tip"));
        });
        ui.add_enabled_ui(!own, |ui| {
            let combo = egui::ComboBox::from_id_salt(("app-profile", i))
                .width(PROFILE_WIDTH)
                .truncate()
                .selected_text(format!("{}  {selected}", icon::FILE_TEXT))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(app.profile.is_none(), &none).clicked() {
                        *edit = Some(Edit::Profile(i, None));
                    }
                    for p in known {
                        let chosen = app.profile.as_deref().is_some_and(|a| same_path(a, p));
                        let r = ui
                            .selectable_label(chosen, file_name(p))
                            .on_hover_text(p.display().to_string());
                        if r.clicked() {
                            *edit = Some(Edit::Profile(i, Some(p.clone())));
                        }
                    }
                    let choose =
                        format!("{}  {}", icon::FOLDER_OPEN, i18n.tr("apps-profile-choose"));
                    if ui.selectable_label(false, choose).clicked() {
                        *edit = Some(Edit::ChooseProfile(i));
                    }
                });
            let tip = i18n.tr(if own {
                "apps-own-no-profile"
            } else {
                "apps-profile-tip"
            });
            combo
                .response
                .on_hover_text(&tip)
                .on_disabled_hover_text(&tip);
        });
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, h),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(width);
                ui.add(egui::Label::new(RichText::new(&full).weak()).truncate())
                    .on_hover_text(&full);
            },
        );
    });
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
