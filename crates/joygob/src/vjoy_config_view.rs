//! Adding, changing and removing vJoy devices like "Configure vJoy", without vJoyConfig.exe:
//! the configuration is read from the registry in the background; changes run this exe
//! elevated (UAC, `--vjoy-config`) on a worker thread.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use eframe::egui::{self, RichText};
use gob_i18n::I18n;
use gob_vjoy::config::{DeviceConfig, EFFECTS, MAX_BUTTONS, MAX_DEVICES, MAX_HATS};
use gob_vjoy::configure::{self, Change, Outcome};

use crate::style::{self, icon};

/// What finished in the background.
enum Done {
    Read(Result<Vec<DeviceConfig>, String>),
    /// A change: its log label and result.
    Change(String, Result<Outcome, String>),
}

pub struct VJoyConfig {
    /// This exe, started elevated for changes; `None` without vJoy.
    exe: Option<PathBuf>,
    /// Configuration of the existing devices (from the registry).
    pub configs: Vec<DeviceConfig>,
    /// Edited copy of the selected device (or of a new one).
    pub edit: Option<DeviceConfig>,
    job: Option<Receiver<Done>>,
    pub message: Option<Result<String, String>>,
    confirm_remove: Option<u32>,
}

impl VJoyConfig {
    /// `installed`: the vJoy driver is there.
    pub fn new(installed: bool) -> Self {
        let mut s = Self {
            exe: std::env::current_exe().ok().filter(|_| installed),
            configs: Vec::new(),
            edit: None,
            job: None,
            message: None,
            confirm_remove: None,
        };
        s.reload();
        s
    }

    pub fn available(&self) -> bool {
        self.exe.is_some()
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn reload(&mut self) {
        if self.exe.is_none() {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(Done::Read(gob_vjoy::descriptor::read_registry()));
        });
        self.job = Some(rx);
    }

