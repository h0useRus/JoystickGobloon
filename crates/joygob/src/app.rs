use eframe::egui;
use gob_i18n::{FluentValue, I18n};

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
    status: String,
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
        Self { i18n, settings, tab: Tab::Devices, show_about: false, status }
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
                    self.status = self.i18n.tr("not-implemented");
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(self.i18n.tr("menu-exit")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(self.i18n.tr("menu-view"), |ui| {
                ui.menu_button(self.i18n.tr("menu-theme"), |ui| {
                    for (theme, key) in [(Theme::Light, "theme-light"), (Theme::Dark, "theme-dark")] {
                        if ui.radio(self.settings.theme == theme, self.i18n.tr(key)).clicked() {
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
            let key = match self.tab {
                Tab::Devices => "placeholder-devices",
                Tab::HidHide => "placeholder-hidhide",
                Tab::VJoy => "placeholder-vjoy",
            };
            ui.centered_and_justified(|ui| ui.label(self.i18n.tr(key)));
        });

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
