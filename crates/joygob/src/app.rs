use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, RichText, Vec2};
use gob_i18n::I18n;

use crate::devices_view::DevicesView;
use crate::profile_view::{report_window, LoadedProfile};
use crate::settings::Settings;
use crate::settings_view::{self, Change, SettingsTab};
use crate::style::{self, icon};
use crate::substitute_view::{missing_devices, SubstituteDialog};
use crate::vjoy_view::VJoyView;

const LOGO_PNG: &[u8] = include_bytes!("../../../assets/icon/joygob-256.png");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Devices,
    HidHide,
    VJoy,
    Settings,
}

pub struct JoyGobApp {
    i18n: I18n,
    settings: Settings,
    page: Page,
    settings_tab: SettingsTab,
    show_report: bool,
    status: String,
    profile: Option<LoadedProfile>,
    logo: egui::TextureHandle,
    input: gob_input::InputService,
    devices_view: DevicesView,
    substitute: Option<SubstituteDialog>,
    vjoy_view: VJoyView,
}

impl JoyGobApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        let lang_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("lang")));
        let mut i18n = I18n::new(lang_dir.as_deref());
        i18n.set_language(&settings.language);
        style::install_fonts(&cc.egui_ctx);
        style::apply(&cc.egui_ctx, settings.theme);

        let logo =
            eframe::icon_data::from_png_bytes(LOGO_PNG).expect("embedded logo is a valid PNG");
        let logo = cc.egui_ctx.load_texture(
            "logo",
            egui::ColorImage::from_rgba_unmultiplied(
                [logo.width as usize, logo.height as usize],
                &logo.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );

        let status = i18n.tr("status-ready");
        let reopen = settings
            .last_profile
            .clone()
            .filter(|p| settings.reopen_last && p.exists());
        let mut app = Self {
            i18n,
            settings,
            page: Page::Devices,
            settings_tab: SettingsTab::default(),
            show_report: false,
            status,
            profile: None,
            logo,
            input: {
                let ctx = cc.egui_ctx.clone();
                gob_input::InputService::start(move || ctx.request_repaint())
            },
            devices_view: DevicesView::default(),
            substitute: None,
            vjoy_view: VJoyView::new(),
        };
        if let Some(path) = reopen {
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
                self.devices_view.reset_mode();
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
        let Some(loaded) = &mut self.profile else {
            return;
        };
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
            Ok(()) => {
                let status = self.i18n.trf(
                    "status-saved",
                    &[("file", path.display().to_string().into())],
                );
                loaded.path = path.clone();
                loaded.dirty = false;
                self.settings.last_profile = Some(path);
                self.settings.save();
                status
            }
            Err(e) => self
                .i18n
                .trf("save-error", &[("error", e.to_string().into())]),
        };
    }

    fn nav_rail(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(34.0)));
            ui.add_space(12.0);
            for (page, glyph, key) in [
                (Page::Devices, icon::JOYSTICK, "nav-devices"),
                (Page::HidHide, icon::EYE_SLASH, "nav-hidhide"),
                (Page::VJoy, icon::GAME_CONTROLLER, "nav-vjoy"),
            ] {
                if style::icon_toggle(ui, self.page == page, glyph, &self.i18n.tr(key)).clicked() {
                    self.page = page;
                }
            }
        });
        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.add_space(8.0);
            let selected = self.page == Page::Settings;
            if style::icon_toggle(ui, selected, icon::GEAR_SIX, &self.i18n.tr("nav-settings"))
                .clicked()
            {
                self.page = Page::Settings;
            }
        });
    }

    fn devices_page(&mut self, ui: &mut egui::Ui) {
        let loaded = self.profile.is_some();
        ui.horizontal(|ui| {
            if style::icon_button(ui, true, icon::FOLDER_OPEN, &self.i18n.tr("action-open"))
                .clicked()
            {
                self.import_dialog();
            }
            let dirty = self.profile.as_ref().is_some_and(|p| p.dirty);
            let save = if dirty {
                icon::FLOPPY_DISK_BACK
            } else {
                icon::FLOPPY_DISK
            };
            if style::icon_button(ui, loaded, save, &self.i18n.tr("action-save")).clicked() {
                self.save_dialog();
            }
            let has_warnings = self
                .profile
                .as_ref()
                .is_some_and(|p| !p.report.warnings.is_empty());
            let glyph = if has_warnings {
                icon::WARNING
            } else {
                icon::CLIPBOARD_TEXT
            };
            if style::icon_button(ui, loaded, glyph, &self.i18n.tr("action-report")).clicked() {
                self.show_report = true;
            }
            if let Some(p) = &self.profile {
                ui.separator();
                ui.label(RichText::new(icon::FILE_TEXT).size(style::ICON_SIZE));
                ui.strong(p.file_name())
                    .on_hover_text(p.path.display().to_string());
                if p.dirty {
                    let (rect, r) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 4.0, style::accent(ui));
                    r.on_hover_text(self.i18n.tr("profile-modified"));
                }
            }
        });
        ui.separator();
        let no_devices = self.input.devices().iter().all(|d| d.info.is_vjoy());
        if self.profile.is_none() && no_devices {
            empty_state(ui, icon::JOYSTICK, &self.i18n.tr("no-profile"));
            ui.vertical_centered(|ui| {
                let open = format!("{}  {}", icon::FOLDER_OPEN, self.i18n.tr("action-open"));
                if ui.button(open).clicked() {
                    self.import_dialog();
                }
            });
            return;
        }
        let missing = self.profile.as_ref().map_or(0, |p| {
            missing_devices(&p.profile, &self.input.devices()).len()
        });
        let any_physical = self.input.devices().iter().any(|d| !d.info.is_vjoy());
        if missing > 0 && any_physical {
            let banner = egui::Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .corner_radius(8)
                .inner_margin(8);
            banner.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::PLUGS).size(style::ICON_SIZE));
                    ui.label(self.i18n.trf("subst-banner", &[("count", missing.into())]));
                    let text = format!("{}  {}", icon::SWAP, self.i18n.tr("subst-title"));
                    if ui.button(text).clicked() {
                        if let Some(p) = &self.profile {
                            self.substitute =
                                Some(SubstituteDialog::new(&p.profile, &self.input.devices()));
                        }
                    }
                });
            });
            ui.add_space(4.0);
        }
        let devices = self.input.devices();
        self.devices_view
            .show(ui, &self.i18n, self.profile.as_ref(), &devices);
    }

    fn apply_change(&mut self, ctx: &egui::Context, change: Change) {
        match change {
            Change::Theme(theme) => {
                self.settings.theme = theme;
                style::apply(ctx, theme);
            }
            Change::Language(code) => {
                self.i18n.set_language(&code);
                self.settings.language = self.i18n.language().to_string();
                self.status = self.i18n.tr("status-ready");
            }
        }
        self.settings.save();
    }
}

