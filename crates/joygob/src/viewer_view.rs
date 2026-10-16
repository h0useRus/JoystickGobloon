//! Input viewer (like Gremlin's): per device an axis history graph, the current axis
//! values and the buttons and hats, for physical and vJoy devices alike.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use gob_i18n::I18n;
use gob_input::DeviceState;

use crate::devices_view::hat_glyph;
use crate::style::{self, icon};

/// Seconds shown by the history graph.
const HISTORY: f64 = 20.0;
const SAMPLE_EVERY: Duration = Duration::from_millis(30);
const REFRESH: Duration = Duration::from_millis(33);

/// One colour per axis slot (X, Y, Z, Rx, Ry, Rz, Slider 1, Slider 2).
const AXIS_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x3b, 0x8e, 0xde),
    Color32::from_rgb(0xe5, 0x48, 0x4d),
    Color32::from_rgb(0x46, 0xa7, 0x58),
    Color32::from_rgb(0xf7, 0x8d, 0x1e),
    Color32::from_rgb(0x8b, 0x8d, 0x98),
    Color32::from_rgb(0xe6, 0xc2, 0x29),
    Color32::from_rgb(0x12, 0xb8, 0xc6),
    Color32::from_rgb(0x8e, 0x4e, 0xc6),
];

/// What is shown for a device.
#[derive(Clone, Copy)]
struct Shown {
    history: bool,
    current: bool,
    buttons: bool,
}

impl Default for Shown {
    fn default() -> Self {
        Self {
            history: true,
            current: true,
            buttons: true,
        }
    }
}

#[derive(Default)]
pub struct ViewerView {
    shown: HashMap<String, Shown>,
    /// Per device: (seconds since `start`, axis values by slot).
    history: HashMap<String, VecDeque<(f64, [f32; 8])>>,
    start: Option<Instant>,
    last_sample: Option<Instant>,
}

impl ViewerView {
    /// `detached`: shown in its own window. Returns true when the user switches between
    /// the page and the separate window.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        devices: &[DeviceState],
        detached: bool,
    ) -> bool {
        self.sample(devices);
        ui.ctx().request_repaint_after(REFRESH);
        let (glyph, key) = if detached {
            (icon::ARROW_SQUARE_IN, "viewer-attach")
        } else {
            (icon::ARROW_SQUARE_OUT, "viewer-detach")
        };
        let mut toggle = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.tr("nav-viewer")).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                toggle = style::icon_button(ui, true, glyph, &i18n.tr(key)).clicked();
            });
        });
        ui.separator();
        if devices.is_empty() {
            crate::app::empty_state(ui, icon::JOYSTICK, &i18n.tr("no-devices-attached"));
            return toggle;
        }

        egui::SidePanel::left("viewer-devices")
            .resizable(true)
            .default_width(230.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for d in devices {
                        let shown = self.shown.entry(d.info.guid.clone()).or_default();
                        let glyph = if d.info.is_vjoy() {
                            icon::GAME_CONTROLLER
                        } else {
                            icon::JOYSTICK
                        };
                        egui::CollapsingHeader::new(format!("{glyph}  {}", d.info.name.trim()))
                            .id_salt(&d.info.guid)
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.checkbox(&mut shown.history, i18n.tr("viewer-history"));
                                ui.checkbox(&mut shown.current, i18n.tr("viewer-current"));
                                ui.checkbox(&mut shown.buttons, i18n.tr("viewer-buttons"));
                            });
                    }
                });
            });

        let now = self.now();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for d in devices {
                    let shown = self.shown.get(&d.info.guid).copied().unwrap_or_default();
                    let name = d.info.name.trim();
                    if shown.history && !d.info.axes.is_empty() {
                        section(ui, &format!("{name} · {}", i18n.tr("viewer-history")));
                        let samples = self.history.get(&d.info.guid);
                        history_graph(ui, i18n, d, samples, now);
                    }
                    if shown.current && !d.info.axes.is_empty() {
                        section(ui, &format!("{name} · {}", i18n.tr("viewer-current")));
                        current_axes(ui, i18n, d);
                    }
                    if shown.buttons && (d.info.buttons > 0 || d.info.hats > 0) {
                        section(ui, &format!("{name} · {}", i18n.tr("viewer-buttons")));
                        buttons_and_hats(ui, d);
                    }
                }
            });
        toggle
    }

    fn now(&mut self) -> f64 {
        self.start
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_secs_f64()
    }

    /// Records axis values about every 30 ms and drops what is older than the graph shows.
    fn sample(&mut self, devices: &[DeviceState]) {
        if self.last_sample.is_some_and(|t| t.elapsed() < SAMPLE_EVERY) {
            return;
        }
        self.last_sample = Some(Instant::now());
        let now = self.now();
        for d in devices {
            let h = self.history.entry(d.info.guid.clone()).or_default();
            h.push_back((now, d.axes));
            while h.front().is_some_and(|(t, _)| now - t > HISTORY + 1.0) {
                h.pop_front();
            }
        }
        self.history
            .retain(|guid, _| devices.iter().any(|d| &d.info.guid == guid));
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong());
        let rect = ui.available_rect_before_wrap();
        let y = rect.center().y;
        ui.painter().line_segment(
            [Pos2::new(rect.left() + 6.0, y), Pos2::new(rect.right(), y)],
            ui.visuals().widgets.noninteractive.bg_stroke,
        );
    });
    ui.add_space(4.0);
}

