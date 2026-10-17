//! Device hiding window: HidHide's device list with the same options as its own client.

use eframe::egui::{self, RichText};
use gob_hidhide::{DeviceGroup, HidHide};
use gob_i18n::I18n;

use crate::settings::Settings;
use crate::style::{self, icon};

pub struct HidHideView {
    hh: Result<HidHide, String>,
    loaded: bool,
    /// The list was read with this "gaming only" filter.
    gaming_only: bool,
    cloaked: bool,
    groups: Vec<DeviceGroup>,
    hidden: Vec<String>,
    error: Option<String>,
}

impl HidHideView {
    pub fn new() -> Self {
        Self {
            hh: HidHide::find().map_err(|e| e.to_string()),
            loaded: false,
            gaming_only: true,
            cloaked: false,
            groups: Vec::new(),
            hidden: Vec::new(),
            error: None,
        }
    }

    fn refresh(&mut self, gaming_only: bool) {
        self.loaded = true;
        self.gaming_only = gaming_only;
        let Ok(hh) = &self.hh else { return };
        let result = (|| {
            self.cloaked = hh.cloaked()?;
            self.hidden = hh.hidden()?;
            self.groups = hh.device_groups(gaming_only)?;
            Ok::<_, gob_hidhide::Error>(())
        })();
        if let Err(e) = &result {
            log::error!("reading HidHide state: {e}");
        }
        self.error = result.err().map(|e| e.to_string());
    }

    /// Runs a change, then reloads the state from HidHide.
    fn apply(&mut self, change: impl FnOnce(&HidHide) -> Result<(), gob_hidhide::Error>) {
        if let Ok(hh) = &self.hh {
            if let Err(e) = change(hh) {
                log::error!("changing HidHide state: {e}");
                self.error = Some(e.to_string());
                return;
            }
        }
        self.refresh(self.gaming_only);
    }

    /// The connected group of a DirectInput device: same VID/PID, and the same product name
    /// when several devices share them. `None` when not found or still ambiguous.
    fn find_group(&self, info: &gob_input::DeviceInfo) -> Option<usize> {
        let id = Some((info.vendor_id, info.product_id));
        let matches: Vec<usize> = (0..self.groups.len())
            .filter(|&i| {
                let g = &self.groups[i];
                g.present() && g.devices.iter().any(|d| d.present && d.vid_pid() == id)
            })
            .collect();
        if matches.len() <= 1 {
            return matches.first().copied();
        }
        let name = info.name.trim();
        let named: Vec<usize> = matches
            .into_iter()
            .filter(|&i| {
                self.groups[i]
                    .devices
                    .iter()
                    .any(|d| d.product.trim().eq_ignore_ascii_case(name))
            })
            .collect();
        (named.len() == 1).then(|| named[0])
    }

    /// Whether a connected device is hidden; `None` without HidHide or a matching device.
    /// Reads HidHide's state the first time.
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
        Some(self.groups[i].is_hidden(&self.hidden))
    }

    /// Hides or shows a connected device (all its collections and the USB container).
    pub fn set_device_hidden(&mut self, info: &gob_input::DeviceInfo, hide: bool) {
        if let Some(i) = self.find_group(info) {
            let group = self.groups[i].clone();
            self.apply(|hh| hh.set_group_hidden(&group, hide));
        }
    }

    /// Whether device hiding (cloak) is on; `None` without HidHide. Reads the state the first time.
    pub fn cloak(&mut self, gaming_only: bool) -> Option<bool> {
        self.hh.as_ref().ok()?;
        if !self.loaded {
            self.refresh(gaming_only);
        }
        Some(self.cloaked)
    }

    pub fn set_cloak(&mut self, on: bool) {
        self.apply(|hh| hh.set_cloaked(on));
    }

    /// HidHide was changed elsewhere (tray menu): read it again when next needed.
    pub fn reload(&mut self) {
        self.loaded = false;
    }

    /// Shows the window content; returns whether `settings` changed.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, settings: &mut Settings) -> bool {
        let version = match &self.hh {
            Ok(hh) => hh.version.clone(),
            Err(_) => {
                crate::app::empty_state(ui, icon::WARNING, &i18n.tr("hidhide-missing"));
                ui.vertical_centered(|ui| {
                    ui.hyperlink("https://github.com/nefarius/HidHide/releases")
                });
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
                            self.apply(|hh| hh.set_cloaked(cloaked));
                        }
                    });
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("HidHide {version}")).weak());
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
                        .filter(|&i| {
                            !settings.hidhide_hide_disconnected || self.groups[i].present()
                        })
                        .collect();
                    if shown.is_empty() {
                        ui.label(RichText::new(i18n.tr("hidhide-no-devices")).weak());
                    }
                    for i in shown {
                        let group = &self.groups[i];
                        let hidden = group.is_hidden(&self.hidden);
                        ui.horizontal(|ui| {
                            // Closed eye: hidden from other programs; a click switches it.
                            let (glyph, color) = style::eye(ui, hidden);
                            let tip = i18n.tr(if hidden {
                                "hidhide-hidden"
                            } else {
                                "hidhide-visible"
                            });
                            if style::icon_button_colored(ui, true, glyph, color, &tip).clicked() {
                                toggle = Some((i, !hidden));
                            }
                            let name = RichText::new(group.name());
                            ui.label(if group.present() { name } else { name.weak() })
                                .on_hover_text(group.instance_paths().join("\n"));
                            if !group.present() {
                                ui.label(RichText::new(icon::PLUGS).weak())
                                    .on_hover_text(i18n.tr("device-disconnected"));
                            }
                        });
                    }
                });
        });
        if let Some((i, hide)) = toggle {
            let group = self.groups[i].clone();
            self.apply(|hh| hh.set_group_hidden(&group, hide));
        }
        changed
    }
}
