use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, RichText, Vec2};
use gob_i18n::I18n;

use crate::activation::Activation;
use crate::apps_view::AppsView;
use crate::devices_view::DevicesView;
use crate::hidhide_view::HidHideView;
use crate::profile_view::{LoadedProfile, important_warnings};
use crate::settings::Settings;
use crate::settings_view::{self, Change, SettingsTab};
use crate::style::{self, icon};
use crate::substitute_view::{SubstituteDialog, missing_devices};
use crate::vjoy_view::VJoyView;

const LOGO_PNG: &[u8] = include_bytes!("../../../assets/icon/JoystickGobloon-256.png");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Devices,
    Apps,
    VJoy,
    Viewer,
    Calibration,
    Settings,
}

pub struct JoystickGobloonApp {
    i18n: I18n,
    settings: Settings,
    page: Page,
    settings_tab: SettingsTab,
    /// Warnings and errors for the user, shown one at a time as dialogs.
    alerts: Vec<Alert>,
    status: String,
    profile: Option<LoadedProfile>,
    logo: egui::TextureHandle,
    input: gob_input::InputService,
    devices_view: DevicesView,
    substitute: Option<SubstituteDialog>,
    vjoy_view: VJoyView,
    viewer_view: crate::viewer_view::ViewerView,
    /// The input viewer is shown in its own window.
    viewer_window: bool,
    calibration_view: crate::calibration_view::CalibrationView,
    /// The calibration is shown in its own window.
    calibration_window: bool,
    /// Saved axis calibrations (also applied in the input service).
    calibrations: gob_input::Calibrations,
    hidhide_view: HidHideView,
    /// The device hiding window is open.
    hidhide_window: bool,
    apps_view: AppsView,
    /// Banner for device access the user still has to grant (Linux).
    device_access: crate::device_access::DeviceAccess,
    /// Running profile, if activated.
    /// New releases from GitHub (About page, check on start).
    updater: crate::update::Updater,
    /// A downloaded update waiting for the app to close.
    update: Option<crate::update::Prepared>,
    /// The running profile; shared with the tray menu, which works while the window is hidden.
    activation: Activation,
    /// Active in the last frame: a change made from the tray updates the status bar.
    was_active: bool,
    /// Time of the last edit; a running profile restarts once edits settle.
    edited: Option<std::time::Instant>,
    /// Action waiting for the "save changes?" answer.
    pending: Option<Pending>,
    /// The user already agreed to close without saving.
    allow_close: bool,
    /// The window was minimized last frame ("minimize to tray" acts once per minimize).
    was_minimized: bool,
    show_modes: bool,
    modes_window: crate::modes_view::ModesWindow,
    /// Sound and speech previews from the editor.
    media: gob_engine::Media,
    leds: crate::leds::Leds,
    tray: Option<crate::tray::Tray>,
    /// The window was shown from outside winit (tray, second start): sync it next frame.
    restored: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Started at logon with a tray icon: frames left in which the window is hidden again
    /// (eframe shows it after its first frame whatever the viewport said).
    hide_frames: u8,
    /// Main window handle, for showing it from the tray.
    main_window: Option<crate::platform::Window>,
    /// Input changes repaint the window only while it shows live input values.
    live_input: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The window was on screen last frame (not in the tray, not minimized).
    was_shown: bool,
    /// Plugin DLLs from `<exe folder>\plugins`, kept loaded while the app runs.
    _plugins: gob_plugin::Registry,
    /// Window title last sent to the viewport.
    title: String,
}

/// A dialog with a title and lines of text; technical details go to the log file.
struct Alert {
    title: String,
    lines: Vec<String>,
}

#[derive(Clone)]
enum Pending {
    Close,
    /// A downloaded update: the app closes and the update takes over.
    Update,
    /// A load-profile action asked for this file; it is activated after loading.
    Load(PathBuf),
    New,
    Open,
}

