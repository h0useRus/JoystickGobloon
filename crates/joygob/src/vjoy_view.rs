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
    /// Output tester of the selected device; usable while the lock button holds the device.
    test: Option<Tester>,
    error: Option<String>,
    config: crate::vjoy_config_view::VJoyConfig,
    /// Device number being created.
    adding: Option<u32>,
}

struct Tester {
    /// The device as the driver reported it; a new layout makes a new tester.
    info: DeviceInfo,
    /// Taken with the lock button; released when the page is left, another device is
    /// selected, the window is minimized or hidden, the configuration changes or a profile starts.
    device: Option<gob_vjoy::Device>,
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
        }
    }

    /// Lets go of the device under test; the tester starts over from neutral values.
    pub fn release(&mut self) {
        if self.test.take().is_some_and(|t| t.device.is_some()) {
            self.refreshed = None;
        }
    }

    /// `locked`: a profile is active and holds vJoy devices, so they cannot be reconfigured.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, locked: bool) {
        if self.config.poll(i18n) {
            self.refreshed = None;
            self.test = None;
            self.adding = None;
        }
        // A change restarts vJoy; a held device would make Windows ask for a reboot.
        if locked || self.config.busy() {
            self.release();
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
                    self.release();
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
                let mut caption = format!(
                    "{}    {} {}   {} {}   {} {}",
                    i18n.trf("vjoy-device-n", &[("n", d.id.into())]),
                    icon::ARROWS_OUT_LINE_HORIZONTAL,
                    d.axes.len(),
                    icon::RADIO_BUTTON,
                    d.buttons,
                    icon::ARROWS_OUT_CARDINAL,
                    d.hats()
                );
                // Our own hold shows at the tester's lock; only someone else's shows here.
                if let Some(glyph) = status_glyph(d.status) {
                    caption = format!("{caption}    {glyph}");
                }
                let tip = format!(
                    "{}\n{}: {}, {}: {}, {}: {}",
                    i18n.tr(status_key(d.status)),
                    i18n.tr("vjoy-axes"),
                    d.axes.len(),
                    i18n.tr("vjoy-buttons"),
                    d.buttons,
                    i18n.tr("vjoy-hats"),
                    d.hats()
                );
                let r = style::tab(ui, d.id == self.selected, icon::GAME_CONTROLLER, &caption)
                    .on_hover_text(tip);
                if r.clicked() && d.id != self.selected {
                    self.selected = d.id;
                    // Like release(); self.devices is borrowed here.
                    self.test = None;
                    self.refreshed = None;
                    self.error = None;
                    self.config.edit = None;
                    self.refreshed = None;
                }
            }
            if self.config.available() {
                let can = free.is_some() && !locked && !self.config.busy();
                if style::icon_button(ui, can, icon::PLUS, &i18n.tr("vjoy-add")).clicked() {
                    if let Some(id) = free {
                        self.release();
                        self.adding = Some(id);
                        self.config.start_new(id);
                    }
                }
            }
        });
        ui.separator();

        let Some(info) = self.devices.iter().find(|d| d.id == self.selected).cloned() else {
            return;
        };
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if self.config.available() {
                    self.config.editor(ui, i18n, info.id, false, locked);
                    ui.separator();
                }
                // The tester shows the editor's layout at once; unapplied changes cannot be
                // tested, so the lock waits for Apply.
                let pending = self.config.edit.as_ref().filter(|e| {
                    e.id == info.id
                        && self.config.configs.iter().find(|c| c.id == info.id) != Some(e)
                });
                let layout = pending.map_or_else(|| info.clone(), |e| preview(&info, e));
                let pending = pending.is_some();
                if self.test.as_ref().is_none_or(|t| {
                    t.info.id != layout.id
                        || (t.info.axes.as_slice(), t.info.buttons, t.info.hats())
                            != (layout.axes.as_slice(), layout.buttons, layout.hats())
                }) {
                    self.release();
                    self.test = Some(Tester::new(&layout));
                }
                if pending {
                    self.release();
                    self.test.get_or_insert_with(|| Tester::new(&layout));
                }
                let can = !locked && !self.config.busy() && !pending;
                let held = self.test.as_ref().is_some_and(|t| t.device.is_some());
                ui.horizontal(|ui| {
                    let (glyph, tip) = if held {
                        (icon::LOCK_SIMPLE, "vjoy-unlock")
                    } else {
                        (icon::LOCK_SIMPLE_OPEN, "vjoy-lock")
                    };
                    let r = ui.add_enabled_ui(can, |ui| {
                        style::icon_toggle(ui, held, glyph, &i18n.tr(tip))
                    });
                    if r.inner.clicked() {
                        if held {
                            self.release();
                        } else {
                            self.take(&vjoy, &info, i18n);
                        }
                    }
                    let hint = if pending {
                        "vjoy-test-pending"
                    } else {
                        "vjoy-test-hint"
                    };
                    ui.label(RichText::new(i18n.tr(hint)).weak());
                });
                if let Some(e) = &self.error {
                    ui.colored_label(ui.visuals().warn_fg_color, e);
                }
                ui.add_space(6.0);
                let held = self.test.as_ref().is_some_and(|t| t.device.is_some());
                if let Some(test) = &mut self.test {
                    ui.add_enabled_ui(can && held, |ui| test.show(ui, &layout));
                }
            });
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

    /// The lock button: takes the device for the tester.
    fn take(&mut self, vjoy: &Arc<VJoy>, info: &DeviceInfo, i18n: &I18n) {
        let Some(test) = &mut self.test else { return };
        match vjoy.acquire(info.id) {
            Ok(device) => {
                self.error = None;
                test.device = Some(device);
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
    fn new(info: &DeviceInfo) -> Self {
        Self {
            info: info.clone(),
            device: None,
            axes: [0.0; 8],
            buttons: vec![false; info.buttons as usize],
            hats: vec![None; info.hats() as usize],
        }
    }

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
                            if let Some(d) = &self.device {
                                d.set_axis(slot, self.axes[i]);
                            }
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
                    if let Some(d) = &self.device {
                        d.set_button(i as u8 + 1, down);
                    }
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
                        if let Some(d) = &self.device {
                            d.set_hat(h as u8 + 1, wanted);
                        }
                    }
                    ui.vertical_centered(|ui| ui.label(RichText::new(format!("{}", h + 1)).weak()));
                });
                ui.add_space(12.0);
            }
        });
    }
}

/// The device as it will look once the edited configuration is applied.
fn preview(info: &DeviceInfo, c: &gob_vjoy::config::DeviceConfig) -> DeviceInfo {
    let hats = c.hats.min(gob_vjoy::config::MAX_HATS);
    DeviceInfo {
        axes: (1..=8u8).filter(|s| c.axes[usize::from(*s) - 1]).collect(),
        buttons: c.buttons.min(gob_vjoy::config::MAX_BUTTONS),
        cont_hats: if c.discrete_hats { 0 } else { hats },
        disc_hats: if c.discrete_hats { hats } else { 0 },
        ..info.clone()
    }
}

/// Tab mark for a device someone else holds or that is not there; none for free or our own.
fn status_glyph(status: Status) -> Option<&'static str> {
    match status {
        Status::Own | Status::Free => None,
        Status::Busy => Some(icon::PROHIBIT),
        Status::Missing | Status::Unknown => Some(icon::QUESTION),
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