    fn run(&mut self, label: String, change: Change) {
        let Some(exe) = self.exe.clone() else {
            return;
        };
        log::info!("vJoy: {label} ({})", change.args().join(" "));
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let result = configure::run_elevated(&exe, &change);
            let _ = tx.send(Done::Change(label, result));
        });
        self.job = Some(rx);
        self.message = None;
    }

    /// Collects finished background work; returns true when the device list changed.
    pub fn poll(&mut self, i18n: &I18n) -> bool {
        let Some(rx) = &self.job else { return false };
        let Ok(done) = rx.try_recv() else {
            return false;
        };
        self.job = None;
        match done {
            Done::Read(Ok(configs)) => {
                // An untouched copy of the old configuration (taken while this read ran,
                // e.g. right after Apply) follows the new one.
                if self.edit.as_ref().is_some_and(|e| self.configs.contains(e)) {
                    self.edit = None;
                }
                self.configs = configs;
                false
            }
            Done::Read(Err(e)) => {
                log::error!("reading vJoy configuration: {e}");
                false
            }
            Done::Change(label, result) => {
                match result {
                    Ok(Outcome::Done) => {
                        log::info!("vJoy: {label} done");
                        self.message = Some(Ok(i18n.tr("vjoy-config-done")));
                    }
                    Ok(Outcome::RebootNeeded) => {
                        log::warn!("vJoy: {label} done, Windows wants a restart");
                        self.message = Some(Err(i18n.tr("vjoy-config-reboot")));
                    }
                    Err(e) => {
                        log::error!("vJoy: {label} failed: {e}");
                        self.message =
                            Some(Err(i18n.trf("vjoy-config-error", &[("error", e.into())])));
                    }
                }
                self.edit = None;
                self.reload();
                true
            }
        }
    }

    /// Lowest device number not in use.
    pub fn free_id(&self, used: &[u32]) -> Option<u32> {
        (1..=MAX_DEVICES).find(|n| !used.contains(n) && !self.configs.iter().any(|c| c.id == *n))
    }

    pub fn start_new(&mut self, id: u32) {
        self.edit = Some(DeviceConfig::new(id));
        self.message = None;
    }

    pub fn enable(&mut self, on: bool, i18n: &I18n) {
        let label = i18n.tr(if on { "vjoy-enable" } else { "vjoy-disable" });
        self.run(label, Change::Enable(on));
    }

    /// Editor for device `id` (`new`: not created yet). `locked`: a profile holds vJoy.
    pub fn editor(&mut self, ui: &mut egui::Ui, i18n: &I18n, id: u32, new: bool, locked: bool) {
        if self.edit.as_ref().is_none_or(|e| e.id != id) {
            self.edit = self.configs.iter().find(|c| c.id == id).cloned();
        }
        let Some(edit) = &mut self.edit else {
            ui.label(RichText::new(i18n.tr("vjoy-config-reading")).weak());
            return;
        };
        let original = self.configs.iter().find(|c| c.id == id);

        egui::Grid::new(("vjoy-config", id))
            .num_columns(2)
            .spacing([14.0, 8.0])
            .show(ui, |ui| {
                ui.label(i18n.tr("vjoy-axes"));
                ui.horizontal_wrapped(|ui| {
                    for (i, on) in edit.axes.iter_mut().enumerate() {
                        ui.add(style::switch(on, i18n.tr(&format!("vjoy-axis-{}", i + 1))));
                    }
                });
                ui.end_row();

                ui.label(i18n.tr("vjoy-buttons"));
                style::stepper(ui, &mut edit.buttons, 0..=MAX_BUTTONS);
                ui.end_row();

                ui.label(i18n.tr("vjoy-hats"));
                ui.horizontal(|ui| {
                    style::stepper(ui, &mut edit.hats, 0..=MAX_HATS);
                    ui.add_enabled_ui(edit.hats > 0, |ui| {
                        ui.radio_value(
                            &mut edit.discrete_hats,
                            false,
                            i18n.tr("vjoy-hat-continuous"),
                        );
                        ui.radio_value(&mut edit.discrete_hats, true, i18n.tr("vjoy-hat-discrete"));
                    });
                });
                ui.end_row();

                ui.label(i18n.tr("vjoy-ffb"));
                ui.vertical(|ui| {
                    let mut any = edit.effects.iter().any(|e| *e);
                    if ui
                        .add(style::switch(&mut any, i18n.tr("vjoy-ffb-enable")))
                        .changed()
                    {
                        edit.effects = [any; 11];
                    }
                    if any {
                        ui.horizontal_wrapped(|ui| {
                            for (i, on) in edit.effects.iter_mut().enumerate() {
                                let key = format!("vjoy-ffb-{}", EFFECTS[i].to_lowercase());
                                ui.add(style::switch(on, i18n.tr(&key)));
                            }
                        });
                    }
                });
                ui.end_row();
            });

        ui.add_space(8.0);
        let changed = new || original != Some(&*edit);
        let can = !locked && self.job.is_none();
        let edit = edit.clone();
        ui.horizontal(|ui| {
            let caption = if new {
                "vjoy-config-create"
            } else {
                "vjoy-config-apply"
            };
            let apply = ui
                .add_enabled(
                    can && changed,
                    egui::Button::new(format!("{}  {}", icon::SHIELD_CHECK, i18n.tr(caption))),
                )
                .on_hover_text(i18n.tr("vjoy-config-admin"));
            if apply.clicked() {
                let label = i18n.trf("vjoy-config-label", &[("n", id.into())]);
                self.run(label, Change::Set(edit));
            }
            if !new
                && changed
                && ui
                    .add_enabled(can, egui::Button::new(i18n.tr("vjoy-config-revert")))
                    .clicked()
            {
                self.edit = None;
            }
            if new && ui.button(i18n.tr("cancel")).clicked() {
                self.edit = None;
            }
            if !new {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            can,
                            egui::Button::new(format!(
                                "{}  {}",
                                icon::TRASH,
                                i18n.tr("vjoy-config-remove")
                            )),
                        )
                        .clicked()
                    {
                        self.confirm_remove = Some(id);
                    }
                });
            }
            if self.job.is_some() {
                ui.spinner();
            }
        });
        if locked {
            ui.label(RichText::new(i18n.tr("vjoy-config-locked")).weak());
        }
        self.remove_dialog(ui.ctx(), i18n);
    }

    fn remove_dialog(&mut self, ctx: &egui::Context, i18n: &I18n) {
        let Some(id) = self.confirm_remove else {
            return;
        };
        let mut choice = None;
        egui::Modal::new(egui::Id::new("vjoy-remove")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading(i18n.trf("vjoy-remove-title", &[("n", id.into())]));
            ui.label(i18n.tr("vjoy-remove-text"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .button(format!(
                        "{}  {}",
                        icon::TRASH,
                        i18n.tr("vjoy-config-remove")
                    ))
                    .clicked()
                {
                    choice = Some(true);
                }
                if ui.button(i18n.tr("cancel")).clicked() {
                    choice = Some(false);
                }
            });
        });
        if let Some(remove) = choice {
            self.confirm_remove = None;
            if remove {
                let label = i18n.trf("vjoy-remove-title", &[("n", id.into())]);
                self.run(label, Change::Delete(id));
            }
        }
    }

    /// Success or error of the last change.
    pub fn message(&self, ui: &mut egui::Ui) {
        match &self.message {
            Some(Ok(text)) => {
                ui.label(
                    RichText::new(format!("{}  {text}", icon::CHECK_CIRCLE))
                        .color(style::accent(ui)),
                );
            }
            Some(Err(text)) => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!("{}  {text}", icon::WARNING),
                );
            }
            None => {}
        }
    }
}
