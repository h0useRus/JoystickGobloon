//! vJoy page: driver status, configured devices, their configuration (add / change /
//! remove, like "Configure vJoy") and a manual output tester.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText, Vec2};
use gob_i18n::I18n;
use gob_vjoy::{DeviceInfo, Status, VJoy};

use crate::style::{self, icon};

const AXIS_NAMES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "S1", "S2"];
const REFRESH: Duration = Duration::from_millis(500);
/// Hat directions for the 3x3 tester grid (hundredths of a degree), row by row.
const HAT_GRID: [Option<u16>; 9] = [
    Some(31500),
    Some(0),
    Some(4500),
    Some(27000),
    None,
    Some(9000),
    Some(22500),
    Some(18000),
    Some(13500),
];

pub struct VJoyView {
    vjoy: Result<Arc<VJoy>, String>,
    devices: Vec<DeviceInfo>,
    refreshed: Option<Instant>,
    selected: u32,
    /// Device under manual test, with the values last sent to it.
    test: Option<Tester>,
    error: Option<String>,
    config: crate::vjoy_config_view::VJoyConfig,
    /// Device number being created.
    adding: Option<u32>,
    /// The configuration editor is open instead of the tester.
    configure: bool,
}

struct Tester {
    device: gob_vjoy::Device,
    axes: [f32; 8],
    buttons: Vec<bool>,
    hats: Vec<Option<u16>>,
}

impl VJoyView {
    pub fn new() -> Self {
        let vjoy = VJoy::load().map_err(|e| e.to_string());
        let config = crate::vjoy_config_view::VJoyConfig::new(vjoy.is_ok());
        Self {
            vjoy,
            devices: Vec::new(),
            refreshed: None,
            selected: 1,
            test: None,
            error: None,
            config,
            adding: None,
            // Debug builds: JOYGOB_VJOY=configure opens the editor at start (screenshots).
            configure: cfg!(debug_assertions)
                && std::env::var("JOYGOB_VJOY").as_deref() == Ok("configure"),
        }
    }

    /// `locked`: a profile is active and holds vJoy devices, so they cannot be reconfigured.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, locked: bool) {
        if self.config.poll(i18n) {
            self.refreshed = None;
            self.test = None;
            self.adding = None;
        }
        let vjoy = match &self.vjoy {
            Ok(v) => v.clone(),
            Err(e) => {
                crate::app::empty_state(ui, icon::WARNING, &i18n.tr("vjoy-missing"));
                ui.vertical_centered(|ui| ui.label(RichText::new(e).small().weak()));
                return;
            }
        };
        if self.refreshed.is_none_or(|t| t.elapsed() > REFRESH) {
            self.devices = vjoy.devices();
            self.refreshed = Some(Instant::now());
        }
        ui.ctx().request_repaint_after(REFRESH);

        ui.horizontal(|ui| {
            let (glyph, key) = if vjoy.enabled() {
                (icon::CHECK_CIRCLE, "vjoy-enabled")
            } else {
                (icon::WARNING, "vjoy-disabled")
            };
            ui.label(
                RichText::new(glyph)
                    .size(style::ICON_SIZE)
                    .color(style::accent(ui)),
            );
            ui.label(format!("vJoy {}", vjoy.version()))
                .on_hover_text(vjoy.dll_path.display().to_string());
            ui.label(RichText::new(i18n.tr(key)).weak());
            if self.config.available() {
                let on = vjoy.enabled();
                let tip = i18n.tr(if on { "vjoy-disable" } else { "vjoy-enable" });
                let can = !locked && !self.config.busy();
                if style::icon_button(ui, can, icon::POWER, &tip).clicked() {
                    self.test = None;
                    self.config.enable(!on, i18n);
                }
            }
            if self.config.busy() {
                ui.spinner();
            }
        });
        self.config.message(ui);
        ui.separator();

