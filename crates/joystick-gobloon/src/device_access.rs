//! Banner offering device access the user lacks (Linux: controllers and virtual devices need a
//! udev rule, see `platform::grant_device_access`). Nothing where the platform needs none.

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
