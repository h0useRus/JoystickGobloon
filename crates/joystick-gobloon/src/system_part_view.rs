//! The system part (SystemGobloon on Linux, `platform::system_part`): a banner on the Devices page
//! while it is missing, stopped or of another version, and a section in the settings to install,
//! update or remove it. Installing and removing ask for the administrator password, so they run
//! on a worker thread.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use gob_i18n::I18n;

use crate::platform::SystemPart;
use crate::style::{self, icon};

/// Checked again this often: the user may stop or start the service elsewhere.
const RECHECK: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Install,
    Remove,
}

pub struct SystemPartView {
    state: SystemPart,
    checked: Instant,
    job: Option<(Job, Receiver<Result<(), String>>)>,
    /// "Not now" on the banner, for this run.
    dismissed: bool,
    confirm_remove: bool,
}

impl SystemPartView {
    pub fn new() -> Self {
        Self {
            state: crate::platform::system_part(crate::VERSION),
            checked: Instant::now(),
            job: None,
            dismissed: false,
            confirm_remove: false,
        }
    }

    fn state(&mut self) -> &SystemPart {
        if self.job.is_none() && self.checked.elapsed() > RECHECK {
            self.state = crate::platform::system_part(crate::VERSION);
            self.checked = Instant::now();
        }
        &self.state
    }

    /// A job that finished in this frame and its result.
    pub fn poll(&mut self) -> Option<(Job, Result<(), String>)> {
        let (job, rx) = self.job.as_ref()?;
        let result = rx.try_recv().ok()?;
        let job = *job;
        self.job = None;
        self.state = crate::platform::system_part(crate::VERSION);
        self.checked = Instant::now();
        match &result {
            Ok(()) => log::info!("SystemGobloon: {job:?} done, now {:?}", self.state),
            Err(e) => log::error!("SystemGobloon: {job:?} failed: {e}"),
        }
        Some((job, result))
    }

    fn start(&mut self, ctx: &egui::Context, job: Job) {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        log::info!("SystemGobloon: {job:?}");
        let spawned = std::thread::Builder::new()
            .name("system-part".into())
            .spawn(move || {
                let result = match job {
                    Job::Install => crate::platform::install_system_part(),
                    Job::Remove => remove(),
                };
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.job = Some((job, rx)),
            Err(e) => log::error!("SystemGobloon: {e}"),
        }
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn spinner(&self, ui: &mut egui::Ui) {
        if self.busy() {
            ui.spinner();
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    }

    /// Install, Update or Start, as the state asks; nothing when it is current.
    fn install_button(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        let key = match self.state {
            SystemPart::NotInstalled => "system-install",
            SystemPart::Outdated { .. } => "system-update",
            SystemPart::Stopped => "system-start",
            _ => return,
        };
        let text = format!("{}  {}", icon::SHIELD_CHECK, i18n.tr(key));
        let button = ui
            .add_enabled(!self.busy(), egui::Button::new(text))
            .on_hover_text(i18n.tr("system-admin"));
        if button.clicked() {
            self.start(ui.ctx(), Job::Install);
        }
    }

    /// Devices page: offers the system part while it is not running in this program's version;
    /// whether it is shown.
    pub fn banner(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> bool {
        let text = match self.state().clone() {
            SystemPart::NotInstalled => i18n.tr("system-banner-install"),
            SystemPart::Stopped => i18n.tr("system-banner-stopped"),
            SystemPart::Outdated { version } => {
                i18n.trf("system-banner-update", &[("version", version.into())])
            }
            SystemPart::Unavailable | SystemPart::Current { .. } => return false,
        };
        if self.dismissed {
            return false;
        }
        let banner = egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(8)
            .inner_margin(8);
        banner.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(icon::SHIELD_CHECK).size(style::ICON_SIZE));
                ui.label(text);
                self.install_button(ui, i18n);
                self.spinner(ui);
                if !self.busy()
                    && style::icon_button(ui, true, icon::X, &i18n.tr("system-not-now")).clicked()
                {
                    self.dismissed = true;
                }
            });
        });
        ui.add_space(4.0);
        true
    }

    /// Settings section: what runs, and the buttons to install, update or remove it.
    pub fn section(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        let status = match self.state().clone() {
            SystemPart::Unavailable => return,
            SystemPart::NotInstalled => i18n.tr("system-status-missing"),
            SystemPart::Stopped => i18n.tr("system-status-stopped"),
            SystemPart::Outdated { version } => {
                i18n.trf("system-status-outdated", &[("version", version.into())])
            }
            SystemPart::Current { version } => {
                i18n.trf("system-status-current", &[("version", version.into())])
            }
        };
        ui.add_space(18.0);
        ui.label(RichText::new(format!("{}  SystemGobloon", icon::SHIELD_CHECK)).strong());
        ui.add_space(4.0);
        ui.label(RichText::new(i18n.tr("system-about")).weak());
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(status);
            self.install_button(ui, i18n);
            let installed = !matches!(self.state, SystemPart::NotInstalled);
            if installed {
                let text = format!("{}  {}", icon::TRASH, i18n.tr("system-remove"));
                if ui
                    .add_enabled(!self.busy(), egui::Button::new(text))
                    .on_hover_text(i18n.tr("system-admin"))
                    .clicked()
                {
                    self.confirm_remove = true;
                }
            }
            self.spinner(ui);
        });
        self.remove_dialog(ui.ctx(), i18n);
    }

    fn remove_dialog(&mut self, ctx: &egui::Context, i18n: &I18n) {
        if !self.confirm_remove {
            return;
        }
        let mut choice = None;
        style::modal(ctx, "system-remove").show(ctx, |ui| {
            ui.set_width(440.0);
            style::dialog_title(
                ui,
                Some((icon::TRASH, None)),
                &i18n.tr("system-remove-title"),
            );
            ui.label(i18n.tr("system-remove-text"));
            let remove = format!("{}  {}", icon::TRASH, i18n.tr("system-remove"));
            let buttons = [
                style::DialogButton::primary(remove),
                style::DialogButton::secondary(i18n.tr("cancel")),
            ];
            if let Some(i) = style::dialog_buttons(ui, &buttons) {
                choice = Some(i == 0);
            }
        });
        if let Some(remove) = choice {
            self.confirm_remove = false;
            if remove {
                self.start(ctx, Job::Remove);
            }
        }
    }
}

/// Copies what the system part keeps (hidden devices, virtual device layouts) into the
/// program's own files, then removes it: hiding and the devices go on without it.
fn remove() -> Result<(), String> {
    gob_hide::keep_locally().map_err(|e| format!("hidden devices: {e}"))?;
    gob_virtual::keep_locally().map_err(|e| format!("virtual devices: {e}"))?;
    crate::platform::remove_system_part()
}
