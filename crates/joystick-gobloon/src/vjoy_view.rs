//! vJoy page: driver status, configured devices, their configuration (add / change /
//! remove, like "Configure vJoy") and a manual output tester. Where the backend has virtual
//! gamepads, a row of gamepad tabs under the devices' tabs: add, delete and test them.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText, Vec2};
use gob_i18n::I18n;
use gob_virtual::{
    Backend, Capabilities, ControlKind, DeviceConfig, DeviceInfo, GamepadControl, GamepadInfo,
    Status,
};

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
    /// The backend, or why there is none: (i18n key, details).
    vjoy: Result<Arc<dyn Backend>, (&'static str, String)>,
    devices: Vec<DeviceInfo>,
    refreshed: Option<Instant>,
    selected: u32,
    /// Output tester of the selected device; usable while the lock button holds the device.
    test: Option<Tester>,
    error: Option<String>,
    config: crate::vjoy_config_view::VJoyConfig,
    /// A device added with "+" but not created yet: its own tab, editor (Create) and a
    /// disabled tester. Kept until the driver lists the device, or dropped by Cancel.
    draft: Option<DeviceConfig>,
    gamepads: Vec<GamepadInfo>,
    /// The selected gamepad tab; `None` shows the devices.
    pad: Option<u32>,
    /// Output tester of the selected gamepad, like `test`.
    pad_test: Option<PadTester>,
}

struct PadTester {
    id: u32,
    device: Option<Box<dyn gob_virtual::Gamepad>>,
    /// Stick and trigger values by control.
    axes: Vec<(GamepadControl, f32)>,
    buttons: Vec<(GamepadControl, bool)>,
    dpad: Option<u16>,
}

struct Tester {
    /// The device as the driver reported it; a new layout makes a new tester.
    info: DeviceInfo,
    /// Taken with the lock button; released when the page is left, another device is
    /// selected, the window is minimized or hidden, the configuration changes or a profile starts.
    device: Option<Box<dyn gob_virtual::Device>>,
    axes: [f32; 8],
    buttons: Vec<bool>,
    hats: Vec<Option<u16>>,
}

impl VJoyView {
    pub fn new() -> Self {
        let vjoy = gob_virtual::load().map_err(|e| {
            let key = match e {
                gob_virtual::Error::NoAccess(_) => "vjoy-no-access",
                _ => "vjoy-missing",
            };
            (key, e.to_string())
        });
        let config = crate::vjoy_config_view::VJoyConfig::new(vjoy.as_ref().ok().cloned());
        Self {
            vjoy,
            devices: Vec::new(),
            refreshed: None,
            selected: 1,
            test: None,
            error: None,
            config,
            draft: None,
            gamepads: Vec::new(),
            pad: None,
            pad_test: None,
        }
    }

    /// Lets go of the device (and gamepad) under test; the testers start over from neutral
    /// values.
    pub fn release(&mut self) {
        let held = self.test.take().is_some_and(|t| t.device.is_some());
        let pad_held = self.pad_test.take().is_some_and(|t| t.device.is_some());
        if held || pad_held {
            self.refreshed = None;
        }
    }