fn history_graph(
    ui: &mut egui::Ui,
    i18n: &I18n,
    d: &DeviceState,
    samples: Option<&VecDeque<(f64, [f32; 8])>>,
    now: f64,
) {
    // Legend.
    ui.horizontal_wrapped(|ui| {
        for &slot in &d.info.axes {
            let color = AXIS_COLORS[usize::from(slot - 1) % 8];
            ui.label(RichText::new("■").color(color));
            ui.label(RichText::new(i18n.trf("viewer-axis", &[("n", slot.into())])).small());
            ui.add_space(6.0);
        }
    });
    let width = ui.available_width().max(200.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 200.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let text = visuals.weak_text_color();
    let grid = visuals.widgets.noninteractive.bg_stroke.color;
    let font = FontId::monospace(10.0);
    let plot = Rect::from_min_max(
        Pos2::new(rect.left() + 40.0, rect.top() + 6.0),
        Pos2::new(rect.right() - 8.0, rect.bottom() - 18.0),
    );
    let x_of = |t: f64| plot.left() + ((t - (now - HISTORY)) / HISTORY) as f32 * plot.width();
    let y_of = |v: f32| plot.center().y - v.clamp(-1.0, 1.0) * plot.height() / 2.0;
    for v in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
        let y = y_of(v);
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0, grid),
        );
        painter.text(
            Pos2::new(plot.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            format!("{v:.2}"),
            font.clone(),
            text,
        );
    }
    for s in [0, 5, 10, 15, 20] {
        let x = plot.left() + (1.0 - s as f32 / HISTORY as f32) * plot.width();
        painter.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0, grid),
        );
        painter.text(
            Pos2::new(x, plot.bottom() + 3.0),
            Align2::CENTER_TOP,
            format!("-{s} s"),
            font.clone(),
            text,
        );
    }
    let Some(samples) = samples else { return };
    for &slot in &d.info.axes {
        let i = usize::from(slot - 1);
        let points: Vec<Pos2> = samples
            .iter()
            .filter(|(t, _)| now - t <= HISTORY)
            .map(|(t, axes)| Pos2::new(x_of(*t), y_of(axes[i])))
            .collect();
        if points.len() > 1 {
            painter.add(egui::Shape::line(
                points,
                Stroke::new(1.8, AXIS_COLORS[i % 8]),
            ));
        }
    }
}

fn current_axes(ui: &mut egui::Ui, i18n: &I18n, d: &DeviceState) {
    ui.horizontal(|ui| {
        for &slot in &d.info.axes {
            let value = d.axes[usize::from(slot - 1)];
            ui.vertical(|ui| {
                ui.set_width(64.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(i18n.trf("viewer-axis", &[("n", slot.into())])).small());
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 100.0), Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, 4, ui.visuals().extreme_bg_color);
                    let fill = rect.height() * (value.clamp(-1.0, 1.0) + 1.0) / 2.0;
                    let bar =
                        Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - fill), rect.max);
                    painter.rect_filled(bar, 4, AXIS_COLORS[usize::from(slot - 1) % 8]);
                    ui.label(
                        RichText::new(format!("{:.0} %", value * 100.0))
                            .monospace()
                            .small(),
                    );
                });
            });
        }
    });
}

fn buttons_and_hats(ui: &mut egui::Ui, d: &DeviceState) {
    let accent = style::accent(ui);
    ui.horizontal_top(|ui| {
        // 16 per row, like Gremlin.
        ui.vertical(|ui| {
            for row in d.buttons.chunks(16).enumerate() {
                ui.horizontal(|ui| {
                    for (i, &pressed) in row.1.iter().enumerate() {
                        let n = row.0 * 16 + i + 1;
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                        let painter = ui.painter();
                        let (fill, color) = if pressed {
                            (accent, ui.visuals().extreme_bg_color)
                        } else {
                            (ui.visuals().faint_bg_color, ui.visuals().text_color())
                        };
                        painter.rect_filled(rect, style::RADIUS, fill);
                        painter.text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            n.to_string(),
                            FontId::proportional(12.0),
                            color,
                        );
                    }
                });
            }
        });
        ui.add_space(16.0);
        for (n, &direction) in d.hats.iter().enumerate() {
            hat_rose(ui, n + 1, direction, accent);
        }
    });
}

/// Eight direction marks around the hat number; the active one is highlighted.
fn hat_rose(ui: &mut egui::Ui, n: usize, direction: Option<u16>, accent: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(80.0), Sense::hover());
    let painter = ui.painter();
    let idle = ui.visuals().weak_text_color();
    let c = rect.center();
    for i in 0..8u16 {
        let deg = i * 4500;
        let a = f32::from(deg) / 100.0 * std::f32::consts::PI / 180.0;
        let pos = c + Vec2::new(a.sin(), -a.cos()) * 28.0;
        let on = direction == Some(deg);
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            hat_glyph(Some(deg)),
            FontId::proportional(if on { 18.0 } else { 14.0 }),
            if on { accent } else { idle },
        );
    }
    let center_color = if direction.is_none() { idle } else { accent };
    painter.text(
        c,
        Align2::CENTER_CENTER,
        n.to_string(),
        FontId::proportional(14.0),
        center_color,
    );
}
