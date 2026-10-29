//! Banner offering device access the user lacks (Linux: controllers and virtual devices need a
//! udev rule, see `platform::grant_device_access`), and keyboard access for the Keyboard tab.
//! Nothing where the platform needs none.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::{self, RichText};
use gob_i18n::I18n;

use crate::style::{self, icon};

pub struct DeviceAccess {
    missing: bool,
    /// The running request; the system is asking the user for the password.
    job: Option<Receiver<Result<(), String>>>,
}

impl DeviceAccess {
    pub fn new() -> Self {
        Self {
            missing: crate::platform::device_access_missing(),
            job: None,
        }
    }

    /// Shows the banner while access is missing; returns the result of a request that finished
    /// in this frame.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> Option<Result<(), String>> {
        let done = self.job.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = &done {
            self.job = None;
            if result.is_ok() {
                self.missing = crate::platform::device_access_missing();
            }
        }
        if !self.missing {
            return done;
        }
        let banner = egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(8)
            .inner_margin(8);
        banner.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::LOCK_KEY).size(style::ICON_SIZE));
                ui.label(i18n.tr("access-banner"));
                let text = format!("{}  {}", icon::SHIELD_CHECK, i18n.tr("access-grant"));
                let button = ui.add_enabled(self.job.is_none(), egui::Button::new(text));
                if button.clicked() {
                    let (tx, rx) = mpsc::channel();
                    let ctx = ui.ctx().clone();
                    let spawned = std::thread::Builder::new()
                        .name("device-access".into())
                        .spawn(move || {
                            let _ = tx.send(crate::platform::grant_device_access());
                            ctx.request_repaint();
                        });
                    match spawned {
                        Ok(_) => self.job = Some(rx),
                        Err(e) => log::error!("device access: {e}"),
                    }
                }
                if self.job.is_some() {
                    ui.spinner();
                    ui.ctx().request_repaint_after(Duration::from_millis(200));
                }
            });
        });
        ui.add_space(4.0);
        done
    }
}

/// Keyboard tab: a banner allowing the program to read the keyboards while it may not, and a
/// button taking back what the user allowed (`platform::set_keyboard_access`).
#[derive(Default)]
pub struct KeyboardAccess {
    /// Checked when first shown and after each request.
    state: Option<crate::platform::KeyboardAccess>,
    /// The running request (allow or take back); the system is asking for the password.
    job: Option<(bool, Receiver<Result<(), String>>)>,
}

impl KeyboardAccess {
    /// Banner while the keyboards cannot be read; returns a request that finished in this frame
    /// (allowed or taken back, result).
    pub fn banner(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> Option<(bool, Result<(), String>)> {
        let done = self.poll();
        if self.state() != crate::platform::KeyboardAccess::Missing {
            return done;
        }
        let banner = egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(8)
            .inner_margin(8);
        banner.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(icon::LOCK_KEY).size(style::ICON_SIZE));
                ui.label(i18n.tr("keyboard-access-banner"));
                let text = format!(
                    "{}  {}",
                    icon::SHIELD_CHECK,
                    i18n.tr("keyboard-access-allow")
                );
                let button = ui.add_enabled(self.job.is_none(), egui::Button::new(text));
                if button.clicked() {
                    self.start(ui.ctx(), true);
                }
                self.spinner(ui);
            });
        });
        ui.add_space(4.0);
        done
    }

    /// Icon button taking the access back, while the user allowed it in the program.
    pub fn remove_button(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        if self.state() != crate::platform::KeyboardAccess::Allowed {
            return;
        }
        let tip = i18n.tr("keyboard-access-remove");
        if style::icon_button(ui, self.job.is_none(), icon::LOCK_KEY_OPEN, &tip).clicked() {
            self.start(ui.ctx(), false);
        }
        self.spinner(ui);
    }

    fn state(&mut self) -> crate::platform::KeyboardAccess {
        *self
            .state
            .get_or_insert_with(crate::platform::keyboard_access)
    }

    fn poll(&mut self) -> Option<(bool, Result<(), String>)> {
        let (allow, rx) = self.job.as_ref()?;
        let result = rx.try_recv().ok()?;
        let allow = *allow;
        self.job = None;
        self.state = None;
        Some((allow, result))
    }

    fn start(&mut self, ctx: &egui::Context, allow: bool) {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("keyboard-access".into())
            .spawn(move || {
                let _ = tx.send(crate::platform::set_keyboard_access(allow));
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.job = Some((allow, rx)),
            Err(e) => log::error!("keyboard access: {e}"),
        }
    }

    fn spinner(&self, ui: &mut egui::Ui) {
        if self.job.is_some() {
            ui.spinner();
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    }
}