        let used: Vec<u32> = self.devices.iter().map(|d| d.id).collect();
        let free = self.config.free_id(&used);
        if let Some(id) = self.adding {
            ui.label(
                RichText::new(format!(
                    "{}  {}",
                    icon::PLUS_CIRCLE,
                    i18n.trf("vjoy-new-device", &[("n", id.into())])
                ))
                .strong(),
            );
            ui.add_space(6.0);
            self.config.editor(ui, i18n, id, true, locked);
            if self.config.edit.is_none() && !self.config.busy() {
                self.adding = None;
            }
            return;
        }
        if self.devices.is_empty() {
            crate::app::empty_state(ui, icon::GAME_CONTROLLER, &i18n.tr("vjoy-no-devices"));
            if let (true, Some(id)) = (self.config.available(), free) {
                ui.vertical_centered(|ui| {
                    let text = format!("{}  {}", icon::PLUS, i18n.tr("vjoy-add"));
                    if ui.add_enabled(!locked, egui::Button::new(text)).clicked() {
                        self.adding = Some(id);
                        self.config.start_new(id);
                    }
                });
            }
            return;
        }
        if !self.devices.iter().any(|d| d.id == self.selected) {
            self.selected = self.devices[0].id;
        }
        ui.horizontal_wrapped(|ui| {
            for d in &self.devices {
                let caption = format!("#{}  {}", d.id, status_glyph(d.status));
                let r = style::tab(ui, d.id == self.selected, icon::GAME_CONTROLLER, &caption)
                    .on_hover_text(i18n.tr(status_key(d.status)));
                if r.clicked() && d.id != self.selected {
                    self.selected = d.id;
                    self.test = None;
                    self.error = None;
                    self.config.edit = None;
                }
            }
            if self.config.available() {
                let can = free.is_some() && !locked && !self.config.busy();
                if style::icon_button(ui, can, icon::PLUS, &i18n.tr("vjoy-add")).clicked() {
                    if let Some(id) = free {
                        self.test = None;
                        self.adding = Some(id);
                        self.config.start_new(id);
                    }
                }
            }
        });
        ui.add_space(4.0);

        let Some(info) = self.devices.iter().find(|d| d.id == self.selected).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            chip(
                ui,
                icon::ARROWS_OUT_LINE_HORIZONTAL,
                info.axes.len() as u32,
                "vjoy-axes",
                i18n,
            );
            chip(ui, icon::RADIO_BUTTON, info.buttons, "vjoy-buttons", i18n);
            chip(
                ui,
                icon::ARROWS_OUT_CARDINAL,
                info.hats(),
                "vjoy-hats",
                i18n,
            );
            ui.separator();
            let testing = self.test.is_some();
            let glyph = if testing {
                icon::LOCK_SIMPLE_OPEN
            } else {
                icon::FLASK
            };
            if style::icon_toggle(ui, testing, glyph, &i18n.tr("vjoy-test")).clicked() {
                self.configure = false;
                self.toggle_test(&vjoy, &info, i18n);
            }
            if self.config.available() {
                let tip = i18n.tr("vjoy-configure");
                if style::icon_toggle(ui, self.configure, icon::SLIDERS_HORIZONTAL, &tip).clicked()
                {
                    self.configure = !self.configure;
                    // The tester holds the device; release it before reconfiguring.
                    self.test = None;
                    self.config.edit = None;
                }
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        ui.separator();

        if self.configure {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    self.config.editor(ui, i18n, info.id, false, locked)
                });
        } else if let Some(test) = &mut self.test {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| test.show(ui, &info));
        } else {
            crate::app::empty_state(ui, icon::FLASK, &i18n.tr("vjoy-test-hint"));
        }
    }

    /// Configured devices, refreshed at most every [`REFRESH`].
    pub fn device_list(&mut self) -> Vec<DeviceInfo> {
        if let Ok(vjoy) = &self.vjoy {
            if self.refreshed.is_none_or(|t| t.elapsed() > REFRESH * 4) {
                self.devices = vjoy.devices();
                self.refreshed = Some(Instant::now());
            }
        }
        self.devices.clone()
    }

    fn toggle_test(&mut self, vjoy: &Arc<VJoy>, info: &DeviceInfo, i18n: &I18n) {
        self.error = None;
        if self.test.take().is_some() {
            self.refreshed = None;
            return;
        }
        match vjoy.acquire(info.id) {
            Ok(device) => {
                self.test = Some(Tester {
                    device,
                    axes: [0.0; 8],
                    buttons: vec![false; info.buttons as usize],
                    hats: vec![None; info.hats() as usize],
                });
            }
            Err(e) => {
                log::error!("acquiring vJoy device {}: {e}", info.id);
                self.error = Some(i18n.trf("vjoy-error", &[("error", e.to_string().into())]));
            }
        }
        self.refreshed = None;
    }
}

