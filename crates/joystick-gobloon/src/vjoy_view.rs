//! vJoy page: driver status, configured devices, their configuration (add / change /
//! remove, like "Configure vJoy") and a manual output tester. Where the backend has virtual
//! gamepads, a row of gamepad tabs under the devices' tabs: add, delete and test them.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText, Vec2};
use gob_i18n::I18n;
use gob_virtual::{
    Backend, Capabilities, Change, ControlKind, DeviceConfig, DeviceInfo, GamepadControl,
    GamepadInfo, Status,
};

use crate::output_marks::OutputMarks;
use crate::style::{self, OutputMark, icon};

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
    /// Legend ticks: used and conflicting outputs are colored.
    shown: [bool; 2],
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
            shown: [true; 2],
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

    /// Collects finished configuration work; `Some(succeeded)` when changes finished. Call it
    /// every frame, also while the page is not shown.
    pub fn poll(&mut self, i18n: &I18n) -> Option<bool> {
        let done = self.config.poll(i18n);
        if done.is_some() {
            self.refreshed = None;
            self.test = None;
        }
        done
    }

    /// Whether configuration changes are running.
    pub fn busy(&self) -> bool {
        self.config.busy()
    }

    /// The backend's capabilities, `None` without virtual devices.
    pub fn capabilities(&self) -> Option<Capabilities> {
        self.vjoy.as_ref().ok().map(|v| v.capabilities())
    }

    /// The stored configuration of the devices (may still be loading).
    pub fn configs(&self) -> &[DeviceConfig] {
        &self.config.configs
    }

    /// The last configuration result for the user (done, or why it failed).
    pub fn config_message(&self) -> Option<&Result<String, String>> {
        self.config.message.as_ref()
    }

    /// Applies `changes` one after another (each may ask for administrator rights).
    pub fn apply(&mut self, changes: Vec<Change>) {
        self.release();
        let labelled = changes.into_iter().map(|c| (format!("{c:?}"), c)).collect();
        self.config.run_all(labelled);
    }

    /// `locked`: a profile is active and holds vJoy devices, so they cannot be reconfigured.
    /// `marks`: outputs the loaded profile uses, colored on the testers.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        locked: bool,
        mut marks: Option<&mut OutputMarks>,
    ) {
        if let Some(m) = marks.as_deref_mut() {
            m.set_shown(self.shown);
        }
        let marks = marks.as_deref();
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
            if self.config.available() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.add_buttons(ui, i18n, caps.max_gamepads, locked);
                });
            }
        });
        self.config.message(ui);
        if !caps.persistent {
            ui.label(RichText::new(i18n.tr("vjoy-not-persistent")).weak());
        }
        ui.separator();

        // A created draft becomes a normal tab once the driver lists it.
        if let Some(d) = &self.draft
            && self.devices.iter().any(|x| x.id == d.id)
        {
            self.draft = None;
        }
        let draft_id = self.draft.as_ref().map(|d| d.id);
        if self
            .pad
            .is_some_and(|id| !self.gamepads.iter().any(|g| g.id == id))
        {
            self.pad = None;
        }
        if self.devices.is_empty() && draft_id.is_none() && self.pad.is_none() {
            if self.gamepads.is_empty() {
                crate::app::empty_state(ui, icon::GAME_CONTROLLER, &i18n.tr("vjoy-no-devices"));
                return;
            }
            self.pad = self.gamepads.first().map(|g| g.id);
        }
        if !self.devices.iter().any(|d| d.id == self.selected) && draft_id != Some(self.selected) {
            self.selected = self.devices.first().map_or(self.selected, |d| d.id);
        }
        self.tabs(ui, i18n, &caps);
        ui.separator();
        if let Some(id) = self.pad {
            self.gamepad_page(ui, i18n, &vjoy, id, locked, marks);
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
                    let source = marks.and_then(|m| m.profile().main_source(info.id));
                    self.config.editor(ui, i18n, info.id, new, locked, source);
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
                legend(ui, i18n, marks, &mut self.shown);
                if let Some(e) = &self.error {
                    ui.colored_label(ui.visuals().warn_fg_color, e);
                }
                ui.add_space(6.0);
                let held = self.test.as_ref().is_some_and(|t| t.device.is_some());
                if let Some(test) = &mut self.test {
                    ui.add_enabled_ui(can && held, |ui| test.show(ui, i18n, &layout, marks));
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

    /// One row of tabs: the virtual sticks, the draft of a new one, then the gamepads.
    fn tabs(&mut self, ui: &mut egui::Ui, i18n: &I18n, caps: &Capabilities) {
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
            let info = draft_info(d, caps);
            let tip = i18n.trf("vjoy-new-device", &[("n", d.id.into())]);
            tabs.push((info, icon::PENCIL_SIMPLE, tip));
        }
        let mut clicked = None;
        let mut clicked_pad = None;
        ui.horizontal_wrapped(|ui| {
            for (d, glyph, status) in &tabs {
                let caption = format!(
                    "{}    {} {}   {} {}   {} {}    {glyph}",
                    device_name(i18n, d),
                    icon::ARROWS_OUT_LINE_HORIZONTAL,
                    d.axes.len(),
                    icon::RADIO_BUTTON,
                    d.buttons,
                    icon::ARROWS_OUT_CARDINAL,
                    d.hats()
                );
                let tip = format!(
                    "{status}
{}: {}, {}: {}, {}: {}",
                    i18n.tr("vjoy-axes"),
                    d.axes.len(),
                    i18n.tr("vjoy-buttons"),
                    d.buttons,
                    i18n.tr("vjoy-hats"),
                    d.hats()
                );
                let selected = d.id == self.selected && self.pad.is_none();
                let r = style::tab(ui, selected, icon::JOYSTICK, &caption).on_hover_text(tip);
                if r.clicked() && !selected {
                    clicked = Some(d.id);
                }
            }
            for g in &self.gamepads {
                let caption = format!(
                    "{}    {}",
                    i18n.trf("gamepad-n", &[("n", g.id.into())]),
                    status_glyph(g.status)
                );
                let tip = format!(
                    "{}
{}",
                    i18n.tr(status_key(g.status)),
                    i18n.tr("gamepad-tip")
                );
                let r = style::tab(ui, self.pad == Some(g.id), icon::GAME_CONTROLLER, &caption)
                    .on_hover_text(tip);
                if r.clicked() && self.pad != Some(g.id) {
                    clicked_pad = Some(g.id);
                }
            }
        });
        if let Some(id) = clicked {
            self.select(id);
        }
        if let Some(id) = clicked_pad {
            self.select_pad(id);
        }
    }

    /// "Add a virtual stick" and, where the backend has gamepads, "Add a virtual gamepad".
    fn add_buttons(&mut self, ui: &mut egui::Ui, i18n: &I18n, max_pads: u32, locked: bool) {
        let idle = !locked && !self.config.busy();
        // Right to left: the gamepad's button first.
        if max_pads > 0 {
            let free = (1..=max_pads).find(|id| !self.gamepads.iter().any(|g| g.id == *id));
            let text = i18n.tr("gamepad-add");
            let r = ui.add_enabled(idle && free.is_some(), egui::Button::new(text));
            let r = if free.is_none() {
                r.on_disabled_hover_text(i18n.trf("gamepad-add-max", &[("n", max_pads.into())]))
            } else {
                r
            };
            if let (true, Some(id)) = (r.clicked(), free) {
                self.config.gamepad(true, id, i18n);
                self.refreshed = None;
                self.select_pad(id);
            }
        }
        let used: Vec<u32> = self.devices.iter().map(|d| d.id).collect();
        let free = self.config.free_id(&used);
        let text = i18n.tr("vjoy-add");
        let can = idle && free.is_some() && self.draft.is_none();
        let r = ui.add_enabled(can, egui::Button::new(text));
        let r = if free.is_none() {
            r.on_disabled_hover_text(i18n.tr("vjoy-add-max"))
        } else {
            r
        };
        if let (true, Some(id)) = (r.clicked(), free) {
            self.add(id);
        }
    }

    fn select_pad(&mut self, id: u32) {
        self.release();
        self.error = None;
        self.pad = Some(id);
    }

    /// The selected gamepad: delete, the lock button and its output tester.
    fn gamepad_page(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        vjoy: &Arc<dyn Backend>,
        id: u32,
        locked: bool,
        marks: Option<&OutputMarks>,
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
        legend(ui, i18n, marks, &mut self.shown);
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        ui.add_space(6.0);
        let held = self.pad_test.as_ref().is_some_and(|t| t.device.is_some());
        if let Some(test) = &mut self.pad_test {
            ui.add_enabled_ui(can && held, |ui| test.show(ui, i18n, marks));
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

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        info: &DeviceInfo,
        marks: Option<&OutputMarks>,
    ) {
        use gob_core::InputType;
        let mark = |ty, n: u32| OutputMarks::vjoy(info.id, ty, n);
        ui.spacing_mut().slider_width = 320.0;
        egui::Grid::new("vjoy-axes")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for &slot in &info.axes {
                    let output = mark(InputType::Axis, slot.into());
                    if !visible(marks, &output) {
                        continue;
                    }
                    let i = usize::from(slot) - 1;
                    let name = crate::devices_view::axis_name(i18n, slot.into());
                    output_label(ui, i18n, marks, &output, &name);
                    ui.horizontal(|ui| {
                        let changed =
                            style::slider(ui, &mut self.axes[i], -1.0..=1.0, 0.01, "").changed();
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
                let n = i as u32 + 1;
                let output = mark(InputType::Button, n);
                if !visible(marks, &output) {
                    continue;
                }
                let (m, count) = marks.map_or((OutputMark::Free, 0), |m| m.mark(&output));
                let size = Vec2::new(40.0, 26.0);
                let r = style::output_button(ui, n.to_string(), size, *state, m, count);
                let r = hover(r, i18n, marks, &output, || {
                    i18n.trf("vjoy-button-n", &[("n", n.into())])
                });
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
                let output = mark(InputType::Hat, h as u32 + 1);
                if !visible(marks, &output) {
                    continue;
                }
                let (m, _) = marks.map_or((OutputMark::Free, 0), |m| m.mark(&output));
                let title = || i18n.trf("vjoy-hat-n", &[("n", (h + 1).into())]);
                ui.vertical(|ui| {
                    // The 3x3 grid's width, so the centered caption stays under it.
                    ui.set_max_width(3.0 * 26.0 + 2.0 * 2.0);
                    let mut wanted = None;
                    egui::Grid::new(("hat", h))
                        .spacing([2.0, 2.0])
                        .show(ui, |ui| {
                            for (cell, dir) in HAT_GRID.iter().enumerate() {
                                let glyph = crate::devices_view::hat_glyph(*dir);
                                let r = style::output_button(
                                    ui,
                                    glyph,
                                    Vec2::splat(26.0),
                                    *state == *dir && dir.is_some(),
                                    m,
                                    0,
                                );
                                let r = hover(r, i18n, marks, &output, title);
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
                    ui.vertical_centered(|ui| {
                        output_label(ui, i18n, marks, &output, &format!("{}", h + 1));
                    });
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

    fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, marks: Option<&OutputMarks>) {
        let id = self.id;
        let mark = |c: GamepadControl| OutputMarks::gamepad(id, c.name());
        let name = |c: GamepadControl| i18n.tr(&format!("pad-{}", c.name()));
        ui.spacing_mut().slider_width = 320.0;
        egui::Grid::new("pad-axes")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for (control, value) in &mut self.axes {
                    let output = mark(*control);
                    if !visible(marks, &output) {
                        continue;
                    }
                    output_label(ui, i18n, marks, &output, &name(*control));
                    let range = if control.kind() == ControlKind::Trigger {
                        0.0..=1.0
                    } else {
                        -1.0..=1.0
                    };
                    ui.horizontal(|ui| {
                        let changed = style::slider(ui, value, range, 0.01, "").changed();
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
                let output = mark(*control);
                if !visible(marks, &output) {
                    continue;
                }
                let (m, count) = marks.map_or((OutputMark::Free, 0), |m| m.mark(&output));
                let label = name(*control);
                let font = egui::TextStyle::Button.resolve(ui.style());
                let width = ui.fonts(|f| {
                    f.layout_no_wrap(label.clone(), font, egui::Color32::WHITE)
                        .size()
                        .x
                });
                let size = Vec2::new((width + 16.0).max(40.0), 26.0);
                let r = style::output_button(ui, label, size, *state, m, count);
                let r = hover(r, i18n, marks, &output, || name(*control));
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
            let output = mark(GamepadControl::Dpad);
            if !visible(marks, &output) {
                return;
            }
            let (m, _) = marks.map_or((OutputMark::Free, 0), |m| m.mark(&output));
            egui::Grid::new("pad-dpad")
                .spacing([2.0, 2.0])
                .show(ui, |ui| {
                    for (cell, dir) in HAT_GRID.iter().enumerate() {
                        let glyph = crate::devices_view::hat_glyph(*dir);
                        let r = style::output_button(
                            ui,
                            glyph,
                            Vec2::splat(26.0),
                            self.dpad == *dir && dir.is_some(),
                            m,
                            0,
                        );
                        let r = hover(r, i18n, marks, &output, || name(GamepadControl::Dpad));
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
            output_label(ui, i18n, marks, &output, &name(GamepadControl::Dpad));
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
        name: None,
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
        name: caps.custom_names.then(|| {
            c.name
                .clone()
                .unwrap_or_else(|| DeviceConfig::default_name(c.id))
        }),
        ..info.clone()
    }
}

/// How the program names a virtual device: its own name where devices have one, else
/// "vJoy #N".
pub fn device_name(i18n: &I18n, d: &DeviceInfo) -> String {
    match &d.name {
        Some(name) => name.clone(),
        None => i18n.trf("vjoy-device-n", &[("n", d.id.into())]),
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

/// Whether the tester shows `output`: the legend ticks hide used or conflicting ones.
fn visible(marks: Option<&OutputMarks>, output: &gob_core::VirtualOutput) -> bool {
    marks.is_none_or(|m| m.visible(output))
}

/// Tooltip of a tester output, shown whether or not the tester is held.
fn hover(
    r: egui::Response,
    i18n: &I18n,
    marks: Option<&OutputMarks>,
    output: &gob_core::VirtualOutput,
    title: impl Fn() -> String,
) -> egui::Response {
    let Some(marks) = marks else {
        return r;
    };
    let tip = |ui: &mut egui::Ui| {
        ui.label(marks.tooltip(i18n, output, &title()));
    };
    r.on_hover_ui(tip).on_disabled_hover_ui(tip)
}

/// Caption of an axis, hat or d-pad in the color of its mark, with the tooltip.
fn output_label(
    ui: &mut egui::Ui,
    i18n: &I18n,
    marks: Option<&OutputMarks>,
    output: &gob_core::VirtualOutput,
    text: &str,
) {
    let (mark, count) = marks.map_or((OutputMark::Free, 0), |m| m.mark(output));
    let caption = if count > 1 {
        format!("{text}  ×{count}")
    } else {
        text.to_string()
    };
    let rich = match style::mark_colors(ui, mark) {
        Some((_, ink)) => RichText::new(caption).color(ink),
        None => RichText::new(caption),
    };
    let r = ui.add(egui::Label::new(rich).sense(egui::Sense::hover()));
    hover(r, i18n, marks, output, || text.to_string());
}

/// What the tester colors mean, and for which profile; a tick per mark turns its color on
/// and off.
fn legend(ui: &mut egui::Ui, i18n: &I18n, marks: Option<&OutputMarks>, shown: &mut [bool; 2]) {
    let Some(marks) = marks else {
        return;
    };
    ui.horizontal(|ui| {
        if let Some(name) = marks.name() {
            ui.label(
                RichText::new(i18n.trf("output-profile", &[("name", name.to_string().into())]))
                    .weak(),
            );
        }
        for (i, (mark, key)) in [
            (OutputMark::Used, "output-legend-used"),
            (OutputMark::Conflict, "output-legend-conflict"),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some((_, ink)) = style::mark_colors(ui, mark) {
                style::tick(ui, &mut shown[i], &i18n.tr(key), ink)
                    .on_hover_text(i18n.tr("output-legend-tip"));
            }
        }
    });
}
