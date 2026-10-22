//! Device hiding window: the devices the system can hide, with the options of HidHide's own
//! client.

use std::sync::Arc;

use eframe::egui::{self, RichText};
use gob_hide::{Backend, Device};
use gob_i18n::I18n;

use crate::settings::Settings;
use crate::style::{self, icon};

pub struct HidHideView {
    hh: Result<Arc<dyn Backend>, String>,
    loaded: bool,
    /// The list was read with this "gaming only" filter.
    gaming_only: bool,
    cloaked: bool,
    groups: Vec<Device>,
    error: Option<String>,
    /// Connected input devices when the list was read: a device plugged in later is not in it.
    inputs: Vec<String>,
}

impl HidHideView {
    pub fn new() -> Self {
        Self {
            hh: gob_hide::load().map_err(|e| e.to_string()),
            loaded: false,
            gaming_only: true,
            cloaked: false,
            groups: Vec::new(),
            error: None,
            inputs: Vec::new(),
        }
    }

    fn refresh(&mut self, gaming_only: bool) {
        self.loaded = true;
        self.gaming_only = gaming_only;
        let Ok(hh) = &self.hh else { return };
        let result = (|| {
            self.cloaked = hh.enabled()?;
            self.groups = hh.devices(gaming_only)?;
            Ok::<_, gob_hide::Error>(())
        })();
        if let Err(e) = &result {
            log::error!("reading the device hiding state: {e}");
        }
        self.error = result.err().map(|e| e.to_string());
    }

    /// Runs a change, then reloads the state.
    fn apply(&mut self, change: impl FnOnce(&dyn Backend) -> Result<(), gob_hide::Error>) {
        if let Ok(hh) = &self.hh
            && let Err(e) = change(hh.as_ref())
        {
            log::error!("changing the device hiding state: {e}");
            self.error = Some(e.to_string());
            return;
        }
        self.refresh(self.gaming_only);
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
        if !self.loaded {
            self.refresh(gaming_only);
        }
        let i = self.find_group(info)?;
        Some(self.groups[i].hidden)
    }

    /// Hides or shows a connected device (all its collections and the USB container).
    pub fn set_device_hidden(&mut self, info: &gob_input::DeviceInfo, hide: bool) {
        if let Some(i) = self.find_group(info) {
            let group = self.groups[i].clone();
            self.apply(|hh| hh.set_hidden(&group, hide));
        }
    }

    /// Whether device hiding is on; `None` without it. Reads the state the first time.
    pub fn cloak(&mut self, gaming_only: bool) -> Option<bool> {
        self.hh.as_ref().ok()?;
        if !self.loaded {
            self.refresh(gaming_only);
        }
        Some(self.cloaked)
    }

    pub fn set_cloak(&mut self, on: bool) {
        self.apply(|hh| hh.set_enabled(on));
    }

    /// The connected input devices (ids); when they change the list is read again when next
    /// needed, so newly plugged devices get their state.
    pub fn track_inputs(&mut self, ids: Vec<String>) {
        if ids != self.inputs {
            self.inputs = ids;
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
        if !self.loaded || self.gaming_only != settings.hidhide_gaming_only {
            self.refresh(settings.hidhide_gaming_only);
        }

        let mut changed = false;
        egui::TopBottomPanel::bottom("hidhide-options")
            .show_separator_line(false)
            .show_inside(ui, |ui| {
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
                            self.apply(|hh| hh.set_enabled(cloaked));
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
                        });
                    }
                });
        });
        if let Some((i, hide)) = toggle {
            let group = self.groups[i].clone();
            self.apply(|hh| hh.set_hidden(&group, hide));
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
