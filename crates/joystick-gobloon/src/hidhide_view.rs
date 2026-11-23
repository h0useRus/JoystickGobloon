//! Device hiding window: the devices the system can hide, with the options of HidHide's own
//! client.
//!
//! Reading the devices (SetupAPI, tens of milliseconds) and changing them run on a thread of
//! their own; the window shows the last state read until the next one arrives.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use eframe::egui::{self, RichText};
use gob_hide::{Backend, Device};
use gob_i18n::I18n;

use crate::settings::Settings;
use crate::style::{self, icon};

pub struct HidHideView {
    hh: Result<Arc<dyn Backend>, String>,
    /// Asks the worker for reads and changes; `None` without device hiding.
    worker: Option<Sender<Work>>,
    replies: Option<Receiver<Reply>>,
    /// A first state arrived.
    have_state: bool,
    ctx: egui::Context,
    /// A read is asked for (or arrived) since the last change elsewhere.
    loaded: bool,
    /// The list was read with this "gaming only" filter.
    gaming_only: bool,
    cloaked: bool,
    groups: Vec<Device>,
    error: Option<String>,
    /// Connected input devices when the list was read: a device plugged in later is not in it.
    inputs: Vec<String>,
}

/// A change to run, then a read.
type Change = Box<dyn FnOnce(&dyn Backend) -> Result<(), gob_hide::Error> + Send>;

enum Work {
    /// Read the state with this "gaming only" filter.
    Read(bool),
    Change(Change, bool),
}

enum Reply {
    State(Result<(bool, Vec<Device>), String>),
    ChangeFailed(String),
}

