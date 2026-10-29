//! Adding, changing and removing virtual devices (vJoy: like "Configure vJoy", without
//! vJoyConfig.exe). Reading and applying the configuration may block (vJoy: the registry, an
//! elevated copy of this exe behind a UAC prompt), so both run on worker threads.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, RichText};
use gob_i18n::I18n;
use gob_virtual::{Backend, Capabilities, Change, DeviceConfig, Outcome};

use crate::style::{self, icon};

/// What finished in the background.
enum Done {
    Read(Result<Vec<DeviceConfig>, String>),
    /// A change: its log label and result.
    Change(String, Result<Outcome, String>),
}

pub struct VJoyConfig {
    /// Where the configuration lives; `None` without virtual devices.
    backend: Option<Arc<dyn Backend>>,
    /// Configuration of the existing devices (from the registry).
    pub configs: Vec<DeviceConfig>,
    /// Edited copy of the selected device (or of a new one).
    pub edit: Option<DeviceConfig>,
    job: Option<Receiver<Done>>,
    /// Changes waiting for the running one, see [`Self::run_all`].
    queue: VecDeque<(String, Change)>,
    pub message: Option<Result<String, String>>,
    confirm_remove: Option<u32>,
    /// Cancel was pressed on a new device's editor (the page drops its draft).
    pub cancelled: bool,
}

impl VJoyConfig {
    pub fn new(backend: Option<Arc<dyn Backend>>) -> Self {
        let mut s = Self {
            backend,
            configs: Vec::new(),
            edit: None,
            job: None,
            queue: VecDeque::new(),
            message: None,
            confirm_remove: None,
            cancelled: false,
        };
        s.reload();
        s
    }

    pub fn available(&self) -> bool {
        self.backend.is_some()
    }

    fn capabilities(&self) -> Option<Capabilities> {
        self.backend.as_ref().map(|b| b.capabilities())
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn reload(&mut self) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(Done::Read(backend.read_config()));
        });
        self.job = Some(rx);
    }

    fn run(&mut self, label: String, change: Change) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        log::info!("virtual devices: {label} ({change:?})");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let result = backend.apply(&change);
            let _ = tx.send(Done::Change(label, result));
        });
        self.job = Some(rx);
        self.message = None;
    }

    /// Runs `changes` one after another (labels for the log); a failed one drops the rest.
    pub fn run_all(&mut self, changes: Vec<(String, Change)>) {
        self.queue = changes.into();
        if let Some((label, change)) = self.queue.pop_front() {
            self.run(label, change);
        }
    }

    /// Collects finished background work; `Some(succeeded)` when a change finished.
    pub fn poll(&mut self, i18n: &I18n) -> Option<bool> {
        let Some(rx) = &self.job else { return None };
        let Ok(done) = rx.try_recv() else {
            return None;
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
                None
            }
            Done::Read(Err(e)) => {
                log::error!("reading the virtual device configuration: {e}");
                None
            }
            Done::Change(label, result) => {
                let ok = result.is_ok();
                match result {
                    Ok(Outcome::Done) => {
                        log::info!("virtual devices: {label} done");
                        self.message = Some(Ok(i18n.tr("vjoy-config-done")));
                    }
                    Ok(Outcome::RebootNeeded) => {
                        log::warn!("virtual devices: {label} done, the system wants a restart");
                        self.message = Some(Err(i18n.tr("vjoy-config-reboot")));
                    }
                    Err(e) => {
                        log::error!("virtual devices: {label} failed: {e}");
                        self.message =
                            Some(Err(i18n.trf("vjoy-config-error", &[("error", e.into())])));
                    }
                }
                self.edit = None;
                match self.queue.pop_front() {
                    Some((label, change)) if ok => {
                        self.run(label, change);
                        return None;
                    }
                    _ => self.queue.clear(),
                }
                self.reload();
                Some(ok)
            }
        }
    }

    /// Lowest device number not in use.
    pub fn free_id(&self, used: &[u32]) -> Option<u32> {
        let max = self.capabilities().map_or(0, |c| c.max_devices);
        (1..=max).find(|n| !used.contains(n) && !self.configs.iter().any(|c| c.id == *n))
    }

    /// A new device `id` set up like the first existing one, or with everything at maximum.
    pub fn template(&self, id: u32) -> DeviceConfig {
        let caps = self.capabilities().unwrap_or(gob_virtual::NO_CAPABILITIES);
        let mut c = self
            .configs
            .iter()
            .min_by_key(|c| c.id)
            .cloned()
            .unwrap_or_else(|| DeviceConfig::new(id, &caps));
        c.id = id;
        c
    }

    /// Adds or deletes virtual gamepad `id`.
    pub fn gamepad(&mut self, add: bool, id: u32, i18n: &I18n) {
        let (key, change) = if add {
            ("gamepad-add-n", Change::AddGamepad(id))
        } else {
            ("gamepad-delete-n", Change::DeleteGamepad(id))
        };
        self.run(i18n.trf(key, &[("n", id.into())]), change);
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
        let caps = self.capabilities().unwrap_or(gob_virtual::NO_CAPABILITIES);
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
                ui.add(style::Spin::new(&mut edit.buttons).range(0..=caps.max_buttons));
                ui.end_row();

                ui.label(i18n.tr("vjoy-hats"));
                ui.horizontal(|ui| {
                    ui.add(style::Spin::new(&mut edit.hats).range(0..=caps.max_hats));
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
                        edit.effects = vec![any; caps.effects.len()];
                    }
                    if any {
                        ui.horizontal_wrapped(|ui| {
                            for (i, on) in edit.effects.iter_mut().enumerate() {
                                let key = format!("vjoy-ffb-{}", caps.effects[i].to_lowercase());
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
            let admin = self.capabilities().is_some_and(|c| c.needs_admin);
            let glyph = if admin {
                icon::SHIELD_CHECK
            } else {
                icon::CHECK
            };
            let mut apply = ui.add_enabled(
                can && changed,
                egui::Button::new(format!("{glyph}  {}", i18n.tr(caption))),
            );
            if admin {
                apply = apply.on_hover_text(i18n.tr("vjoy-config-admin"));
            }
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
            if new
                && ui
                    .add_enabled(can, egui::Button::new(i18n.tr("cancel")))
                    .clicked()
            {
                self.edit = None;
                self.cancelled = true;
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
        style::modal(ctx, "vjoy-remove").show(ctx, |ui| {
            ui.set_width(420.0);
            let title = i18n.trf("vjoy-remove-title", &[("n", id.into())]);
            style::dialog_title(ui, Some((icon::TRASH, None)), &title);
            ui.label(i18n.tr("vjoy-remove-text"));
            let remove = format!("{}  {}", icon::TRASH, i18n.tr("vjoy-config-remove"));
            let buttons = [
                style::DialogButton::primary(remove),
                style::DialogButton::secondary(i18n.tr("cancel")),
            ];
            if let Some(i) = style::dialog_buttons(ui, &buttons) {
                choice = Some(i == 0);
            }
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
