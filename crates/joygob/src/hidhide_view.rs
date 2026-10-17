//! Device hiding window: HidHide's device list with the same options as its own client.

use eframe::egui::{self, Color32, RichText};
use gob_hidhide::{DeviceGroup, HidHide};
use gob_i18n::I18n;

use crate::settings::Settings;
use crate::style::{self, icon};

const LOCK_RED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);

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
                            .checkbox(
                                &mut settings.hidhide_hide_disconnected,
                                i18n.tr("hidhide-filter-disconnected"),
                            )
                            .changed();
                        changed |= ui
                            .checkbox(
                                &mut settings.hidhide_gaming_only,
                                i18n.tr("hidhide-gaming-only"),
                            )
                            .changed();
                        let mut cloaked = self.cloaked;
                        if ui
                            .checkbox(&mut cloaked, i18n.tr("hidhide-enable"))
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
                            let mut checked = hidden;
                            if ui.checkbox(&mut checked, "").changed() {
                                toggle = Some((i, checked));
                            }
                            let (glyph, color) = if hidden {
                                (icon::LOCK, LOCK_RED)
                            } else {
                                (icon::LOCK_OPEN, ui.visuals().weak_text_color())
                            };
                            let tip = if hidden {
                                "hidhide-hidden"
                            } else {
                                "hidhide-visible"
                            };
                            ui.label(RichText::new(glyph).size(style::ICON_SIZE).color(color))
                                .on_hover_text(i18n.tr(tip));
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