/// The worker: changes and reads in the order they were asked for.
fn work(hh: &dyn Backend, ctx: &egui::Context, jobs: &Receiver<Work>, replies: &Sender<Reply>) {
    for job in jobs {
        let gaming_only = match job {
            Work::Read(g) => g,
            Work::Change(change, g) => {
                if let Err(e) = change(hh) {
                    log::error!("changing the device hiding state: {e}");
                    let _ = replies.send(Reply::ChangeFailed(e.to_string()));
                    ctx.request_repaint();
                    continue;
                }
                g
            }
        };
        let state = (|| Ok::<_, gob_hide::Error>((hh.enabled()?, hh.devices(gaming_only)?)))()
            .map_err(|e| {
                log::error!("reading the device hiding state: {e}");
                e.to_string()
            });
        if replies.send(Reply::State(state)).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

impl HidHideView {
    pub fn new(ctx: egui::Context) -> Self {
        let hh = gob_hide::load().map_err(|e| e.to_string());
        let (worker, replies) = match &hh {
            Ok(backend) => {
                let (tx, jobs) = std::sync::mpsc::channel();
                let (reply, replies) = std::sync::mpsc::channel();
                let (backend, ctx) = (backend.clone(), ctx.clone());
                let spawned = std::thread::Builder::new()
                    .name("device-hiding".into())
                    .spawn(move || work(backend.as_ref(), &ctx, &jobs, &reply));
                match spawned {
                    Ok(_) => (Some(tx), Some(replies)),
                    Err(e) => {
                        log::error!("device hiding thread: {e}");
                        (None, None)
                    }
                }
            }
            Err(_) => (None, None),
        };
        Self {
            hh,
            worker,
            replies,
            have_state: false,
            ctx,
            loaded: false,
            gaming_only: true,
            cloaked: false,
            groups: Vec::new(),
            error: None,
            inputs: Vec::new(),
        }
    }

    /// Made again (device access changed): a new backend, the same window.
    pub fn renew(&mut self) {
        *self = Self::new(self.ctx.clone());
    }

    /// Asks for the state; it arrives in a later frame.
    fn refresh(&mut self, gaming_only: bool) {
        self.loaded = true;
        self.gaming_only = gaming_only;
        if let Some(worker) = &self.worker {
            let _ = worker.send(Work::Read(gaming_only));
        }
    }

    /// Runs a change, then reads the state again (both on the worker).
    fn apply(
        &mut self,
        change: impl FnOnce(&dyn Backend) -> Result<(), gob_hide::Error> + Send + 'static,
    ) {
        self.loaded = true;
        if let Some(worker) = &self.worker {
            let _ = worker.send(Work::Change(Box::new(change), self.gaming_only));
        }
    }

    /// Takes what the worker finished.
    fn poll(&mut self) {
        let Some(replies) = &self.replies else {
            return;
        };
        for reply in replies.try_iter() {
            match reply {
                Reply::State(Ok((cloaked, groups))) => {
                    self.cloaked = cloaked;
                    self.groups = groups;
                    self.have_state = true;
                    self.error = None;
                }
                Reply::State(Err(e)) => {
                    // Shown as off, as before the read moved to the worker.
                    self.have_state = true;
                    self.error = Some(e);
                }
                Reply::ChangeFailed(e) => self.error = Some(e),
            }
        }
    }

    /// The hideable device an input device belongs to (see [`gob_hide::find_device`]).
    fn find_group(&self, info: &gob_input::DeviceInfo) -> Option<usize> {
        gob_hide::find_device(&self.groups, (info.vendor_id, info.product_id), &info.name)
    }

    /// Whether a connected device is hidden; `None` without device hiding or a matching device.
    /// Reads the state the first time.
    pub fn device_hidden(
        &mut self,
        info: &gob_input::DeviceInfo,
        gaming_only: bool,
    ) -> Option<bool> {
        self.hh.as_ref().ok()?;
        self.poll();
        if !self.loaded {
            self.refresh(gaming_only);
        }
        let i = self.find_group(info)?;
        Some(self.groups[i].hidden)
    }

    /// Whether a connected device is hidden but other programs can still read it (see
    /// `gob_hide::Device::exposed`); uses the state read by [`Self::device_hidden`].
    pub fn device_exposed(&self, info: &gob_input::DeviceInfo) -> bool {
        self.find_group(info)
            .is_some_and(|i| self.groups[i].exposed)
    }

    /// Hides or shows a connected device (all its collections and the USB container).
    pub fn set_device_hidden(&mut self, info: &gob_input::DeviceInfo, hide: bool) {
        if let Some(i) = self.find_group(info) {
            let group = self.groups[i].clone();
            self.apply(move |hh| hh.set_hidden(&group, hide));
        }
    }

    /// Whether device hiding is on; `None` without it. Reads the state the first time.
    pub fn cloak(&mut self, gaming_only: bool) -> Option<bool> {
        self.hh.as_ref().ok()?;
        self.poll();
        if !self.loaded {
            self.refresh(gaming_only);
        }
        self.have_state.then_some(self.cloaked)
    }

    pub fn set_cloak(&mut self, on: bool) {
        self.apply(move |hh| hh.set_enabled(on));
    }

    /// The connected input devices (ids); when they change the list is read again when next
    /// needed, so newly plugged devices get their state.
    pub fn track_inputs(&mut self, ids: Vec<String>) {
        if ids != self.inputs {
            self.inputs = ids;
            self.loaded = false;
        }
        self.poll_changed();
    }

    /// Reads the state again when next needed if the backend changed it in the background.
    fn poll_changed(&mut self) {
        if self.hh.as_ref().is_ok_and(|hh| hh.take_changed()) {
            self.loaded = false;
        }
    }

    /// Device hiding was changed elsewhere (tray menu): read it again when next needed.
    pub fn reload(&mut self) {
        self.loaded = false;
    }

    /// Shows the window content; returns whether `settings` changed.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, settings: &mut Settings) -> bool {
        let (caps, version) = match &self.hh {
            Ok(hh) => (hh.capabilities(), hh.version()),
            Err(_) => {
                crate::app::empty_state(ui, icon::WARNING, &i18n.tr("hidhide-missing"));
                if let Some(url) = gob_hide::download_url() {
                    ui.vertical_centered(|ui| ui.hyperlink(url));
                }
                return false;
            }
        };
        self.poll_changed();
        self.poll();
        if !self.loaded || self.gaming_only != settings.hidhide_gaming_only {
            self.refresh(settings.hidhide_gaming_only);
        }

        let mut changed = false;
        egui::Panel::bottom("hidhide-options")
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        changed |= ui
                            .add(style::switch(
                                &mut settings.hidhide_hide_disconnected,
                                i18n.tr("hidhide-filter-disconnected"),
                            ))
                            .changed();
                        changed |= ui
                            .add(style::switch(
                                &mut settings.hidhide_gaming_only,
                                i18n.tr("hidhide-gaming-only"),
                            ))
                            .changed();
                        let mut cloaked = self.cloaked;
                        if ui
                            .add(style::switch(&mut cloaked, i18n.tr("hidhide-enable")))
                            .changed()
                        {
                            self.apply(move |hh| hh.set_enabled(cloaked));
                        }
                    });
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if !caps.name.is_empty() {
                                ui.label(RichText::new(format!("{} {version}", caps.name)).weak());
                            }
                            if style::icon_button(
                                ui,
                                true,
                                icon::ARROWS_CLOCKWISE,
                                &i18n.tr("action-refresh"),
                            )
                            .clicked()
                            {
                                self.refresh(settings.hidhide_gaming_only);
                            }
                        });
                        ui.label(RichText::new(i18n.tr("hidhide-hint")).weak());
                        if !caps.hides_everywhere {
                            ui.label(RichText::new(i18n.tr("hidhide-while-running")).weak());
                        }
                    });
                });
            });

        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        let mut toggle = None;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let shown: Vec<usize> = (0..self.groups.len())
                        .filter(|&i| !settings.hidhide_hide_disconnected || self.groups[i].present)
                        .collect();
                    if shown.is_empty() {
                        ui.label(RichText::new(i18n.tr("hidhide-no-devices")).weak());
                    }
                    for i in shown {
                        let group = &self.groups[i];
                        let hidden = group.hidden;
                        ui.horizontal(|ui| {
                            // Closed eye: hidden from other programs; a click switches it.
                            let (glyph, color) = style::eye(ui, hidden, self.cloaked);
                            let tip = eye_tip(i18n, hidden, self.cloaked);
                            if style::icon_button_colored(ui, true, glyph, color, &tip).clicked() {
                                toggle = Some((i, !hidden));
                            }
                            let name = RichText::new(&group.name);
                            ui.label(if group.present { name } else { name.weak() })
                                .on_hover_text(&group.detail);
                            if !group.present {
                                ui.label(RichText::new(icon::PLUGS).weak())
                                    .on_hover_text(i18n.tr("device-disconnected"));
                            }
                            if group.exposed && self.cloaked {
                                let warn = ui.visuals().warn_fg_color;
                                ui.label(RichText::new(icon::WARNING).color(warn))
                                    .on_hover_text(i18n.tr("hidhide-exposed"));
                            }
                        });
                    }
                });
        });
        if let Some((i, hide)) = toggle {
            let group = self.groups[i].clone();
            self.apply(move |hh| hh.set_hidden(&group, hide));
        }
        changed
    }
}

/// Tooltip of a device eye; while hiding is off it says so, the eye shows the saved state.
pub fn eye_tip(i18n: &I18n, hidden: bool, hiding_on: bool) -> String {
    let state = i18n.tr(if hidden {
        "hidhide-hidden"
    } else {
        "hidhide-visible"
    });
    if hiding_on {
        state
    } else {
        format!(
            "{state}
{}",
            i18n.tr("hidhide-off")
        )
    }
}