impl JoystickGobloonApp {
    /// `instance`: the single-instance wake-up event (another start shows this window);
    /// `None` when several copies may run.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        instance: Option<crate::platform::InstanceEvent>,
    ) -> Self {
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
        let live_input = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let main_window = crate::platform::main_window(cc);
        let input = {
            let (ctx, live) = (cc.egui_ctx.clone(), live_input.clone());
            gob_input::InputService::start(move || {
                // `live` is set per frame; the tray check covers the time after hiding.
                if live.load(std::sync::atomic::Ordering::Relaxed)
                    && !main_window.is_some_and(crate::platform::in_tray)
                {
                    ctx.request_repaint();
                }
            })
        };
        let activation = Activation::new(input.handle(), cc.egui_ctx.clone());
        let mut app = Self {
            i18n,
            settings,
            page: Page::Devices,
            settings_tab: SettingsTab::default(),
            alerts: Vec::new(),
            status,
            profile: None,
            logo,
            input,
            devices_view: DevicesView::default(),
            substitute: None,
            vjoy_view: VJoyView::new(),
            viewer_view: Default::default(),
            viewer_window: false,
            calibration_view: Default::default(),
            calibration_window: false,
            calibrations: Default::default(),
            hidhide_view: HidHideView::new(),
            hidhide_window: false,
            apps_view: AppsView::new(),
            device_access: crate::device_access::DeviceAccess::new(),
            activation,
            was_active: false,
            edited: None,
            pending: None,
            allow_close: false,
            was_minimized: false,
            show_modes: false,
            modes_window: Default::default(),
            title: crate::APP_NAME.into(),
            media: Default::default(),
            leds: Default::default(),
            _plugins: load_plugins(),
            updater: Default::default(),
            update: None,
            tray: None,
            restored: Default::default(),
            hide_frames: 0,
            main_window,
            live_input,
            was_shown: true,
        };
        app.update_tray(&cc.egui_ctx);
        app.activation.set_led_defaults(app.settings.led_defaults());
        app.calibrations = app.settings.calibrations();
        for ((device, slot), c) in &app.calibrations {
            app.input.set_calibration(device, *slot, Some(*c));
        }
        if let (Some(main_window), Some(instance)) = (app.main_window, instance) {
            let (restored, ctx) = (app.restored.clone(), cc.egui_ctx.clone());
            crate::platform::listen_instance(instance, move || {
                log::info!("started again: showing the running window");
                restored.store(true, std::sync::atomic::Ordering::Relaxed);
                crate::platform::show_window(main_window);
                ctx.request_repaint();
            });
        }
        // Debug builds: open a page at start for screenshot checks without clicking.
        #[cfg(debug_assertions)]
        match std::env::var("JOYSTICK_GOBLOON_START_PAGE").as_deref() {
            Ok("hidhide") => app.hidhide_window = true,
            Ok("apps") => app.page = Page::Apps,
            Ok("vjoy") => app.page = Page::VJoy,
            Ok("viewer") => app.page = Page::Viewer,
            Ok("calibration") => app.page = Page::Calibration,
            Ok("viewer-window") => {
                app.page = Page::Viewer;
                app.viewer_window = true;
            }
            Ok("settings") => app.page = Page::Settings,
            Ok("general") => {
                app.page = Page::Settings;
                app.settings_tab = SettingsTab::General;
            }
            Ok("about") => {
                app.page = Page::Settings;
                app.settings_tab = SettingsTab::About;
            }
            Ok("lighting") => {
                app.page = Page::Settings;
                app.settings_tab = SettingsTab::Lighting;
            }
            _ => {}
        }
        // Started at logon without a (working) tray icon: show the window, minimized, so it
        // can never stay hidden with no way back.
        let at_logon = std::env::args().any(|a| a == crate::tray::autostart::ARG);
        if at_logon && app.tray.is_some() {
            app.hide_frames = 3;
        }
        if at_logon && app.tray.is_none() {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(true));
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if app.settings.hidhide_whitelist_self {
            match app.apps_view.whitelist_self() {
                Some(Ok(true)) => {
                    log::info!("added to the HidHide whitelist");
                    app.status = app.i18n.tr("status-whitelisted");
                }
                Some(Err(e)) => {
                    let text = app
                        .i18n
                        .trf("hidhide-whitelist-error", &[("error", e.into())]);
                    app.error(text);
                }
                _ => {}
            }
        }
        crate::update::cleanup();
        if app.settings.check_updates && crate::update::enabled() {
            app.updater.check(&cc.egui_ctx, true);
        }
        if let Some(path) = reopen {
            app.open_profile(path, false);
            if app.settings.activate_on_start && app.profile.is_some() {
                app.toggle_runner(&cc.egui_ctx);
            }
        }
        // Nothing reopened: start with an empty profile (like Gremlin), so inputs can get
        // actions right away instead of only after "New profile".
        if app.profile.is_none() {
            let status = std::mem::take(&mut app.status);
            app.new_profile();
            app.status = status;
        }
        app
    }

    /// Creates or drops the tray icon to match the settings.
    fn update_tray(&mut self, ctx: &egui::Context) {
        match (
            self.settings.show_tray,
            self.tray.is_some(),
            self.main_window,
        ) {
            (true, false, Some(main_window)) => {
                let (show, quit) = (self.i18n.tr("tray-show"), self.i18n.tr("tray-quit"));
                let hiding = self.i18n.tr("tray-hiding");
                let restored = self.restored.clone();
                let tray = crate::tray::Tray::new(
                    ctx,
                    main_window,
                    restored,
                    self.activation.clone(),
                    crate::APP_NAME,
                    &show,
                    &hiding,
                    &quit,
                );
                match tray {
                    Ok(t) => {
                        log::info!("tray icon shown");
                        self.tray = Some(t);
                    }
                    Err(e) => log::error!("tray icon: {e}"),
                }
            }
            (true, false, None) => log::error!("tray icon: no window handle"),
            (false, true, _) => self.tray = None,
            _ => {}
        }
    }

    /// Logs an error and shows it to the user.
    fn error(&mut self, text: String) {
        log::error!("{text}");
        self.alerts.push(Alert {
            title: self.i18n.tr("alert-error"),
            lines: vec![text],
        });
    }

    /// `interactive`: opened by the user, so import warnings are shown, not only logged.
    fn open_profile(&mut self, path: PathBuf, interactive: bool) {
        match gob_gremlin::load(&path) {
            Ok(profile) => {
                let report = gob_gremlin::report(&profile);
                log::info!("loaded {}: {report}", path.display());
                for w in &report.warnings {
                    log::warn!("{}: {w}", path.display());
                }
                let loaded = LoadedProfile::new(profile, Some(path.clone()));
                self.status = self
                    .i18n
                    .trf("status-loaded", &[("file", loaded.file_name().into())]);
                let lines = important_warnings(&self.i18n, &report);
                if interactive && !lines.is_empty() {
                    self.alerts.push(Alert {
                        title: self
                            .i18n
                            .trf("alert-import", &[("file", loaded.file_name().into())]),
                        lines,
                    });
                }
                self.profile = Some(loaded);
                self.activation.stop();
                self.devices_view.reset_mode();
                self.settings.last_profile = Some(path);
                self.settings.save();
            }
            Err(e) => {
                log::error!("loading {}: {e}", path.display());
                let text = self
                    .i18n
                    .trf("import-error", &[("error", e.to_string().into())]);
                self.error(text);
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
                log::info!("saved {}", path.display());
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
                log::error!("saving {}: {e}", path.display());
                let text = self
                    .i18n
                    .trf("save-error", &[("error", e.to_string().into())]);
                self.error(text);
                false
            }
        }
    }

    fn new_profile(&mut self) {
        let mut profile = gob_core::Profile::default();
        profile.set_startup_mode("Default");
        self.profile = Some(LoadedProfile::new(profile, None));
        self.activation.stop();
        self.devices_view.reset_mode();
        self.status = self.i18n.tr("status-new");
    }

    /// Runs `action`, first asking to save unsaved changes.
    /// Close or minimize to tray. Without a window handle (not Windows) the window is hidden.
    fn hide_to_tray(&self, ctx: &egui::Context) {
        match self.main_window {
            Some(main_window) => crate::platform::hide_to_tray(main_window),
            None => ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false)),
        }
    }

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
            Pending::Update => {
                let Some(prepared) = self.update.take() else {
                    return;
                };
                match crate::update::apply(&prepared) {
                    Ok(()) => {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => {
                        let text = self
                            .i18n
                            .trf("update-install-error", &[("error", e.into())]);
                        self.error(text);
                    }
                }
            }
            Pending::New => self.new_profile(),
            Pending::Open => self.import_dialog(),
            Pending::Load(path) => {
                self.open_profile(path, true);
                if self.profile.is_some() && !self.activation.is_active() {
                    self.toggle_runner(ctx);
                }
            }
        }
    }

    /// The input viewer and the calibration in their own native windows, when detached.
    fn detached_windows(&mut self, ctx: &egui::Context) {
        let devices = self.input.devices().clone();
        if self.viewer_window {
            let (viewer, i18n) = (&mut self.viewer_view, &self.i18n);
            let title = i18n.tr("nav-viewer");
            let attach = detached_window(ctx, "input-viewer", &title, BIG_WINDOW, |ui| {
                viewer.show(ui, i18n, &devices, true)
            });
            if attach {
                self.viewer_window = false;
            }
        }
        if self.calibration_window {
            let (view, i18n, saved) = (&mut self.calibration_view, &self.i18n, &self.calibrations);
            let title = i18n.tr("nav-calibration");
            let mut out = crate::calibration_view::Output::default();
            let attach = detached_window(ctx, "calibration", &title, BIG_WINDOW, |ui| {
                out = view.show(ui, i18n, &devices, saved, true);
                out.toggle_window
            });
            self.apply_calibration(out);
            if attach {
                self.calibration_window = false;
            }
        }
        if self.hidhide_window {
            let (view, i18n, settings) = (&mut self.hidhide_view, &self.i18n, &mut self.settings);
            let title = i18n.tr("nav-hidhide");
            let mut changed = false;
            let close = detached_window(ctx, "hidhide", &title, [720.0, 420.0], |ui| {
                changed |= view.show(ui, i18n, settings);
                false
            });
            if changed {
                self.settings.save();
            }
            if close {
                self.hidhide_window = false;
            }
        }
    }

    /// Saves and applies calibrations from the calibration page.
    fn apply_calibration(&mut self, out: crate::calibration_view::Output) {
        for (device, slot, c) in out.save {
            log::info!("calibration saved: {device} axis {slot}: {c:?}");
            self.settings
                .calibration
                .entry(device.clone())
                .or_default()
                .insert(slot.to_string(), c.into());
            self.input.set_calibration(&device, slot, Some(c));
            self.calibrations.insert((device, slot), c);
        }
        if !self.calibrations.is_empty() {
            self.settings.save();
        }
    }

    /// Warning/error dialogs, one at a time.
    fn alert_dialog(&mut self, ctx: &egui::Context) {
        let Some(alert) = self.alerts.first() else {
            return;
        };
        let mut close = false;
        egui::Modal::new(egui::Id::new("alert")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::WARNING)
                        .size(style::ICON_SIZE)
                        .color(ui.visuals().warn_fg_color),
                );
                ui.heading(&alert.title);
            });
            ui.add_space(4.0);
            for line in &alert.lines {
                ui.label(line);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(self.i18n.tr("ok")).clicked() {
                    close = true;
                }
                if let Some(path) = crate::logfile::path() {
                    let text = format!("{}  {}", icon::FILE_TEXT, self.i18n.tr("alert-open-log"));
                    if ui
                        .button(text)
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        crate::platform::open_text_file(&path);
                    }
                }
            });
        });
        if close {
            self.alerts.remove(0);
        }
    }

    /// "Save changes?" dialog for a pending action.
    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending.clone() else {
            return;
        };
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
            // The logo is in the window title bar; the rail starts with the pages.
            ui.add_space(8.0);
            for (page, glyph, key) in [
                (Page::Devices, icon::JOYSTICK, "nav-devices"),
                (Page::Apps, icon::APP_WINDOW, "nav-apps"),
                (Page::VJoy, icon::GAME_CONTROLLER, "nav-vjoy"),
                (Page::Viewer, icon::CHART_LINE, "nav-viewer"),
                (Page::Calibration, icon::CROSSHAIR_SIMPLE, "nav-calibration"),
            ] {
                // The Applications page edits the hiding program list; not every platform has one.
                if page == Page::Apps && !gob_hide::capabilities().app_list {
                    continue;
                }
                if style::nav_toggle(ui, self.page == page, glyph, &self.i18n.tr(key)).clicked() {
                    self.page = page;
                }
            }
        });
        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.add_space(8.0);
            let selected = self.page == Page::Settings;
            if style::nav_toggle(ui, selected, icon::GEAR_SIX, &self.i18n.tr("nav-settings"))
                .clicked()
            {
                self.page = Page::Settings;
            }
        });
    }

    /// "{app} | {profile file}[*] | {Active|Inactive}"; just the app name without a profile.
    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match &self.profile {
            None => crate::APP_NAME.to_string(),
            Some(p) => {
                let state = if self.activation.is_active() {
                    "title-active"
                } else {
                    "title-inactive"
                };
                let modified = if p.dirty { "*" } else { "" };
                format!(
                    "{} | {}{modified} | {}",
                    crate::APP_NAME,
                    p.file_name(),
                    self.i18n.tr(state)
                )
            }
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn toggle_runner(&mut self, _ctx: &egui::Context) {
        if self.activation.stop() {
            self.status = self.i18n.tr("status-stopped");
            return;
        }
        if self.profile.is_none() {
            return;
        }
        self.sync_activation();
        // The vJoy tester must not hold a device the profile needs.
        self.vjoy_view.release();
        match self.activation.start() {
            Ok(()) => self.status = self.i18n.tr("status-active"),
            Err(e) => {
                let text = self.i18n.trf("activate-error", &[("error", e.into())]);
                self.error(text);
            }
        }
        self.was_active = self.activation.is_active();
    }

    /// Hands the current profile to the shared activation (for the tray menu too).
    fn sync_activation(&mut self) {
        self.activation
            .set_profile(self.profile.as_ref().map(|p| (&p.profile, p.file_name())));
    }

    /// Main toolbar: always shown, above every page.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let loaded = self.profile.is_some();
        let row = Vec2::new(ui.available_width(), 44.0);
        ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
            if style::toolbar_button(ui, true, icon::FILE_PLUS, &self.i18n.tr("action-new"))
                .clicked()
            {
                self.request(Pending::New, ui.ctx());
            }
            if style::toolbar_button(ui, true, icon::FOLDER_OPEN, &self.i18n.tr("action-open"))
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
            if style::toolbar_button(ui, loaded, save, &self.i18n.tr("action-save")).clicked() {
                self.save();
            }
            if style::toolbar_button(
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
            if style::toolbar_button(
                ui,
                can_undo,
                icon::ARROW_COUNTER_CLOCKWISE,
                &self.i18n.tr("action-undo"),
            )
            .clicked()
            {
                self.history(true);
            }
            if style::toolbar_button(
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
            // Device hiding configuration (its own window); on/off is in the status bar.
            let open = self.hidhide_window;
            let tip = self.i18n.tr("nav-hidhide");
            if style::toolbar_toggle(ui, open, icon::EYE_SLASH, &tip).clicked() {
                self.hidhide_window = !open;
            }
            ui.separator();
            let active = self.activation.is_active();
            let (glyph, tip) = if active {
                (icon::STOP, "action-deactivate")
            } else {
                (icon::PLAY, "action-activate")
            };
            let toggle = ui.add_enabled_ui(loaded, |ui| {
                style::toolbar_toggle(ui, active, glyph, &self.i18n.tr(tip))
            });
            if toggle.inner.clicked() {
                self.toggle_runner(ui.ctx());
            }
        });
    }

    fn devices_page(&mut self, ui: &mut egui::Ui) {
        match self.device_access.show(ui, &self.i18n) {
            Some(Ok(())) => {
                log::info!("device access granted");
                self.status = self.i18n.tr("access-granted");
                // They found no access when they were made: make them again.
                self.vjoy_view = VJoyView::new();
                self.hidhide_view = HidHideView::new();
            }
            Some(Err(e)) => {
                let text = self.i18n.trf("access-error", &[("error", e.into())]);
                self.error(text);
            }
            None => {}
        }
        let no_devices = self.input.devices().iter().all(|d| d.info.is_vjoy());
        // Nothing to edit yet: a new, empty profile and no controllers. Offer the import.
        let empty = self
            .profile
            .as_ref()
            .is_none_or(|p| p.path.is_none() && !p.dirty && p.profile.devices.is_empty());
        if empty && no_devices {
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
                    if ui.button(text).clicked()
                        && let Some(p) = &self.profile
                    {
                        self.substitute =
                            Some(SubstituteDialog::new(&p.profile, &self.input.devices()));
                    }
                });
            });
            ui.add_space(4.0);
        }
        let devices = self.input.devices();
        let mut ops = Vec::new();
        let vjoy = self.vjoy_view.device_list();
        let gaming_only = self.settings.hidhide_gaming_only;
        self.devices_view.hidden = devices
            .iter()
            .filter(|d| !d.info.is_vjoy())
            .filter_map(|d| {
                let hidden = self.hidhide_view.device_hidden(&d.info, gaming_only)?;
                Some((d.info.guid.clone(), hidden))
            })
            .collect();
        self.devices_view.experimental = self.settings.experimental;
        self.devices_view.leds = if self.settings.experimental {
            self.leds.update(&devices, ui.ctx());
            self.leds.guids(&devices)
        } else {
            Default::default()
        };
        self.devices_view.show(
            ui,
            &self.i18n,
            self.profile.as_ref(),
            &devices,
            &mut ops,
            &vjoy,
        );
        if let Some((guid, hide)) = self.devices_view.hide_request.take()
            && let Some(d) = devices.iter().find(|d| d.info.guid == guid)
        {
            self.hidhide_view.set_device_hidden(&d.info, hide);
        }
        drop(devices);
        if std::mem::take(&mut self.devices_view.open_modes) {
            self.show_modes = true;
        }
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
        use crate::action_editor::Op;
        let (previews, ops): (Vec<Op>, Vec<Op>) = ops.into_iter().partition(|op| {
            matches!(
                op,
                Op::PreviewSound { .. } | Op::PreviewSpeech(_) | Op::PreviewLight { .. }
            )
        });
        for op in previews {
            match op {
                Op::PreviewSound { file, volume } => self.media.play(&file, volume),
                Op::PreviewSpeech(request) => self.media.speak(request),
                Op::PreviewLight { device, led } => {
                    let devices = self.input.devices();
                    // A profile device from another PC lights the attached one standing in.
                    let device = self
                        .profile
                        .as_ref()
                        .map(|p| crate::substitute_view::auto_aliases(&p.profile, &devices))
                        .and_then(|a| a.get(&device).cloned())
                        .unwrap_or(device);
                    self.leds.preview(&devices, &device, vec![led.to_led()]);
                }
                _ => {}
            }
        }
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
            Change::Tray => self.update_tray(ctx),
            Change::Autostart(on) => {
                if let Err(e) = crate::tray::autostart::set(on) {
                    let text = self.i18n.trf("autostart-error", &[("error", e.into())]);
                    self.error(text);
                } else {
                    log::info!("start with Windows: {on}");
                }
            }
            Change::Language(code) => {
                self.i18n.set_language(&code);
                self.settings.language = self.i18n.language().to_string();
                self.status = self.i18n.tr("status-ready");
            }
            Change::PreviewLeds { device, leds } => {
                let leds = leds
                    .into_iter()
                    .map(crate::leds::LedValue::to_led)
                    .collect();
                self.leds.preview(&self.input.devices(), &device, leds);
            }
            Change::LedDefaults => {
                self.activation
                    .set_led_defaults(self.settings.led_defaults());
            }
        }
        self.settings.save();
    }
}

