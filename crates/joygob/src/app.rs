use std::path::PathBuf;

use eframe::egui;
use gob_i18n::{FluentValue, I18n};

use crate::profile_view::{report_window, LoadedProfile};
use crate::settings::{Settings, Theme};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Devices,
    HidHide,
    VJoy,
}

pub struct JoyGobApp {
    i18n: I18n,
    settings: Settings,
    tab: Tab,
    show_about: bool,
    show_report: bool,
    status: String,
    profile: Option<LoadedProfile>,
}

impl JoyGobApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        let lang_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("lang")));
        let mut i18n = I18n::new(lang_dir.as_deref());
        i18n.set_language(&settings.language);
        apply_theme(&cc.egui_ctx, settings.theme);
        let status = i18n.tr("status-ready");
        let last = settings.last_profile.clone();
        let mut app = Self {
            i18n,
            settings,
            tab: Tab::Devices,
            show_about: false,
            show_report: false,
            status,
            profile: None,
        };
        if let Some(path) = last.filter(|p| p.exists()) {
            app.open_profile(path, false);
        }
        app
    }

    fn open_profile(&mut self, path: PathBuf, show_report: bool) {
        match gob_gremlin::load(&path) {
            Ok(profile) => {
                let loaded = LoadedProfile::new(profile, path.clone());
                self.status = self
                    .i18n
                    .trf("status-loaded", &[("file", loaded.file_name().into())]);
                self.show_report = show_report;
                self.profile = Some(loaded);
                self.settings.last_profile = Some(path);
                self.settings.save();
            }
            Err(e) => {
                self.status = self
                    .i18n
                    .trf("import-error", &[("error", e.to_string().into())])
            }
        }
    }

    fn import_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter(self.i18n.tr("file-filter-gremlin"), &["xml"])
            .pick_file();
        if let Some(path) = picked {
            self.open_profile(path, true);
        }
    }

    fn save_dialog(&mut self) {
        let Some(loaded) = &self.profile else { return };
        let mut dialog = rfd::FileDialog::new()
            .add_filter(self.i18n.tr("file-filter-gremlin"), &["xml"])
            .set_file_name(loaded.file_name());
        if let Some(dir) = loaded.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        self.status = match gob_gremlin::save(&path, &loaded.profile) {
            Ok(()) => self.i18n.trf(
                "status-saved",
                &[("file", path.display().to_string().into())],
            ),
            Err(e) => self
                .i18n
                .trf("save-error", &[("error", e.to_string().into())]),
        };
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.settings.theme = theme;
        apply_theme(ctx, theme);
        self.settings.save();
    }

    fn set_language(&mut self, code: &str) {
        self.i18n.set_language(code);
        self.settings.language = self.i18n.language().to_string();
        self.status = self.i18n.tr("status-ready");
        self.settings.save();
    }

    fn menu_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button(self.i18n.tr("menu-file"), |ui| {
                if ui.button(self.i18n.tr("menu-import-gremlin")).clicked() {
                    ui.close_menu();
                    self.import_dialog();
                }
                let can_save = self.profile.is_some();
                if ui
                    .add_enabled(
                        can_save,
                        egui::Button::new(self.i18n.tr("menu-save-gremlin")),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    self.save_dialog();
                }
                if ui
                    .add_enabled(can_save, egui::Button::new(self.i18n.tr("import-title")))
                    .clicked()
                {
                    self.show_report = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(self.i18n.tr("menu-exit")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(self.i18n.tr("menu-view"), |ui| {
                ui.menu_button(self.i18n.tr("menu-theme"), |ui| {
                    for (theme, key) in [(Theme::Light, "theme-light"), (Theme::Dark, "theme-dark")]
                    {
                        if ui
                            .radio(self.settings.theme == theme, self.i18n.tr(key))
                            .clicked()
                        {
                            self.set_theme(ctx, theme);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button(self.i18n.tr("menu-language"), |ui| {
                    for (code, name) in self.i18n.languages() {
                        if ui.radio(self.i18n.language() == code, name).clicked() {
                            self.set_language(&code);
                            ui.close_menu();
                        }
                    }
                });
            });
            ui.menu_button(self.i18n.tr("menu-help"), |ui| {
                if ui.button(self.i18n.tr("menu-about")).clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
    }
}

impl eframe::App for JoyGobApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ctx, ui));

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (tab, key) in [
                    (Tab::Devices, "tab-devices"),
                    (Tab::HidHide, "tab-hidhide"),
                    (Tab::VJoy, "tab-vjoy"),
                ] {
                    ui.selectable_value(&mut self.tab, tab, self.i18n.tr(key));
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| ui.label(&self.status));

        egui::CentralPanel::default().show(ctx, |ui| {
            if let (Tab::Devices, Some(loaded)) = (self.tab, self.profile.as_mut()) {
                loaded.devices_tab(ui, &self.i18n);
                return;
            }
            let key = match self.tab {
                Tab::Devices => "no-profile",
                Tab::HidHide => "placeholder-hidhide",
                Tab::VJoy => "placeholder-vjoy",
            };
            ui.centered_and_justified(|ui| ui.label(self.i18n.tr(key)));
        });

        if let (true, Some(loaded)) = (self.show_report, &self.profile) {
            report_window(ctx, &self.i18n, &loaded.report, &mut self.show_report);
        }

        if self.show_about {
            let mut args = gob_i18n::FluentArgs::new();
            args.set("version", FluentValue::from(env!("CARGO_PKG_VERSION")));
            let text = self.i18n.tr_args("about-text", Some(&args));
            egui::Window::new(self.i18n.tr("menu-about"))
                .open(&mut self.show_about)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| ui.label(text));
        }
    }
}

fn apply_theme(ctx: &egui::Context, theme: Theme) {
    ctx.set_visuals(match theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    });
}