impl Tester {
    fn show(&mut self, ui: &mut egui::Ui, info: &DeviceInfo) {
        ui.spacing_mut().slider_width = 320.0;
        egui::Grid::new("vjoy-axes")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for &slot in &info.axes {
                    let i = usize::from(slot) - 1;
                    ui.label(RichText::new(AXIS_NAMES[i]).monospace());
                    ui.horizontal(|ui| {
                        let slider =
                            egui::Slider::new(&mut self.axes[i], -1.0..=1.0).fixed_decimals(2);
                        let changed = ui.add(slider).changed();
                        let center = style::icon_button(ui, true, icon::CROSSHAIR, "0").clicked();
                        if center {
                            self.axes[i] = 0.0;
                        }
                        if changed || center {
                            self.device.set_axis(slot, self.axes[i]);
                        }
                    });
                    ui.end_row();
                }
            });
        ui.add_space(8.0);

        // Momentary buttons: pressed while the mouse is held down.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for (i, state) in self.buttons.iter_mut().enumerate() {
                let r = ui.add_sized(
                    [40.0, 26.0],
                    egui::Button::new((i + 1).to_string())
                        .selected(*state)
                        .corner_radius(style::RADIUS),
                );
                let down = r.is_pointer_button_down_on();
                if down != *state {
                    *state = down;
                    self.device.set_button(i as u8 + 1, down);
                }
            }
        });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            for (h, state) in self.hats.iter_mut().enumerate() {
                ui.vertical(|ui| {
                    let mut wanted = None;
                    egui::Grid::new(("hat", h))
                        .spacing([2.0, 2.0])
                        .show(ui, |ui| {
                            for (cell, dir) in HAT_GRID.iter().enumerate() {
                                let glyph = crate::devices_view::hat_glyph(*dir);
                                let r = ui.add_sized(
                                    [26.0, 26.0],
                                    egui::Button::new(glyph)
                                        .selected(*state == *dir && dir.is_some())
                                        .corner_radius(style::RADIUS),
                                );
                                if dir.is_some() && r.is_pointer_button_down_on() {
                                    wanted = *dir;
                                }
                                if cell % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                    if wanted != *state {
                        *state = wanted;
                        self.device.set_hat(h as u8 + 1, wanted);
                    }
                    ui.vertical_centered(|ui| ui.label(RichText::new(format!("{}", h + 1)).weak()));
                });
                ui.add_space(12.0);
            }
        });
    }
}

fn chip(ui: &mut egui::Ui, glyph: &str, count: u32, key: &str, i18n: &I18n) {
    ui.label(RichText::new(format!("{glyph} {count}")).size(15.0))
        .on_hover_text(i18n.tr(key));
}

fn status_glyph(status: Status) -> &'static str {
    match status {
        Status::Own => icon::LOCK_SIMPLE_OPEN,
        Status::Free => icon::CHECK,
        Status::Busy => icon::LOCK_SIMPLE,
        Status::Missing | Status::Unknown => icon::QUESTION,
    }
}

fn status_key(status: Status) -> &'static str {
    match status {
        Status::Own => "vjoy-own",
        Status::Free => "vjoy-free",
        Status::Busy => "vjoy-busy",
        Status::Missing | Status::Unknown => "vjoy-unknown",
    }
}