impl eframe::App for JoystickGobloonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_activation();
        // Started or stopped from the tray menu (maybe while the window was hidden).
        let active = self.activation.is_active();
        if active != self.was_active {
            self.was_active = active;
            self.status = self.i18n.tr(if active {
                "status-active"
            } else {
                "status-stopped"
            });
        }
        if let Some(prepared) = self.updater.poll(ctx) {
            self.update = Some(prepared);
            self.request(Pending::Update, ctx);
        }
        if let Some(e) = self.activation.take_error() {
            let text = self.i18n.trf("activate-error", &[("error", e.into())]);
            self.error(text);
        }
        self.update_title(ctx);
        if self.hide_frames > 0 {
            self.vjoy_view.release();
            self.hide_frames -= 1;
            self.hide_to_tray(ctx);
            ctx.request_repaint();
        }
        if let Some(tray) = &mut self.tray {
            let active = self.activation.is_active();
            let tip = |key: &str| format!("{} | {}", crate::APP_NAME, self.i18n.tr(key));
            tray.set_active(active, [tip("title-inactive"), tip("title-active")]);
            tray.set_double_click_activates(self.settings.tray_double_click_activates);
            match self.activation.profile_name() {
                Some(name) => {
                    let caption = self.i18n.trf("tray-activate", &[("name", name.into())]);
                    tray.set_profile(&caption, true);
                }
                None => tray.set_profile(&self.i18n.tr("tray-activate-none"), false),
            }
            if tray.take_cloak_changed() {
                self.hidhide_view.reload();
            }
            if let Some(on) = self.hidhide_view.cloak(self.settings.hidhide_gaming_only) {
                tray.set_cloaked(on);
            }
        }
        if self
            .restored
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            // Shown from the tray behind winit's back: make winit agree, and focus it.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if self.tray.as_ref().is_some_and(|t| t.take_quit()) {
            self.request(Pending::Close, ctx);
        }
        // Close button with "close to tray": hide instead of quitting (Quit is in the tray menu).
        let to_tray = self.settings.close_to_tray && self.tray.is_some() && !self.allow_close;
        if to_tray && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_to_tray(ctx);
            self.vjoy_view.release();
        }
        // "Minimize to tray": a minimized window leaves the taskbar; the tray brings it back.
        let minimized = ctx.input(|i| i.viewport().minimized) == Some(true);
        if minimized {
            // The vJoy tester lets go of its device: nobody can see or use it now.
            self.vjoy_view.release();
        }
        if minimized && !self.was_minimized && self.settings.minimize_to_tray && self.tray.is_some()
        {
            self.hide_to_tray(ctx);
        }
        self.was_minimized = minimized;
        if let Some(file) = self.activation.take_load_request() {
            self.request(Pending::Load(PathBuf::from(file)), ctx);
        }
        let dirty = self.profile.as_ref().is_some_and(|p| p.dirty);
        if ctx.input(|i| i.viewport().close_requested()) && dirty && !self.allow_close && !to_tray {
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
            if !self.activation.is_active() {
                self.edited = None;
            } else if t.elapsed() >= settle {
                self.edited = None;
                self.activation.stop();
                self.toggle_runner(ctx);
            } else {
                ctx.request_repaint_after(settle - t.elapsed());
            }
        }
        self.input.set_event_mode(self.settings.input_events);
        let in_tray = self.main_window.is_some_and(crate::platform::in_tray);
        let shown = !in_tray && self.main_window.is_none_or(crate::platform::window_shown);
        if in_tray && self.pending.is_some() {
            // A question needs the window, e.g. Quit from the tray with unsaved changes.
            if let Some(main_window) = self.main_window {
                crate::platform::show_window(main_window);
            }
            self.restored
                .store(true, std::sync::atomic::Ordering::Relaxed);
            ctx.request_repaint();
        }
        // Detached windows stay on screen while the main window is only minimized.
        let live = !in_tray
            && ((shown && matches!(self.page, Page::Devices | Page::Viewer | Page::Calibration))
                || self.viewer_window
                || self.calibration_window);
        self.live_input
            .store(live, std::sync::atomic::Ordering::Relaxed);
        // Always drained; events from before the window came back are stale and only dropped.
        let focus =
            self.settings.focus_used_input && shown && self.was_shown && self.page == Page::Devices;
        self.devices_view
            .handle_events(self.input.events(), &self.input.devices(), focus);
        self.was_shown = shown;
        // In the tray only the logic above runs: nothing is drawn, so no page or window
        // schedules repaints. Detached windows close and come back with the main window.
        if in_tray {
            return;
        }
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(60.0)
            .show(ctx, |ui| self.nav_rail(ui));
        let margin = egui::Margin {
            left: 16,
            right: 16,
            top: 4,
            bottom: 4,
        };
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(margin))
            .show(ctx, |ui| self.toolbar(ui));

        let cloak = self.hidhide_view.cloak(self.settings.hidhide_gaming_only);
        let mut switch_cloak = None;
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Device hiding indicator, clicked to switch it: green crossed eye while on.
                if let Some(on) = cloak {
                    let (glyph, tip) = if on {
                        (icon::EYE_SLASH, "hidhide-on")
                    } else {
                        (icon::EYE, "hidhide-off")
                    };
                    let text = RichText::new(glyph).size(14.0);
                    let text = if on {
                        text.color(style::accent(ui))
                    } else {
                        text.weak()
                    };
                    let response = ui
                        .add(egui::Button::new(text).frame(false))
                        .on_hover_text(self.i18n.tr(tip));
                    if response.clicked() {
                        switch_cloak = Some(!on);
                    }
                    ui.separator();
                }
                if let Some(status) = self.activation.status() {
                    let mode = status.mode;
                    if status.paused {
                        ui.label(
                            RichText::new(icon::PAUSE)
                                .size(12.0)
                                .color(ui.visuals().warn_fg_color),
                        )
                        .on_hover_text(self.i18n.tr("status-paused"));
                    }
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
        if let Some(on) = switch_cloak {
            self.hidhide_view.set_cloak(on);
        }

        if self.page != Page::VJoy {
            self.vjoy_view.release();
        }
        let frame = egui::Frame::central_panel(&ctx.style()).inner_margin(16);
        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| match self.page {
                Page::Devices => self.devices_page(ui),
                Page::Apps => {
                    self.apps_view.show(ui, &self.i18n, &self.settings);
                }
                Page::VJoy => {
                    let locked = self.activation.is_active();
                    self.vjoy_view.show(ui, &self.i18n, locked);
                }
                Page::Viewer if self.viewer_window => {
                    empty_state(
                        ui,
                        icon::ARROW_SQUARE_OUT,
                        &self.i18n.tr("viewer-in-window"),
                    );
                    ui.vertical_centered(|ui| {
                        let text = format!(
                            "{}  {}",
                            icon::ARROW_SQUARE_IN,
                            self.i18n.tr("viewer-attach")
                        );
                        if ui.button(text).clicked() {
                            self.viewer_window = false;
                        }
                    });
                }
                Page::Viewer => {
                    let devices = self.input.devices().clone();
                    if self.viewer_view.show(ui, &self.i18n, &devices, false) {
                        self.viewer_window = true;
                    }
                }
                Page::Calibration if self.calibration_window => {
                    empty_state(
                        ui,
                        icon::ARROW_SQUARE_OUT,
                        &self.i18n.tr("calibration-in-window"),
                    );
                    ui.vertical_centered(|ui| {
                        let text = format!(
                            "{}  {}",
                            icon::ARROW_SQUARE_IN,
                            self.i18n.tr("viewer-attach")
                        );
                        if ui.button(text).clicked() {
                            self.calibration_window = false;
                        }
                    });
                }
                Page::Calibration => {
                    let devices = self.input.devices().clone();
                    let out = self.calibration_view.show(
                        ui,
                        &self.i18n,
                        &devices,
                        &self.calibrations,
                        false,
                    );
                    if out.toggle_window {
                        self.calibration_window = true;
                    }
                    self.apply_calibration(out);
                }
                Page::Settings => {
                    let devices = self.input.devices();
                    let led_devices = if self.settings.experimental {
                        self.leds.update(&devices, ctx);
                        self.leds.attached(&devices)
                    } else {
                        Vec::new()
                    };
                    drop(devices);
                    let change = settings_view::show(
                        ui,
                        &self.i18n,
                        &mut self.settings,
                        &mut self.settings_tab,
                        &self.logo,
                        &mut self.updater,
                        &led_devices,
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
                self.activation.stop();
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
            if !ops.is_empty()
                && let Some(loaded) = &mut self.profile
            {
                for op in ops {
                    loaded.checkpoint();
                    crate::modes_view::apply(&mut loaded.profile, op);
                }
                loaded.refresh();
                self.edited = Some(std::time::Instant::now());
            }
        }
        self.unsaved_dialog(ctx);
        self.alert_dialog(ctx);
        self.detached_windows(ctx);
    }
}

/// Loads the plugin DLLs next to the exe and logs what was found (plugins are a stub so far).
fn load_plugins() -> gob_plugin::Registry {
    let mut registry = gob_plugin::Registry::default();
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("plugins")))
    else {
        return registry;
    };
    for e in registry.load_dir(&dir) {
        log::warn!("plugin skipped: {e}");
    }
    for p in registry.loaded() {
        log::info!(
            "plugin loaded: {} {} ({})",
            p.name,
            p.version,
            p.path.display()
        );
    }
    registry
}

