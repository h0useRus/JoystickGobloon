use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, RichText, Vec2};
use gob_i18n::I18n;

use crate::devices_view::DevicesView;
use crate::hidhide_view::HidHideView;
use crate::profile_view::{report_window, LoadedProfile};
use crate::settings::Settings;
use crate::settings_view::{self, Change, SettingsTab};
use crate::style::{self, icon};
use crate::substitute_view::{auto_aliases, missing_devices, SubstituteDialog};
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
    hidhide_view: HidHideView,
    /// Running profile, if activated.
    runner: Option<gob_engine::Runner>,
    /// Time of the last edit; a running profile restarts once edits settle.
    edited: Option<std::time::Instant>,
    /// Action waiting for the "save changes?" answer.
    pending: Option<Pending>,
    /// The user already agreed to close without saving.
    allow_close: bool,
    show_modes: bool,
    modes_window: crate::modes_view::ModesWindow,
}

#[derive(Clone, Copy)]
enum Pending {
    Close,
    New,
    Open,
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
            hidhide_view: HidHideView::new(),
            runner: None,
            edited: None,
            pending: None,
            allow_close: false,
            show_modes: false,
            modes_window: Default::default(),
        };
        if app.settings.hidhide_whitelist_self {
            match app.hidhide_view.whitelist_self() {
                Some(Ok(true)) => app.status = app.i18n.tr("status-whitelisted"),
                Some(Err(e)) => app.status = e,
                _ => {}
            }
        }
        if let Some(path) = reopen {
            app.open_profile(path, false);
        }
        app
    }

    fn open_profile(&mut self, path: PathBuf, show_report: bool) {
        match gob_gremlin::load(&path) {
            Ok(profile) => {
                let loaded = LoadedProfile::new(profile, Some(path.clone()));
                self.status = self
                    .i18n
                    .trf("status-loaded", &[("file", loaded.file_name().into())]);
                self.show_report = show_report;
                self.profile = Some(loaded);
                self.runner = None;
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

    /// Saves to the current file, asking for one when the profile is new.
    fn save(&mut self) -> bool {
        match self.profile.as_ref().and_then(|p| p.path.clone()) {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let Some(loaded) = &self.profile else {
            return false;
        };
        let mut dialog = rfd::FileDialog::new()
            .add_filter(self.i18n.tr("file-filter-gremlin"), &["xml"])
            .set_file_name(loaded.file_name());
        if let Some(dir) = loaded.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        match dialog.save_file() {
            Some(path) => self.save_to(path),
            None => false,
        }
    }

    fn save_to(&mut self, path: PathBuf) -> bool {
        let Some(loaded) = &mut self.profile else {
            return false;
        };
        match gob_gremlin::save(&path, &loaded.profile) {
            Ok(()) => {
                self.status = self.i18n.trf(
                    "status-saved",
                    &[("file", path.display().to_string().into())],
                );
                loaded.path = Some(path.clone());
                loaded.dirty = false;
                self.settings.last_profile = Some(path);
                self.settings.save();
                true
            }
            Err(e) => {
                self.status = self
                    .i18n
                    .trf("save-error", &[("error", e.to_string().into())]);
                false
            }
        }
    }

    fn new_profile(&mut self) {
        let mut profile = gob_core::Profile::default();
        profile.set_startup_mode("Default");
        self.profile = Some(LoadedProfile::new(profile, None));
        self.runner = None;
        self.devices_view.reset_mode();
        self.status = self.i18n.tr("status-new");
    }

    /// Runs `action`, first asking to save unsaved changes.
    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        if self.profile.as_ref().is_some_and(|p| p.dirty) {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }

    fn perform(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::New => self.new_profile(),
            Pending::Open => self.import_dialog(),
        }
    }

    /// "Save changes?" dialog for a pending action.
    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending else { return };
        let file = self
            .profile
            .as_ref()
            .map(|p| p.file_name())
            .unwrap_or_default();
        let mut choice = None;
        egui::Modal::new(egui::Id::new("unsaved")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading(self.i18n.tr("unsaved-title"));
            ui.label(self.i18n.trf("unsaved-text", &[("file", file.into())]));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .button(format!(
                        "{}  {}",
                        icon::FLOPPY_DISK,
                        self.i18n.tr("unsaved-save")
                    ))
                    .clicked()
                {
                    choice = Some(0);
                }
                if ui.button(self.i18n.tr("unsaved-discard")).clicked() {
                    choice = Some(1);
                }
                if ui.button(self.i18n.tr("cancel")).clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.pending = None;
                if self.save() {
                    self.perform(action, ctx);
                }
            }
            Some(1) => {
                self.pending = None;
                self.perform(action, ctx);
            }
            Some(_) => self.pending = None,
            None => {}
        }
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

    fn toggle_runner(&mut self, ctx: &egui::Context) {
        if self.runner.take().is_some() {
            self.status = self.i18n.tr("status-stopped");
            return;
        }
        let Some(loaded) = &self.profile else { return };
        let devices = self.input.devices().clone();
        let aliases = auto_aliases(&loaded.profile, &devices);
        let repaint = {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        };
        match gob_engine::Runner::start(
            &loaded.profile,
            &aliases,
            self.input.subscribe(),
            &devices,
            repaint,
        ) {
            Ok(runner) => {
                self.runner = Some(runner);
                self.status = self.i18n.tr("status-active");
            }
            Err(e) => self.status = self.i18n.trf("activate-error", &[("error", e.into())]),
        }
    }

    fn devices_page(&mut self, ui: &mut egui::Ui) {
        let loaded = self.profile.is_some();
        // Center vertically: the play toggle is taller than the plain icon buttons.
        let row = Vec2::new(ui.available_width(), 44.0);
        ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
            if style::icon_button(ui, true, icon::FILE_PLUS, &self.i18n.tr("action-new")).clicked()
            {
                self.request(Pending::New, ui.ctx());
            }
            if style::icon_button(ui, true, icon::FOLDER_OPEN, &self.i18n.tr("action-open"))
                .clicked()
            {
                self.request(Pending::Open, ui.ctx());
            }
            let dirty = self.profile.as_ref().is_some_and(|p| p.dirty);
            let save = if dirty {
                icon::FLOPPY_DISK_BACK
            } else {
                icon::FLOPPY_DISK
            };
            if style::icon_button(ui, loaded, save, &self.i18n.tr("action-save")).clicked() {
                self.save();
            }
            if style::icon_button(
                ui,
                loaded,
                icon::FILE_ARROW_DOWN,
                &self.i18n.tr("action-save-as"),
            )
            .clicked()
            {
                self.save_as();
            }
            ui.separator();
            let (can_undo, can_redo) = self
                .profile
                .as_ref()
                .map_or((false, false), |p| (p.can_undo(), p.can_redo()));
            if style::icon_button(
                ui,
                can_undo,
                icon::ARROW_COUNTER_CLOCKWISE,
                &self.i18n.tr("action-undo"),
            )
            .clicked()
            {
                self.history(true);
            }
            if style::icon_button(
                ui,
                can_redo,
                icon::ARROW_CLOCKWISE,
                &self.i18n.tr("action-redo"),
            )
            .clicked()
            {
                self.history(false);
            }
            ui.separator();
            if style::icon_button(
                ui,
                loaded,
                icon::TREE_STRUCTURE,
                &self.i18n.tr("modes-title"),
            )
            .clicked()
            {
                self.show_modes = true;
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
            ui.separator();
            let active = self.runner.is_some();
            let (glyph, tip) = if active {
                (icon::STOP, "action-deactivate")
            } else {
                (icon::PLAY, "action-activate")
            };
            let toggle = ui.add_enabled_ui(loaded, |ui| {
                style::icon_toggle(ui, active, glyph, &self.i18n.tr(tip))
            });
            if toggle.inner.clicked() {
                self.toggle_runner(ui.ctx());
            }
            if let Some(p) = &self.profile {
                ui.separator();
                ui.label(RichText::new(icon::FILE_TEXT).size(style::ICON_SIZE));
                ui.strong(p.file_name()).on_hover_text(
                    p.path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                );
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
        let mut ops = Vec::new();
        self.devices_view
            .show(ui, &self.i18n, self.profile.as_ref(), &devices, &mut ops);
        drop(devices);
        self.apply_ops(ops);
    }

    /// Undo (`true`) or redo (`false`) the last edit.
    fn history(&mut self, undo: bool) {
        let Some(loaded) = &mut self.profile else {
            return;
        };
        let done = if undo { loaded.undo() } else { loaded.redo() };
        if done {
            self.edited = Some(std::time::Instant::now());
            self.status = self.i18n.tr(if undo {
                "status-undone"
            } else {
                "status-redone"
            });
        }
    }

    /// Applies queued edits, marks the profile unsaved and restarts a running profile.
    fn apply_ops(&mut self, ops: Vec<crate::action_editor::Op>) {
        let Some(loaded) = &mut self.profile else {
            return;
        };
        if ops.is_empty() {
            return;
        }
        loaded.checkpoint();
        for op in ops {
            crate::action_editor::apply(&mut loaded.profile, op);
        }
        loaded.refresh();
        self.edited = Some(std::time::Instant::now());
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
        let dirty = self.profile.as_ref().is_some_and(|p| p.dirty);
        if ctx.input(|i| i.viewport().close_requested()) && dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        let save_key = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
        if self.profile.is_some() && ctx.input_mut(|i| i.consume_shortcut(&save_key)) {
            self.save();
        }
        // Text fields keep their own Ctrl+Z while focused.
        if !ctx.wants_keyboard_input() {
            let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
            let redo = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            let redo_y = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Y);
            // Check the longer shortcut first: Ctrl+Shift+Z also matches Ctrl+Z.
            if ctx.input_mut(|i| i.consume_shortcut(&redo) || i.consume_shortcut(&redo_y)) {
                self.history(false);
            } else if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
                self.history(true);
            }
        }
        if let Some(t) = self.edited {
            let settle = std::time::Duration::from_millis(600);
            if self.runner.is_none() {
                self.edited = None;
            } else if t.elapsed() >= settle {
                self.edited = None;
                self.runner = None;
                self.toggle_runner(ctx);
            } else {
                ctx.request_repaint_after(settle - t.elapsed());
            }
        }
        self.devices_view
            .handle_events(self.input.events(), &self.input.devices());
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(60.0)
            .show(ctx, |ui| self.nav_rail(ui));

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(runner) = &self.runner {
                    let mode = runner.status().mode;
                    ui.label(
                        RichText::new(icon::PLAY)
                            .size(12.0)
                            .color(style::accent(ui)),
                    );
                    ui.label(RichText::new(format!("{}  {mode}", icon::STACK)).size(12.0))
                        .on_hover_text(self.i18n.tr("label-mode"));
                    ui.separator();
                }
                ui.label(RichText::new(&self.status).size(12.0).weak());
            });
        });

        let frame = egui::Frame::central_panel(&ctx.style()).inner_margin(16);
        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| match self.page {
                Page::Devices => self.devices_page(ui),
                Page::HidHide => self.hidhide_view.show(ui, &self.i18n),
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
                    loaded.checkpoint();
                    loaded.profile.substitute_device(old, new, name);
                }
                loaded.refresh();
                self.runner = None;
                self.status = self.i18n.tr("status-substituted");
            }
            if !open {
                self.substitute = None;
            }
        }

        if let (true, Some(loaded)) = (self.show_modes, &self.profile) {
            let mut open = true;
            let ops = self
                .modes_window
                .show(ctx, &self.i18n, &loaded.profile, &mut open);
            self.show_modes = open;
            if !ops.is_empty() {
                if let Some(loaded) = &mut self.profile {
                    for op in ops {
                        loaded.checkpoint();
                        crate::modes_view::apply(&mut loaded.profile, op);
                    }
                    loaded.refresh();
                    self.edited = Some(std::time::Instant::now());
                }
            }
        }
        self.unsaved_dialog(ctx);

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
