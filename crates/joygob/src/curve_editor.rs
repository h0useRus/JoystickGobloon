//! Response curve editor: curve type, invert, symmetric editing, the chart with draggable
//! knots (and Bezier handles), X/Y of the selected point, and the deadzone as one
//! four-handle slider aligned under the chart's x axis.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use gob_core::curve::{Curve, CurveType, Editable};
use gob_core::Action;

use crate::action_editor::{py_float, Editor, Op};
use crate::style::{self, icon};

/// Space for the X/Y fields next to the chart.
const SIDE: f32 = 190.0;

impl Editor<'_> {
    pub(crate) fn curve(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        curve_type: &str,
        deadzone: [f64; 4],
        points: &[(f64, f64)],
    ) {
        let id = action.id.clone();
        let kind = CurveType::parse(curve_type);
        let symmetric = action
            .str("symmetric")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
        let sel_id = ui.make_persistent_id(("curve-selected", &id));
        let mut selected: usize = ui
            .data(|d| d.get_temp(sel_id))
            .filter(|&i: &usize| i < points.len())
            .unwrap_or(0);
        let size = (ui.available_width() - SIDE).clamp(220.0, 460.0);

        // Type, invert, symmetric.
        ui.horizontal(|ui| {
            ui.label(self.i18n.tr("curve-type"));
            egui::ComboBox::from_id_salt(("curve-type", &id))
                .selected_text(self.i18n.tr(type_key(kind)))
                .show_ui(ui, |ui| {
                    for k in CurveType::ALL {
                        if ui
                            .selectable_label(k == kind, self.i18n.tr(type_key(k)))
                            .clicked()
                            && k != kind
                        {
                            // Like Gremlin: a new curve of that type (the identity line).
                            self.set(&id, "curve-type", "string", k.name());
                            let mut p = k.default_points();
                            if symmetric {
                                p = Editable::symmetric(k, &p);
                            }
                            self.set_points(&id, &p);
                            ui.data_mut(|d| d.insert_temp(sel_id, 0usize));
                        }
                    }
                });
            let invert = format!("{}  {}", icon::FLIP_VERTICAL, self.i18n.tr("edit-invert"));
            if ui.button(invert).clicked() {
                let inverted: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (x, -y)).collect();
                self.set_points(&id, &inverted);
            }
            let mut sym = symmetric;
            if ui
                .checkbox(&mut sym, self.i18n.tr("curve-symmetric"))
                .on_hover_text(self.i18n.tr("curve-symmetric-tip"))
                .changed()
            {
                self.set(&id, "symmetric", "bool", if sym { "True" } else { "False" });
                if sym {
                    self.set_points(&id, &Editable::symmetric(kind, points));
                    let [low, cl, ch, high] = deadzone;
                    let (outer, center) = ((high - low) / 2.0, (ch - cl) / 2.0);
                    self.set_deadzone(&id, deadzone, [-outer, -center, center, outer]);
                }
            }
        });

        ui.horizontal_top(|ui| {
            let help = self.i18n.tr("edit-curve-help");
            let (new, sel) = chart(
                ui, &id, kind, points, deadzone, size, selected, symmetric, &help,
            );
            if sel != selected {
                selected = sel;
                ui.data_mut(|d| d.insert_temp(sel_id, sel));
            }
            if let Some(new) = new {
                self.set_points(&id, &new);
            }
            // X / Y of the selected point.
            ui.vertical(|ui| {
                ui.add_space(size / 2.0 - 30.0);
                let Some(&(px, py)) = points.get(selected) else {
                    return;
                };
                let (mut x, mut y) = (px, py);
                let knots = Editable::knots(kind, points.len());
                let is_end = selected == 0 || selected == points.len() - 1;
                let locked = symmetric && Editable::mirror(points.len(), selected) == selected;
                let mut changed = false;
                egui::Grid::new(("curve-point", &id))
                    .num_columns(2)
                    .show(ui, |ui| {
                        let what = if knots.contains(&selected) {
                            "curve-point"
                        } else {
                            "curve-handle"
                        };
                        ui.label(RichText::new(self.i18n.tr(what)).weak());
                        ui.end_row();
                        ui.label("X");
                        let drag = egui::DragValue::new(&mut x)
                            .range(-1.0..=1.0)
                            .speed(0.005)
                            .fixed_decimals(4);
                        changed |= ui.add_enabled(!is_end && !locked, drag).changed();
                        ui.end_row();
                        ui.label("Y");
                        let drag = egui::DragValue::new(&mut y)
                            .range(-1.0..=1.0)
                            .speed(0.005)
                            .fixed_decimals(4);
                        changed |= ui.add_enabled(!locked, drag).changed();
                        ui.end_row();
                    });
                if changed {
                    self.set_points(&id, &Editable::set(kind, points, selected, x, y, symmetric));
                }
            });
        });

        // Deadzone: a slider aligned with the chart's x axis, its fields under the handles.
        let mut dz = deadzone;
        ui.add_space(2.0);
        let (rect, slider) = deadzone_slider(ui, size, &mut dz, symmetric);
        if slider {
            self.set_deadzone(&id, deadzone, dz);
        }
        let fields = deadzone_fields(ui, rect, &mut dz, symmetric);
        if fields {
            self.set_deadzone(&id, deadzone, dz);
        }
        ui.label(RichText::new(self.i18n.tr("edit-deadzone")).weak().small());
    }

    fn set_points(&mut self, id: &str, points: &[(f64, f64)]) {
        self.ops.push(Op::SetPoints {
            action: id.to_string(),
            points: points.iter().map(|&(x, y)| (x as f32, y as f32)).collect(),
        });
    }

    fn set_deadzone(&mut self, id: &str, old: [f64; 4], new: [f64; 4]) {
        for (i, name) in ["low", "center-low", "center-high", "high"]
            .into_iter()
            .enumerate()
        {
            if (old[i] - new[i]).abs() > 1e-9 {
                self.ops.push(Op::SetInGroup {
                    action: id.to_string(),
                    group: "deadzone".into(),
                    name: name.into(),
                    ty: "float".into(),
                    value: py_float(new[i] as f32),
                });
            }
        }
    }
}