    /// `locked`: a profile is active and holds vJoy devices, so they cannot be reconfigured.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, locked: bool) {
        if self.config.poll(i18n).is_some() {
            self.refreshed = None;
            self.test = None;
        }
        if std::mem::take(&mut self.config.cancelled) {
            self.draft = None;
        }
        // A change restarts vJoy; a held device would make Windows ask for a reboot.
        if locked || self.config.busy() {
            self.release();
        }
        let vjoy = match &self.vjoy {
            Ok(v) => v.clone(),
            Err((key, e)) => {
                crate::app::empty_state(ui, icon::WARNING, &i18n.tr(key));
                ui.vertical_centered(|ui| ui.label(RichText::new(e.as_str()).small().weak()));
                return;
            }
        };
        let caps = vjoy.capabilities();
        if self.refreshed.is_none_or(|t| t.elapsed() > REFRESH) {
            self.devices = vjoy.devices();
            self.gamepads = vjoy.gamepads();
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
            ui.label(format!("{} {}", caps.name, vjoy.version()));
            // Only a driver that can be switched off has an on/off state to show.
            if caps.switchable {
                ui.label(RichText::new(i18n.tr(key)).weak());
            }
            if self.config.available() && caps.switchable {
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

        // A created draft becomes a normal tab once the driver lists it.
        if let Some(d) = &self.draft
            && self.devices.iter().any(|x| x.id == d.id)
        {
            self.draft = None;
        }
        let used: Vec<u32> = self.devices.iter().map(|d| d.id).collect();
        let free = self.config.free_id(&used);
        let draft_id = self.draft.as_ref().map(|d| d.id);
        if self.devices.is_empty() && draft_id.is_none() {
            if self.pad.is_none() {
                crate::app::empty_state(ui, icon::GAME_CONTROLLER, &i18n.tr("vjoy-no-devices"));
                if let (true, Some(id)) = (self.config.available(), free) {
                    ui.vertical_centered(|ui| {
                        let text = format!("{}  {}", icon::PLUS, i18n.tr("vjoy-add"));
                        if ui.add_enabled(!locked, egui::Button::new(text)).clicked() {
                            self.add(id);
                        }
                    });
                }
                ui.separator();
            }
            self.gamepads_section(ui, i18n, &vjoy, caps.max_gamepads, locked);
            return;
        }
        if !self.devices.iter().any(|d| d.id == self.selected) && draft_id != Some(self.selected) {
            self.selected = self.devices.first().map_or(self.selected, |d| d.id);
        }
        // Tabs: the devices, then the draft (laid out as its editor says).
        let mut tabs: Vec<(DeviceInfo, &str, String)> = self
            .devices
            .iter()
            .map(|d| {
                (
                    d.clone(),
                    status_glyph(d.status),
                    i18n.tr(status_key(d.status)),
                )
            })
            .collect();
        if let Some(d) = &self.draft {
            let info = draft_info(d, &caps);
            let tip = i18n.trf("vjoy-new-device", &[("n", d.id.into())]);
            tabs.push((info, icon::PENCIL_SIMPLE, tip));
        }
        let mut clicked = None;
        let mut add = false;
        ui.horizontal_wrapped(|ui| {
            for (d, glyph, status) in &tabs {
                let caption = format!(
                    "{}    {} {}   {} {}   {} {}    {glyph}",
                    i18n.trf("vjoy-device-n", &[("n", d.id.into())]),
                    icon::ARROWS_OUT_LINE_HORIZONTAL,
                    d.axes.len(),
                    icon::RADIO_BUTTON,
                    d.buttons,
                    icon::ARROWS_OUT_CARDINAL,
                    d.hats()
                );
                let tip = format!(
                    "{status}\n{}: {}, {}: {}, {}: {}",
                    i18n.tr("vjoy-axes"),
                    d.axes.len(),
                    i18n.tr("vjoy-buttons"),
                    d.buttons,
                    i18n.tr("vjoy-hats"),
                    d.hats()
                );
                let selected = d.id == self.selected && self.pad.is_none();
                let r =
                    style::tab(ui, selected, icon::GAME_CONTROLLER, &caption).on_hover_text(tip);
                if r.clicked() && !selected {
                    clicked = Some(d.id);
                }
            }
            if self.config.available() {
                let can = free.is_some() && self.draft.is_none() && !locked && !self.config.busy();
                let r = style::icon_button(ui, can, icon::PLUS, &i18n.tr("vjoy-add"));
                add = if free.is_none() {
                    r.on_disabled_hover_text(i18n.tr("vjoy-add-max")).clicked()
                } else {
                    r.clicked()
                };
            }
        });
        if let Some(id) = clicked {
            self.select(id);
        }
        if let (true, Some(id)) = (add, free) {
            self.add(id);
        }
        ui.separator();
        if self.gamepads_section(ui, i18n, &vjoy, caps.max_gamepads, locked) {
            return;
        }

        // The selected device, or the draft as it would look.
        let info = match self.devices.iter().find(|d| d.id == self.selected) {
            Some(d) => d.clone(),
            None => match &self.draft {
                Some(d) if d.id == self.selected => draft_info(d, &caps),
                _ => return,
            },
        };
        let new = !self.config.configs.iter().any(|c| c.id == info.id);
        if let Some(d) = &mut self.draft
            && d.id == info.id
        {
            // The editor works on the draft; it survives switching tabs and a refused UAC.
            match &self.config.edit {
                Some(e) if e.id == d.id => *d = e.clone(),
                _ if new && !self.config.busy() => self.config.edit = Some(d.clone()),
                _ => {}
            }
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if self.config.available() {
                    self.config.editor(ui, i18n, info.id, new, locked);
                    ui.separator();
                }
                // The tester shows the editor's layout at once; unapplied changes cannot be
                // tested, so the lock waits for Apply.
                let pending = self.config.edit.as_ref().filter(|e| {
                    e.id == info.id
                        && self.config.configs.iter().find(|c| c.id == info.id) != Some(e)
                });
                let layout = pending.map_or_else(|| info.clone(), |e| preview(&info, e, &caps));
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
        self.refresh_lists();
        self.devices.clone()
    }

    /// Configured gamepads (like [`Self::device_list`]) and how many the backend can have.
    pub fn gamepad_list(&mut self) -> (u32, Vec<GamepadInfo>) {
        self.refresh_lists();
        let max = self
            .vjoy
            .as_ref()
            .map_or(0, |v| v.capabilities().max_gamepads);
        (max, self.gamepads.clone())
    }

    fn refresh_lists(&mut self) {
        if let Ok(vjoy) = &self.vjoy
            && self.refreshed.is_none_or(|t| t.elapsed() > REFRESH * 4)
        {
            self.devices = vjoy.devices();
            self.gamepads = vjoy.gamepads();
            self.refreshed = Some(Instant::now());
        }
    }

    /// The gamepad row (when the backend has gamepads) and the selected gamepad's page; true
    /// when that page is shown instead of a device's.
    fn gamepads_section(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        vjoy: &Arc<dyn Backend>,
        max: u32,
        locked: bool,
    ) -> bool {
        if max == 0 {
            return false;
        }
        self.gamepad_tabs(ui, i18n, max, locked);
        ui.separator();
        let Some(id) = self.pad else {
            return false;
        };
        self.gamepad_page(ui, i18n, vjoy, id, locked);
        true
    }

    /// The gamepad row: a tab per gamepad, then "+".
    fn gamepad_tabs(&mut self, ui: &mut egui::Ui, i18n: &I18n, max: u32, locked: bool) {
        if self
            .pad
            .is_some_and(|id| !self.gamepads.iter().any(|g| g.id == id))
        {
            self.pad = None;
        }
        let free = (1..=max).find(|id| !self.gamepads.iter().any(|g| g.id == *id));
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            for g in &self.gamepads {
                let caption = format!(
                    "{}    {}",
                    i18n.trf("gamepad-n", &[("n", g.id.into())]),
                    status_glyph(g.status)
                );
                let tip = format!(
                    "{}\n{}",
                    i18n.tr(status_key(g.status)),
                    i18n.tr("gamepad-tip")
                );
                let r = style::tab(ui, self.pad == Some(g.id), icon::GAME_CONTROLLER, &caption)
                    .on_hover_text(tip);
                if r.clicked() && self.pad != Some(g.id) {
                    clicked = Some(g.id);
                }
            }
            if self.config.available() {
                let can = free.is_some() && !locked && !self.config.busy();
                let r = style::icon_button(ui, can, icon::PLUS, &i18n.tr("gamepad-add"));
                let r = if free.is_none() {
                    r.on_disabled_hover_text(i18n.trf("gamepad-add-max", &[("n", max.into())]))
                } else {
                    r
                };
                if let (true, Some(id)) = (r.clicked(), free) {
                    self.config.gamepad(true, id, i18n);
                    self.refreshed = None;
                    clicked = Some(id);
                }
            }
        });
        if let Some(id) = clicked {
            self.release();
            self.error = None;
            self.pad = Some(id);
        }
    }