/// Default size of a detached page window.
const BIG_WINDOW: [f32; 2] = [1000.0, 700.0];

/// Shows `content` in its own native window (egui viewport) titled "<app> | <title>".
/// Returns true when it should close or go back into the main window (its button or closing it).
fn detached_window(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    size: [f32; 2],
    mut content: impl FnMut(&mut egui::Ui) -> bool,
) -> bool {
    let viewport = egui::ViewportId::from_hash_of(id);
    // Opens centered over the main window. The position is fixed when it opens, so the
    // window can be moved afterwards (egui re-applies only builder changes).
    let pos_id = egui::Id::new(("window-pos", id));
    let main = ctx.input(|i| i.viewport().outer_rect);
    let pos = ctx.data(|d| d.get_temp::<egui::Pos2>(pos_id)).or_else(|| {
        let r = main?;
        let corner = (r.center() - egui::vec2(size[0], size[1]) / 2.0).max(r.min);
        ctx.data_mut(|d| d.insert_temp(pos_id, corner));
        Some(corner)
    });
    let mut builder = egui::ViewportBuilder::default()
        .with_title(format!("{} | {title}", crate::APP_NAME))
        .with_inner_size(size)
        .with_min_inner_size([480.0, 320.0]);
    if let Some(pos) = pos {
        builder = builder.with_position(pos);
    }
    if let Ok(icon) = eframe::icon_data::from_png_bytes(LOGO_PNG) {
        builder = builder.with_icon(icon);
    }
    let mut attach = false;
    ctx.show_viewport_immediate(viewport, builder, |ctx, _class| {
        egui::CentralPanel::default().show(ctx, |ui| {
            attach |= content(ui);
        });
        attach |= ctx.input(|i| i.viewport().close_requested());
    });
    if attach {
        // Next time it opens over the main window again.
        ctx.data_mut(|d| d.remove::<egui::Pos2>(pos_id));
    }
    attach
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
