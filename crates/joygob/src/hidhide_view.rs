//! Device hiding page: HidHide state, hidden devices and the application whitelist.

use std::path::PathBuf;

use eframe::egui::{self, RichText};
use gob_hidhide::{same_path, HidDevice, HidHide};
use gob_i18n::I18n;

use crate::style::{self, icon};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Devices,
    Apps,
}

pub struct HidHideView {
    hh: Result<HidHide, String>,
    tab: Tab,
    loaded: bool,
    cloaked: bool,
    devices: Vec<HidDevice>,
    hidden: Vec<String>,
    apps: Vec<PathBuf>,
    error: Option<String>,
}

impl HidHideView {
    pub fn new() -> Self {
        Self {
            hh: HidHide::find().map_err(|e| e.to_string()),
            tab: Tab::Devices,
            loaded: false,
            cloaked: false,
            devices: Vec::new(),
            hidden: Vec::new(),
            apps: Vec::new(),
            error: None,
        }
    }

    /// Adds JoyGob itself to the whitelist so it can read hidden devices.
    pub fn whitelist_self(&self) -> Option<Result<bool, String>> {
        let hh = self.hh.as_ref().ok()?;
        let exe = std::env::current_exe().ok()?;
        Some(hh.ensure_app(&exe).map_err(|e| e.to_string()))
    }

    fn refresh(&mut self) {
        self.loaded = true;
        let Ok(hh) = &self.hh else { return };
        let result = (|| {
            self.cloaked = hh.cloaked()?;
            self.hidden = hh.hidden()?;
            self.devices = hh.gaming_devices()?;
            self.apps = hh.apps()?;
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
        self.refresh();
    }

    fn is_hidden(&self, d: &HidDevice) -> bool {
        self.hidden
            .iter()
            .any(|h| h.eq_ignore_ascii_case(&d.device_instance_path))
    }

    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        let version = match &self.hh {
            Ok(hh) => hh.version.clone(),
            Err(_) => {
                crate::app::empty_state(ui, icon::WARNING, &i18n.tr("hidhide-missing"));
                ui.vertical_centered(|ui| {
                    ui.hyperlink("https://github.com/nefarius/HidHide/releases")
                });
                return;
            }
        };
        if !self.loaded {
            self.refresh();
        }

        ui.horizontal(|ui| {
            let glyph = if self.cloaked {
                icon::EYE_SLASH
            } else {
                icon::EYE
            };
            let key = if self.cloaked {
                "hidhide-on"
            } else {
                "hidhide-off"
            };
            if style::icon_toggle(ui, self.cloaked, glyph, &i18n.tr("hidhide-toggle")).clicked() {
                let on = !self.cloaked;
                self.apply(|hh| hh.set_cloaked(on));
            }
            ui.label(RichText::new(i18n.tr(key)).strong());
            ui.label(RichText::new(format!("HidHide {version}")).weak());
            if style::icon_button(ui, true, icon::ARROWS_CLOCKWISE, &i18n.tr("action-refresh"))
                .clicked()
            {
                self.refresh();
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (tab, glyph, key) in [
                (Tab::Devices, icon::JOYSTICK, "hidhide-devices"),
                (Tab::Apps, icon::APP_WINDOW, "hidhide-apps"),
            ] {
                if style::tab(ui, self.tab == tab, glyph, &i18n.tr(key)).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match self.tab {
                Tab::Devices => self.devices_tab(ui, i18n),
                Tab::Apps => self.apps_tab(ui, i18n),
            });
    }

    fn devices_tab(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        if self.devices.is_empty() {
            ui.label(RichText::new(i18n.tr("no-devices-attached")).weak());
        }
        let mut toggle = None;
        egui::Grid::new("hidhide-devices")
            .num_columns(3)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for (i, d) in self.devices.iter().enumerate() {
                    let hidden = self.is_hidden(d);
                    let (glyph, tip) = if hidden {
                        (icon::EYE_SLASH, "hidhide-hidden")
                    } else {
                        (icon::EYE, "hidhide-visible")
                    };
                    if style::icon_toggle(ui, hidden, glyph, &i18n.tr(tip)).clicked() {
                        toggle = Some((i, !hidden));
                    }
                    let name = RichText::new(d.name());
                    ui.label(if d.present { name } else { name.weak() })
                        .on_hover_text(&d.device_instance_path);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(d.vendor.trim()).weak());
                        if !d.present {
                            ui.label(RichText::new(icon::PLUGS).weak())
                                .on_hover_text(i18n.tr("device-disconnected"));
                        }
                    });
                    ui.end_row();
                }
            });
        if let Some((i, hide)) = toggle {
            let device = self.devices[i].clone();
            self.apply(|hh| hh.set_device_hidden(&device, hide));
        }
    }

    fn apps_tab(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        let own = std::env::current_exe().ok();
        let add = format!("{}  {}", icon::PLUS, i18n.tr("hidhide-add-app"));
        if ui.button(add).clicked() {
            let picked = rfd::FileDialog::new()
                .add_filter("exe", &["exe"])
                .pick_file();
            if let Some(path) = picked {
                self.apply(|hh| hh.set_app(&path, true));
            }
        }
        ui.add_space(6.0);
        let mut remove = None;
        egui::Grid::new("hidhide-apps")
            .num_columns(3)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for app in &self.apps {
                    if style::icon_button(ui, true, icon::TRASH, &i18n.tr("hidhide-remove-app"))
                        .clicked()
                    {
                        remove = Some(app.clone());
                    }
                    let exists = app.exists();
                    let name = app.file_name().map_or_else(
                        || app.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    let text = RichText::new(name);
                    ui.label(if exists { text } else { text.weak() })
                        .on_hover_text(app.display().to_string());
                    ui.horizontal(|ui| {
                        if own.as_deref().is_some_and(|o| same_path(o, app)) {
                            ui.label(
                                RichText::new(format!("{}  JoyGob", icon::STAR))
                                    .color(style::accent(ui)),
                            );
                        }
                        if !exists {
                            ui.label(RichText::new(icon::FILE_X).weak())
                                .on_hover_text(i18n.tr("hidhide-app-missing"));
                        }
                    });
                    ui.end_row();
                }
            });
        if let Some(app) = remove {
            self.apply(|hh| hh.set_app(&app, false));
        }
    }
}
