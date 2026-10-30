use std::path::{Path, PathBuf};

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
    wizard: Option<crate::wizard_view::Wizard>,
    /// The Devices page banner started setting up the profile's virtual devices.
    virtual_setup: bool,
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
    system_part: crate::system_part_view::SystemPartView,
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
    /// Save waiting for "Joystick Gremlin will not open it": the file, then the action to run.
    gremlin_warning: Option<(PathBuf, Option<Pending>)>,
    /// The "don't ask again" switch of that dialog.
    gremlin_dont_ask: bool,
    /// Time of a single click on the device hiding eye, waiting to see if a second one follows.
    cloak_click: Option<f64>,
    /// Asking whether to switch device hiding to this state.
    cloak_confirm: Option<bool>,
    /// The user already agreed to close without saving.
    allow_close: bool,
    /// The window was minimized last frame ("minimize to tray" acts once per minimize).
    was_minimized: bool,
    show_modes: bool,
    modes_window: crate::modes_view::ModesWindow,
    show_modifiers: bool,
    modifiers_window: crate::modifiers_view::ModifiersWindow,
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
    /// A new profile through the wizard (`wizard_view`).
    Wizard,
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
        style::follow_system_theme(&cc.egui_ctx);
        // Double clicks (curve points, the hide button) follow the system's setting.
        let double_click = crate::platform::double_click_time().as_secs_f64();
        cc.egui_ctx
            .options_mut(|o| o.input_options.max_double_click_delay = double_click);

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
            wizard: None,
            virtual_setup: false,
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
            system_part: crate::system_part_view::SystemPartView::new(),
            activation,
            was_active: false,
            edited: None,
            pending: None,
            gremlin_warning: None,
            gremlin_dont_ask: false,
            cloak_click: None,
            cloak_confirm: None,
            allow_close: false,
            was_minimized: false,
            show_modes: false,
            modes_window: Default::default(),
            show_modifiers: false,
            modifiers_window: Default::default(),
            title: crate::APP_NAME.into(),
            media: Default::default(),
            leds: Default::default(),
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
        for (device, ms) in &app.settings.debounce {
            app.input.set_debounce(&device.to_ascii_lowercase(), *ms);
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
            Ok("modifiers") => app.show_modifiers = true,
            Ok("apps") => app.page = Page::Apps,
            Ok("vjoy") => app.page = Page::VJoy,
            Ok("viewer") => app.page = Page::Viewer,
            Ok("calibration") => app.page = Page::Calibration,
            Ok("wizard") => app.wizard = Some(crate::wizard_view::Wizard::new()),
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
        // Nothing (re)opened: start with a new profile, so bindings can be added right away.
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

    /// File dialog for profiles, starting in `dir`, else where the last one was opened or
    /// saved, else in the documents folder.
    fn profile_dialog(&self, dir: Option<&Path>) -> rfd::FileDialog {
        let dialog =
            rfd::FileDialog::new().add_filter(self.i18n.tr("file-filter-gremlin"), &["xml"]);
        let dir = dir
            .map(Path::to_path_buf)
            .or_else(|| self.settings.profile_dir.clone().filter(|d| d.is_dir()))
            .or_else(crate::platform::documents_dir);
        match dir {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        }
    }

    /// Remembers the folder of a profile picked in a file dialog.
    fn remember_profile_dir(&mut self, path: &Path) {
        self.settings.profile_dir = path.parent().map(Path::to_path_buf);
        self.settings.save();
    }

    fn import_dialog(&mut self) {
        if let Some(path) = self.profile_dialog(None).pick_file() {
            self.remember_profile_dir(&path);
            self.open_profile(path, true);
        }
    }

    /// Saves to the current file, asking for one when the profile is new.
    fn save(&mut self) -> bool {
        self.save_then(None)
    }

    /// [`Self::save`], then `after` once saved. Returns `false` when not saved now (cancelled,
    /// failed, or waiting for the Joystick Gremlin question, which runs `after` itself).
    fn save_then(&mut self, after: Option<Pending>) -> bool {
        match self.profile.as_ref().and_then(|p| p.path.clone()) {
            Some(path) => self.save_checked(path, after),
            None => self.save_as_then(after),
        }
    }

    /// Saves, unless that makes a file Joystick Gremlin opens one it cannot: then asks first.
    fn save_checked(&mut self, path: PathBuf, after: Option<Pending>) -> bool {
        self.store_virtual_devices();
        let ask = self.settings.warn_gremlin_format
            && self
                .profile
                .as_ref()
                .is_some_and(|p| p.gremlin_file && p.profile.needs_own_format());
        if ask {
            self.gremlin_dont_ask = false;
            self.gremlin_warning = Some((path, after));
            return false;
        }
        self.save_to(path)
    }

    /// Puts the virtual devices the profile writes to, as set up here, into the profile.
    fn store_virtual_devices(&mut self) {
        let devices = self.vjoy_view.device_list();
        let Some(loaded) = &mut self.profile else {
            return;
        };
        let configs = self.vjoy_view.configs();
        let taken = crate::virtual_setup::capture(&loaded.profile, &devices, configs);
        if taken != loaded.profile.virtual_devices {
            loaded.profile.set_virtual_devices(taken);
        }
    }

    /// Banner when the profile's virtual devices are missing here or smaller than stored, with
    /// a button that sets them up.
    fn virtual_devices_banner(&mut self, ui: &mut egui::Ui) {
        let Some(p) = &self.profile else { return };
        if p.profile.virtual_devices.is_empty() {
            return;
        }
        let Some(caps) = self.vjoy_view.capabilities() else {
            return;
        };
        let devices = self.vjoy_view.device_list();
        let (_, pads) = self.vjoy_view.gamepad_list();
        let changes = crate::virtual_setup::changes(
            &p.profile.virtual_devices,
            &devices,
            self.vjoy_view.configs(),
            &pads,
            &caps,
        );
        if changes.is_empty() {
            return;
        }
        let names: Vec<String> = changes
            .iter()
            .filter_map(|c| match c {
                gob_virtual::Change::Set(c) => {
                    Some(self.i18n.trf("vjoy-device-n", &[("n", c.id.into())]))
                }
                gob_virtual::Change::AddGamepad(id) => {
                    Some(self.i18n.trf("gamepad-n", &[("n", (*id).into())]))
                }
                _ => None,
            })
            .collect();
        let busy = self.vjoy_view.busy();
        let active = self.activation.is_active();
        let banner = egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(8)
            .inner_margin(8);
        banner.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::GAME_CONTROLLER).size(style::ICON_SIZE));
                ui.label(
                    self.i18n
                        .trf("virtual-banner", &[("devices", names.join(", ").into())]),
                );
                let text = format!("{}  {}", icon::WRENCH, self.i18n.tr("virtual-setup"));
                let tip = self.i18n.tr(if active {
                    "virtual-setup-active"
                } else if caps.needs_admin {
                    "virtual-setup-admin"
                } else {
                    "virtual-setup-tip"
                });
                let r = ui
                    .add_enabled(!busy && !active, egui::Button::new(text))
                    .on_hover_text(&tip)
                    .on_disabled_hover_text(&tip);
                if r.clicked() {
                    self.virtual_setup = true;
                    self.vjoy_view.apply(changes);
                }
                if busy {
                    ui.spinner();
                }
            });
        });
        ui.add_space(4.0);
    }

    fn save_as(&mut self) -> bool {
        self.save_as_then(None)
    }

    fn save_as_then(&mut self, after: Option<Pending>) -> bool {
        let Some(loaded) = &self.profile else {
            return false;
        };
        let dir = loaded.path.as_ref().and_then(|p| p.parent());
        let dialog = self.profile_dialog(dir).set_file_name(loaded.file_name());
        match dialog.save_file() {
            Some(path) => {
                self.remember_profile_dir(&path);
                self.save_checked(path, after)
            }
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
                loaded.gremlin_file = !loaded.profile.needs_own_format();
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

    /// A new profile with every controller automapped onto the virtual joystick the wizard
    /// chose for it.
    fn create_from_wizard(&mut self, plan: crate::wizard_view::Plan) {
        self.new_profile();
        let Some(loaded) = &mut self.profile else {
            return;
        };
        for m in &plan.maps {
            loaded
                .profile
                .automap(&m.device, &m.device_name, &m.from, m.vjoy, &m.to, "Default");
            let missing = m.from.missing_in(&m.to);
            log::info!(
                "wizard: {} -> vJoy {}, unmapped: {} axes, {} buttons, {} hats",
                m.device,
                m.vjoy,
                missing.axes.len(),
                missing.buttons,
                missing.hats
            );
        }
        loaded.name = Some(plan.name.clone());
        loaded.refresh();
        self.status = self
            .i18n
            .trf("status-wizard", &[("name", plan.name.into())]);
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
        // An empty profile has nothing worth keeping: no question for it.
        if self
            .profile
            .as_ref()
            .is_some_and(|p| p.dirty && !p.profile.inputs.is_empty())
        {
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
            Pending::Wizard => self.wizard = Some(crate::wizard_view::Wizard::new()),
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
        let devices = self.input.devices();
        if self.viewer_window {
            let mut devices = devices.clone();
            devices.extend(self.logical_device());
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
            let debounce = &self.settings.debounce;
            let title = i18n.tr("nav-calibration");
            let mut out = crate::calibration_view::Output::default();
            let attach = detached_window(ctx, "calibration", &title, BIG_WINDOW, |ui| {
                out = view.show(ui, i18n, &devices, saved, debounce, true);
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
        if let Some((device, ms)) = out.debounce {
            log::info!("debounce of {device}: {ms} ms");
            self.input.set_debounce(&device, ms);
            if ms == 0 {
                self.settings.debounce.remove(&device);
            } else {
                self.settings.debounce.insert(device, ms);
            }
            self.settings.save();
        } else if !self.calibrations.is_empty() {
            self.settings.save();
        }
    }

    /// Warning/error dialogs, one at a time.
    fn alert_dialog(&mut self, ctx: &egui::Context) {
        let Some(alert) = self.alerts.first() else {
            return;
        };
        let mut close = false;
        style::modal(ctx, "alert").show(ctx, |ui| {
            ui.set_width(420.0);
            let warn = ui.visuals().warn_fg_color;
            style::dialog_title(ui, Some((icon::WARNING, Some(warn))), &alert.title);
            for line in &alert.lines {
                ui.label(line);
            }
            let log = crate::logfile::path();
            let mut buttons = Vec::new();
            if log.is_some() {
                let text = format!("{}  {}", icon::FILE_TEXT, self.i18n.tr("alert-open-log"));
                buttons.push(style::DialogButton::secondary(text));
            }
            buttons.push(style::DialogButton::primary(self.i18n.tr("ok")));
            match style::dialog_buttons(ui, &buttons) {
                Some(i) if i + 1 == buttons.len() => close = true,
                Some(_) => {
                    if let Some(path) = &log {
                        crate::platform::open_text_file(path);
                    }
                }
                None => {}
            }
        });
        if close {
            self.alerts.remove(0);
        }
    }

    /// "Joystick Gremlin will not open it" before a save that makes it so.
    fn gremlin_dialog(&mut self, ctx: &egui::Context) {
        let Some((path, _)) = &self.gremlin_warning else {
            return;
        };
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut choice = None;
        style::modal(ctx, "gremlin-format").show(ctx, |ui| {
            ui.set_width(420.0);
            let warn = ui.visuals().warn_fg_color;
            let title = self.i18n.tr("gremlin-format-title");
            style::dialog_title(ui, Some((icon::WARNING, Some(warn))), &title);
            ui.label(
                self.i18n
                    .trf("gremlin-format-text", &[("file", file.into())]),
            );
            ui.add_space(style::SPACE_M);
            ui.add(style::switch(
                &mut self.gremlin_dont_ask,
                self.i18n.tr("gremlin-format-dont-ask"),
            ));
            let save = format!("{}  {}", icon::FLOPPY_DISK, self.i18n.tr("action-save"));
            let buttons = [
                style::DialogButton::primary(save),
                style::DialogButton::secondary(self.i18n.tr("cancel")),
            ];
            if let Some(i) = style::dialog_buttons(ui, &buttons) {
                choice = Some(i == 0);
            }
        });
        let Some(save) = choice else { return };
        let Some((path, after)) = self.gremlin_warning.take() else {
            return;
        };
        if self.gremlin_dont_ask {
            self.settings.warn_gremlin_format = false;
            self.settings.save();
        }
        if save
            && self.save_to(path)
            && let Some(action) = after
        {
            self.perform(action, ctx);
        }
    }

    /// "Turn device hiding on / off?" after a single click on the status bar eye.
    fn cloak_dialog(&mut self, ctx: &egui::Context) {
        let Some(on) = self.cloak_confirm else {
            return;
        };
        let (glyph, title, text, action) = if on {
            (
                icon::EYE_SLASH,
                "hidhide-confirm-on-title",
                "hidhide-confirm-on-text",
                "action-turn-on",
            )
        } else {
            (
                icon::EYE,
                "hidhide-confirm-off-title",
                "hidhide-confirm-off-text",
                "action-turn-off",
            )
        };
        let mut choice = None;
        let modal = style::modal(ctx, "cloak-confirm").show(ctx, |ui| {
            ui.set_width(380.0);
            style::dialog_title(ui, Some((glyph, None)), &self.i18n.tr(title));
            ui.label(self.i18n.tr(text));
            ui.add_space(style::SPACE_M);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::INFO).color(style::accent(ui)));
                    ui.label(RichText::new(self.i18n.tr("hidhide-confirm-hint")).weak());
                });
            });
            let buttons = [
                style::DialogButton::primary(format!("{glyph}  {}", self.i18n.tr(action))),
                style::DialogButton::secondary(self.i18n.tr("cancel")),
            ];
            if let Some(i) = style::dialog_buttons(ui, &buttons) {
                choice = Some(i == 0);
            }
        });
        if modal.should_close() {
            choice.get_or_insert(false);
        }
        if let Some(switch) = choice {
            self.cloak_confirm = None;
            if switch {
                self.hidhide_view.set_cloak(on);
            }
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
        style::modal(ctx, "unsaved").show(ctx, |ui| {
            ui.set_width(420.0);
            let title = self.i18n.tr("unsaved-title");
            style::dialog_title(ui, Some((icon::FLOPPY_DISK, None)), &title);
            ui.label(self.i18n.trf("unsaved-text", &[("file", file.into())]));
            let save = format!("{}  {}", icon::FLOPPY_DISK, self.i18n.tr("unsaved-save"));
            let buttons = [
                style::DialogButton::primary(save),
                style::DialogButton::secondary(self.i18n.tr("unsaved-discard")),
                style::DialogButton::secondary(self.i18n.tr("cancel")),
            ];
            choice = style::dialog_buttons(ui, &buttons);
        });
        match choice {
            Some(0) => {
                self.pending = None;
                if self.save_then(Some(action.clone())) {
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

    /// Applies the edited profile to the running engine without releasing its virtual devices.
    fn reload_runner(&mut self) {
        self.sync_activation();
        // The vJoy tester must not hold a device the profile now needs.
        self.vjoy_view.release();
        match self.activation.reload() {
            Ok(()) => self.status = self.i18n.tr("status-active"),
            Err(e) => {
                let text = self.i18n.trf("activate-error", &[("error", e.into())]);
                self.error(text);
            }
        }
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
            if style::toolbar_button(ui, true, icon::MAGIC_WAND, &self.i18n.tr("wizard-open"))
                .clicked()
            {
                self.request(Pending::Wizard, ui.ctx());
            }
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

    /// The profile's logical device with the values of the running profile.
    fn logical_device(&self) -> Option<gob_input::DeviceState> {
        crate::viewer_view::logical_device(
            &self.profile.as_ref()?.profile,
            self.activation.logical().as_ref(),
            self.i18n.tr("logical-device"),
        )
    }

    /// After the system part was installed or removed, the pages find their backends again
    /// (the service's, or their own).
    fn poll_system_part(&mut self) {
        use crate::system_part_view::Job;
        let Some((job, result)) = self.system_part.poll() else {
            return;
        };
        match result {
            Ok(()) => {
                let key = match job {
                    Job::Install => "system-installed",
                    Job::Remove => "system-removed",
                };
                self.status = self.i18n.tr(key);
                self.vjoy_view.release();
                self.vjoy_view = VJoyView::new();
                self.hidhide_view = HidHideView::new();
                self.device_access = crate::device_access::DeviceAccess::new();
            }
            Err(e) => {
                let text = self.i18n.trf("system-error", &[("error", e.into())]);
                self.error(text);
            }
        }
    }

    fn devices_page(&mut self, ui: &mut egui::Ui) {
        // Installing the system part gives the access too: one offer at a time.
        let offered = self.system_part.banner(ui, &self.i18n);
        let access = if offered {
            None
        } else {
            self.device_access.show(ui, &self.i18n)
        };
        match access {
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
        let no_devices = self.input.devices().iter().all(|d| d.info.is_own());
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
        let any_physical = self.input.devices().iter().any(|d| !d.info.is_own());
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
        self.virtual_devices_banner(ui);
        self.devices_view.logical = self.logical_device();
        let devices = self.input.devices();
        let mut ops = Vec::new();
        let vjoy = self.vjoy_view.device_list();
        (self.devices_view.max_gamepads, self.devices_view.gamepads) =
            self.vjoy_view.gamepad_list();
        let gaming_only = self.settings.hidhide_gaming_only;
        self.hidhide_view
            .track_inputs(devices.iter().map(|d| d.info.guid.clone()).collect());
        self.devices_view.hiding_on = self.hidhide_view.cloak(gaming_only) == Some(true);
        self.devices_view.hidden = devices
            .iter()
            .filter(|d| !d.info.is_own())
            .filter_map(|d| {
                let hidden = self.hidhide_view.device_hidden(&d.info, gaming_only)?;
                Some((d.info.guid.clone(), hidden))
            })
            .collect();
        self.devices_view.exposed = devices
            .iter()
            .filter(|d| self.hidhide_view.device_exposed(&d.info))
            .map(|d| d.info.guid.clone())
            .collect();
        self.devices_view.experimental = self.settings.experimental;
        self.devices_view.monotonic_curves = self.settings.monotonic_curves;
        self.devices_view.curve_right_click_invert = self.settings.curve_right_click_invert;
        self.devices_view.leds = if self.settings.experimental {
            self.leds.update(&devices, ui.ctx());
            self.leds.guids(&devices)
        } else {
            Default::default()
        };
        self.devices_view.system_part_offered = offered;
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
        match self.devices_view.all_buttons_result.take() {
            Some(Ok(())) => {
                log::info!("all buttons allowed");
                self.status = self.i18n.tr("all-buttons-allowed");
            }
            Some(Err(e)) => {
                let text = self.i18n.trf("all-buttons-error", &[("error", e.into())]);
                self.error(text);
            }
            None => {}
        }
        match self.devices_view.keyboard_result.take() {
            Some((allow, Ok(()))) => {
                log::info!(
                    "keyboard access {}",
                    if allow { "allowed" } else { "removed" }
                );
                self.status = self.i18n.tr(if allow {
                    "keyboard-access-allowed"
                } else {
                    "keyboard-access-removed"
                });
            }
            Some((_, Err(e))) => {
                let text = self
                    .i18n
                    .trf("keyboard-access-error", &[("error", e.into())]);
                self.error(text);
            }
            None => {}
        }
        if std::mem::take(&mut self.devices_view.open_modes) {
            self.show_modes = true;
        }
        if std::mem::take(&mut self.devices_view.open_modifiers) {
            self.show_modifiers = true;
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
        if let Some(ok) = self.vjoy_view.poll(&self.i18n)
            && std::mem::take(&mut self.virtual_setup)
        {
            if ok {
                self.status = self.i18n.tr("virtual-setup-done");
            } else if let Some(Err(e)) = self.vjoy_view.config_message() {
                let text = e.clone();
                self.error(text);
            }
        }
        if self.vjoy_view.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
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
                self.reload_runner();
            } else {
                ctx.request_repaint_after(settle - t.elapsed());
            }
        }
        self.input.set_event_mode(self.settings.input_events);
        self.input
            .set_poll_interval(std::time::Duration::from_millis(u64::from(
                self.settings.poll_interval_ms,
            )));
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
        let modifiers = match &self.profile {
            Some(p) if self.settings.focus_skips_modifiers => p.profile.modifiers.as_slice(),
            _ => &[],
        };
        self.devices_view.handle_events(
            self.input.events(),
            &self.input.devices(),
            focus,
            modifiers,
        );
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
                // Device hiding indicator: green crossed eye while on. A click asks before
                // switching it, a double click switches it at once.
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
                    if response.double_clicked() {
                        self.cloak_click = None;
                        switch_cloak = Some(!on);
                    } else if response.clicked() {
                        self.cloak_click = Some(ui.input(|i| i.time));
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
                    if let Some(latency) = status.latency {
                        let ms = |d: std::time::Duration| format!("{:.2}", d.as_secs_f64() * 1e3);
                        ui.separator();
                        ui.label(
                            RichText::new(format!("{}  {} ms", icon::TIMER, ms(latency.average)))
                                .size(12.0),
                        )
                        .on_hover_text(self.i18n.trf(
                            "status-latency",
                            &[
                                ("average", ms(latency.average).into()),
                                ("max", ms(latency.max).into()),
                            ],
                        ));
                    }
                    ui.separator();
                }
                ui.label(RichText::new(&self.status).size(12.0).weak());
            });
        });
        if let Some(clicked) = self.cloak_click {
            let wait = ctx.options(|o| o.input_options.max_double_click_delay);
            let left = clicked + wait - ctx.input(|i| i.time);
            if left <= 0.0 {
                self.cloak_click = None;
                self.cloak_confirm = cloak.map(|on| !on);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
            }
        }
        if let Some(on) = switch_cloak {
            self.hidhide_view.set_cloak(on);
        }

        if self.page != Page::VJoy {
            self.vjoy_view.release();
        }
        self.poll_system_part();
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
                    let name = self.profile.as_ref().map(|p| p.file_name());
                    let mut marks = self.profile.as_ref().map(|p| {
                        crate::output_marks::OutputMarks::new(&p.profile, name.as_deref(), None)
                    });
                    self.vjoy_view.show(ui, &self.i18n, locked, marks.as_mut());
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
                    let mut devices = self.input.devices();
                    devices.extend(self.logical_device());
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
                    let devices = self.input.devices();
                    let out = self.calibration_view.show(
                        ui,
                        &self.i18n,
                        &devices,
                        &self.calibrations,
                        &self.settings.debounce,
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
                    let change = settings_view::show(
                        ui,
                        &self.i18n,
                        &mut self.settings,
                        &mut self.settings_tab,
                        &self.logo,
                        &mut self.updater,
                        settings_view::Attached {
                            led_devices: &led_devices,
                            polls_devices: self.input.polls_devices(),
                            system_part: &mut self.system_part,
                        },
                    );
                    if let Some(change) = change {
                        self.apply_change(ctx, change);
                    }
                }
            });

        if self.wizard.is_some() {
            let devices = self.input.devices();
            let vjoy = self.vjoy_view.device_list();
            let mut open = true;
            let plan = self
                .wizard
                .as_mut()
                .and_then(|w| w.show(ctx, &self.i18n, &devices, &vjoy, &mut open));
            if let Some(plan) = plan {
                self.create_from_wizard(plan);
                open = false;
            }
            if !open {
                self.wizard = None;
            }
        }

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
        if let (true, Some(loaded)) = (self.show_modifiers, &self.profile) {
            let mut open = true;
            let devices = self.input.devices();
            let ops =
                self.modifiers_window
                    .show(ctx, &self.i18n, &loaded.profile, &devices, &mut open);
            self.show_modifiers = open;
            if !ops.is_empty()
                && let Some(loaded) = &mut self.profile
            {
                for op in ops {
                    loaded.checkpoint();
                    crate::modifiers_view::apply(&mut loaded.profile, op);
                }
                loaded.refresh();
                self.edited = Some(std::time::Instant::now());
            }
        }
        self.unsaved_dialog(ctx);
        self.gremlin_dialog(ctx);
        self.cloak_dialog(ctx);
        self.alert_dialog(ctx);
        self.detached_windows(ctx);
    }
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