impl eframe::App for JoyGobApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.devices_view
            .handle_events(self.input.events(), &self.input.devices());
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(60.0)
            .show(ctx, |ui| self.nav_rail(ui));

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(RichText::new(&self.status).size(12.0).weak());
        });

        let frame = egui::Frame::central_panel(&ctx.style()).inner_margin(16);
        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| match self.page {
                Page::Devices => self.devices_page(ui),
                Page::HidHide => {
                    empty_state(ui, icon::EYE_SLASH, &self.i18n.tr("placeholder-hidhide"))
                }
                Page::VJoy => self.vjoy_view.show(ui, &self.i18n),
                Page::Settings => {
                    let change = settings_view::show(
                        ui,
                        &self.i18n,
                        &mut self.settings,
                        &mut self.settings_tab,
                        &self.logo,
                    );
                    if let Some(change) = change {
                        self.apply_change(ctx, change);
                    }
                }
            });

        if let Some(dialog) = &mut self.substitute {
            let mut open = true;
            let applied = dialog.show(ctx, &self.i18n, &self.input.devices(), &mut open);
            if let (Some(list), Some(loaded)) = (applied, &mut self.profile) {
                for (old, new, name) in &list {
                    loaded.profile.substitute_device(old, new, name);
                }
                loaded.refresh();
                self.status = self.i18n.tr("status-substituted");
            }
            if !open {
                self.substitute = None;
            }
        }

        if let (true, Some(loaded)) = (self.show_report, &self.profile) {
            report_window(ctx, &self.i18n, &loaded.report, &mut self.show_report);
        }
    }
}

/// Big faded icon with a hint, for pages without content.
pub(crate) fn empty_state(ui: &mut egui::Ui, glyph: &str, text: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.25);
        ui.label(RichText::new(glyph).size(64.0).weak());
        ui.add_space(8.0);
        ui.label(RichText::new(text).weak());
        ui.add_space(8.0);
    });
}