fn type_key(k: CurveType) -> &'static str {
    match k {
        CurveType::PiecewiseLinear => "curve-linear",
        CurveType::CubicSpline => "curve-cubic",
        CurveType::CubicBezier => "curve-bezier",
    }
}

/// Screen x of curve value `v` in a chart/slider of this rect (same mapping for both).
fn x_of(rect: Rect, v: f64) -> f32 {
    rect.center().x + v as f32 * rect.width() / 2.0
}

/// The chart. Drag a point or handle; double-click empty space adds a knot, right-click
/// removes one, a single click on empty space inverts the curve.
#[allow(clippy::too_many_arguments)]
fn chart(
    ui: &mut egui::Ui,
    id: &str,
    kind: CurveType,
    points: &[(f64, f64)],
    deadzone: [f64; 4],
    size: f32,
    selected: usize,
    symmetric: bool,
    help: &str,
) -> (Option<Vec<(f64, f64)>>, usize) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let response = response.on_hover_text(help);
    let half = size / 2.0;
    let to_screen =
        |(x, y): (f64, f64)| Pos2::new(x_of(rect, x), rect.center().y - y as f32 * half);
    let to_curve = |p: Pos2| {
        (
            f64::from((p.x - rect.center().x) / half).clamp(-1.0, 1.0),
            f64::from((rect.center().y - p.y) / half).clamp(-1.0, 1.0),
        )
    };
    let accent = style::accent(ui);
    let weak = ui.visuals().weak_text_color();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, style::RADIUS, ui.visuals().extreme_bg_color);

    // Deadzones: input ranges that give no change, shaded with their edges marked.
    let [low, center_low, center_high, high] = deadzone;
    let warn = ui.visuals().warn_fg_color;
    for (a, b) in [(-1.0, low), (center_low, center_high), (high, 1.0)] {
        if b > a {
            let r = Rect::from_x_y_ranges(x_of(rect, a)..=x_of(rect, b), rect.y_range());
            painter.rect_filled(r, 0.0, warn.gamma_multiply(0.10));
        }
    }
    for v in deadzone {
        let x = x_of(rect, v);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, warn.gamma_multiply(0.45)),
        );
    }
    for i in 1..8 {
        let t = -1.0 + f64::from(i) * 0.25;
        let strength = if i == 4 { 0.45 } else { 0.15 };
        let stroke = Stroke::new(1.0, weak.gamma_multiply(strength));
        painter.line_segment([to_screen((t, -1.0)), to_screen((t, 1.0))], stroke);
        painter.line_segment([to_screen((-1.0, t)), to_screen((1.0, t))], stroke);
    }

    // The curve itself, sampled.
    let curve = Curve::new(kind, points);
    let line: Vec<Pos2> = (0..=200)
        .map(|i| {
            let x = -1.0 + f64::from(i) / 100.0;
            to_screen((x, curve.eval(x)))
        })
        .collect();
    painter.add(egui::Shape::line(line, Stroke::new(2.0, accent)));

    let knots = Editable::knots(kind, points.len());
    let screen: Vec<Pos2> = points.iter().map(|&p| to_screen(p)).collect();
    if kind == CurveType::CubicBezier {
        for (i, p) in screen.iter().enumerate() {
            if !knots.contains(&i) {
                let knot = if knots.contains(&(i + 1)) {
                    i + 1
                } else {
                    i - 1
                };
                painter.line_segment([screen[knot], *p], Stroke::new(1.0, weak));
            }
        }
    }
    for (i, p) in screen.iter().enumerate() {
        let fill = if i == selected {
            accent
        } else {
            ui.visuals().panel_fill
        };
        if knots.contains(&i) {
            painter.circle(*p, 6.0, fill, Stroke::new(2.0, accent));
        } else {
            painter.rect(
                Rect::from_center_size(*p, Vec2::splat(8.0)),
                1.0,
                fill,
                Stroke::new(1.5, weak),
                egui::StrokeKind::Middle,
            );
        }
    }
    painter.text(
        rect.left_top() + Vec2::splat(4.0),
        Align2::LEFT_TOP,
        "1",
        FontId::monospace(10.0),
        weak,
    );

    let nearest = |pos: Pos2| {
        screen
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.distance(pos)))
            .filter(|(_, d)| *d < 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let drag_id = ui.make_persistent_id(("curve-drag", id));
    let mut selected = selected;
    if response.drag_started() {
        if let Some(i) = response.interact_pointer_pos().and_then(nearest) {
            ui.data_mut(|d| d.insert_temp(drag_id, i));
            selected = i;
        }
    }
    if response.dragged() {
        let dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
        if let (Some(i), Some(pos)) = (dragging, response.interact_pointer_pos()) {
            let (x, y) = to_curve(pos);
            return (
                Some(Editable::set(kind, points, i, x, y, symmetric)),
                selected,
            );
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<usize>(drag_id));
    }
    let on_point = response.interact_pointer_pos().and_then(nearest);
    if response.clicked() {
        if let Some(i) = on_point {
            selected = i;
        }
    }
    // A single click on empty space inverts, but only once it is clear it was not the
    // first half of a double-click (which adds a point instead).
    const DOUBLE_CLICK: f64 = 0.3;
    let pending_id = ui.make_persistent_id(("curve-invert", id));
    let now = ui.input(|i| i.time);
    if response.double_clicked() {
        ui.data_mut(|d| d.remove::<f64>(pending_id));
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| on_point.is_none())
        {
            let (x, y) = to_curve(pos);
            if x > -1.0 && x < 1.0 {
                let (p, at) = Editable::add(kind, points, x, y, symmetric);
                return (Some(p), at);
            }
        }
    } else if response.clicked() && on_point.is_none() {
        ui.data_mut(|d| d.insert_temp(pending_id, now));
    }
    if let Some(clicked_at) = ui.data(|d| d.get_temp::<f64>(pending_id)) {
        if now - clicked_at >= DOUBLE_CLICK {
            ui.data_mut(|d| d.remove::<f64>(pending_id));
            return (
                Some(points.iter().map(|&(x, y)| (x, -y)).collect()),
                selected,
            );
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(DOUBLE_CLICK));
    }
    if response.secondary_clicked() {
        if let Some(i) = on_point {
            let p = Editable::remove(kind, points, i, symmetric);
            if p.len() != points.len() {
                return (Some(p), 0);
            }
        }
    }
    (None, selected)
}