    /// The selected gamepad: delete, the lock button and its output tester.
    fn gamepad_page(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        vjoy: &Arc<dyn Backend>,
        id: u32,
        locked: bool,
    ) {
        if self.pad_test.as_ref().is_none_or(|t| t.id != id) {
            self.release();
            self.pad_test = Some(PadTester::new(id));
        }
        let held = self.pad_test.as_ref().is_some_and(|t| t.device.is_some());
        let can = !locked && !self.config.busy();
        ui.horizontal(|ui| {
            let (glyph, tip) = if held {
                (icon::LOCK_SIMPLE, "vjoy-unlock")
            } else {
                (icon::LOCK_SIMPLE_OPEN, "vjoy-lock")
            };
            let r = ui.add_enabled_ui(can, |ui| style::icon_toggle(ui, held, glyph, &i18n.tr(tip)));
            if r.inner.clicked() {
                if held {
                    self.release();
                } else if let Some(test) = &mut self.pad_test {
                    match vjoy.acquire_gamepad(id) {
                        Ok(device) => {
                            self.error = None;
                            test.device = Some(device);
                        }
                        Err(e) => {
                            log::error!("acquiring virtual gamepad {id}: {e}");
                            self.error =
                                Some(i18n.trf("vjoy-error", &[("error", e.to_string().into())]));
                        }
                    }
                    self.refreshed = None;
                }
            }
            ui.label(RichText::new(i18n.tr("vjoy-test-hint")).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = i18n.trf("gamepad-delete-n", &[("n", id.into())]);
                if style::icon_button(
                    ui,
                    can && !held && self.config.available(),
                    icon::TRASH,
                    &tip,
                )
                .clicked()
                {
                    self.config.gamepad(false, id, i18n);
                    self.refreshed = None;
                    self.pad = None;
                }
            });
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        ui.add_space(6.0);
        let held = self.pad_test.as_ref().is_some_and(|t| t.device.is_some());
        if let Some(test) = &mut self.pad_test {
            ui.add_enabled_ui(can && held, |ui| test.show(ui, i18n));
        }
    }

    /// Switches to another device's tab.
    fn select(&mut self, id: u32) {
        self.pad = None;
        self.selected = id;
        self.release();
        self.error = None;
        self.config.edit = None;
    }

    /// "+": a draft of device `id`, set up like the first device, gets its own tab.
    fn add(&mut self, id: u32) {
        self.draft = Some(self.config.template(id));
        self.select(id);
    }

    /// The lock button: takes the device for the tester.
    fn take(&mut self, vjoy: &Arc<dyn Backend>, info: &DeviceInfo, i18n: &I18n) {
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
                        if (changed || center)
                            && let Some(d) = &self.device
                        {
                            d.set_axis(slot, self.axes[i]);
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

impl PadTester {
    fn new(id: u32) -> Self {
        let of = |kind: ControlKind| {
            GamepadControl::ALL
                .into_iter()
                .filter(move |c| c.kind() == kind)
        };
        Self {
            id,
            device: None,
            axes: of(ControlKind::Stick)
                .chain(of(ControlKind::Trigger))
                .map(|c| (c, 0.0))
                .collect(),
            buttons: of(ControlKind::Button)
                .filter(|c| !c.name().starts_with("dpad"))
                .map(|c| (c, false))
                .collect(),
            dpad: None,
        }
    }

    fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        ui.spacing_mut().slider_width = 320.0;
        egui::Grid::new("pad-axes")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for (control, value) in &mut self.axes {
                    ui.label(i18n.tr(&format!("pad-{}", control.name())));
                    let range = if control.kind() == ControlKind::Trigger {
                        0.0..=1.0
                    } else {
                        -1.0..=1.0
                    };
                    ui.horizontal(|ui| {
                        let slider = egui::Slider::new(value, range).fixed_decimals(2);
                        let changed = ui.add(slider).changed();
                        let rest = style::icon_button(ui, true, icon::CROSSHAIR, "0").clicked();
                        if rest {
                            *value = 0.0;
                        }
                        if (changed || rest)
                            && let Some(d) = &self.device
                        {
                            d.set_axis(*control, *value);
                        }
                    });
                    ui.end_row();
                }
            });
        ui.add_space(8.0);

        // Momentary buttons, as on the device tester.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for (control, state) in &mut self.buttons {
                let r = ui.add(
                    egui::Button::new(i18n.tr(&format!("pad-{}", control.name())))
                        .selected(*state)
                        .min_size(Vec2::new(40.0, 26.0))
                        .corner_radius(style::RADIUS),
                );
                let down = r.is_pointer_button_down_on();
                if down != *state {
                    *state = down;
                    if let Some(d) = &self.device {
                        d.set_button(*control, down);
                    }
                }
            }
        });
        ui.add_space(8.0);

        ui.vertical(|ui| {
            let mut wanted = None;
            egui::Grid::new("pad-dpad")
                .spacing([2.0, 2.0])
                .show(ui, |ui| {
                    for (cell, dir) in HAT_GRID.iter().enumerate() {
                        let glyph = crate::devices_view::hat_glyph(*dir);
                        let r = ui.add_sized(
                            [26.0, 26.0],
                            egui::Button::new(glyph)
                                .selected(self.dpad == *dir && dir.is_some())
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
            if wanted != self.dpad {
                self.dpad = wanted;
                if let Some(d) = &self.device {
                    d.set_dpad(wanted);
                }
            }
            ui.label(RichText::new(i18n.tr("pad-dpad")).weak());
        });
    }
}

/// A draft device as the tabs and the tester show it.
fn draft_info(c: &DeviceConfig, caps: &Capabilities) -> DeviceInfo {
    let base = DeviceInfo {
        id: c.id,
        status: Status::Missing,
        axes: Vec::new(),
        buttons: 0,
        cont_hats: 0,
        disc_hats: 0,
    };
    preview(&base, c, caps)
}

/// The device as it will look once the edited configuration is applied.
fn preview(info: &DeviceInfo, c: &DeviceConfig, caps: &Capabilities) -> DeviceInfo {
    let hats = c.hats.min(caps.max_hats);
    DeviceInfo {
        axes: (1..=8u8).filter(|s| c.axes[usize::from(*s) - 1]).collect(),
        buttons: c.buttons.min(caps.max_buttons),
        cont_hats: if c.discrete_hats { 0 } else { hats },
        disc_hats: if c.discrete_hats { hats } else { 0 },
        ..info.clone()
    }
}

/// Tab mark: free, held by us (the tester's lock or a profile), held by another program.
fn status_glyph(status: Status) -> &'static str {
    match status {
        Status::Own => icon::LOCK_SIMPLE,
        Status::Free => icon::CHECK,
        Status::Busy => icon::PROHIBIT,
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