/// One slider over -1..1 with the four deadzone handles, as wide as the chart and right
/// under it, so every handle sits below its deadzone edge. Symmetric: handles mirror.
fn deadzone_slider(
    ui: &mut egui::Ui,
    width: f32,
    dz: &mut [f64; 4],
    symmetric: bool,
) -> (Rect, bool) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click_and_drag());
    let to_v = |x: f32| f64::from((x - rect.center().x) / (rect.width() / 2.0)).clamp(-1.0, 1.0);
    let handle_id = response.id.with("handle");
    let mut changed = false;
    if response.drag_started() {
        if let Some(p) = response.interact_pointer_pos() {
            let nearest = (0..4)
                .min_by(|&a, &b| {
                    let da = (x_of(rect, dz[a]) - p.x).abs();
                    let db = (x_of(rect, dz[b]) - p.x).abs();
                    da.total_cmp(&db)
                })
                .unwrap_or(0);
            ui.data_mut(|d| d.insert_temp(handle_id, nearest));
        }
    }
    if response.dragged() {
        let handle: Option<usize> = ui.data(|d| d.get_temp(handle_id));
        if let (Some(h), Some(p)) = (handle, response.interact_pointer_pos()) {
            let v = (to_v(p.x) * 10_000.0).round() / 10_000.0;
            set_handle(dz, h, v, symmetric);
            changed = true;
        }
    }
    let accent = style::accent(ui);
    let painter = ui.painter();
    let y = rect.center().y;
    painter.line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        Stroke::new(4.0, ui.visuals().widgets.inactive.bg_fill),
    );
    // Live ranges (outside the deadzones) in the accent colour.
    for (a, b) in [(dz[0], dz[1]), (dz[2], dz[3])] {
        painter.line_segment(
            [Pos2::new(x_of(rect, a), y), Pos2::new(x_of(rect, b), y)],
            Stroke::new(4.0, accent),
        );
    }
    for v in *dz {
        painter.circle(
            Pos2::new(x_of(rect, v), y),
            7.0,
            ui.visuals().panel_fill,
            Stroke::new(1.5, ui.visuals().text_color()),
        );
    }
    (rect, changed)
}

/// Sets deadzone value `h` (0 low, 1 center-low, 2 center-high, 3 high) keeping the order;
/// symmetric mirrors the partner (low <-> high, center-low <-> center-high).
fn set_handle(dz: &mut [f64; 4], h: usize, v: f64, symmetric: bool) {
    let v = match h {
        0 => v.clamp(-1.0, dz[1]),
        1 => v.clamp(dz[0], 0.0),
        2 => v.clamp(0.0, dz[3]),
        _ => v.clamp(dz[2], 1.0),
    };
    dz[h] = v;
    if symmetric {
        let (partner, mirrored) = (3 - h, -v);
        dz[partner] = mirrored;
        // Keep the order after mirroring.
        dz[1] = dz[1].clamp(dz[0], 0.0);
        dz[2] = dz[2].clamp(0.0, dz[3]);
    }
}

/// The four numbers right under their slider handles (centre ones beside the middle).
fn deadzone_fields(ui: &mut egui::Ui, slider: Rect, dz: &mut [f64; 4], symmetric: bool) -> bool {
    let (w, h) = (66.0, 20.0);
    let (row, _) = ui.allocate_exact_size(Vec2::new(slider.width(), h), Sense::hover());
    let mid = row.center().x;
    let lefts = [
        row.left(),
        (x_of(slider, dz[1]) - w / 2.0).clamp(row.left() + w + 4.0, mid - w - 2.0),
        (x_of(slider, dz[2]) - w / 2.0).clamp(mid + 2.0, row.right() - 2.0 * w - 4.0),
        row.right() - w,
    ];
    let mut changed = false;
    for (i, left) in lefts.into_iter().enumerate() {
        let mut v = dz[i];
        let rect = Rect::from_min_size(Pos2::new(left, row.top()), Vec2::new(w, h));
        let drag = egui::DragValue::new(&mut v)
            .range(-1.0..=1.0)
            .speed(0.005)
            .fixed_decimals(4);
        if ui.put(rect, drag).changed() {
            set_handle(dz, i, v, symmetric);
            changed = true;
        }
    }
    changed
}
