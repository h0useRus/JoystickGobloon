//! The wiring schematic: a lane per output, its controllers left of it, its game actions right.

use super::bind::{add_ops, remove_ops};
use super::graph::ActionNode;
use super::state::State;
use super::*;

impl GameView {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wiring(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        game: &str,
        graph: &Graph,
        live: &Live,
        state: &State,
        now: f64,
        ops: &mut Vec<Op>,
    ) {
        if graph.outputs.is_empty() {
            crate::app::empty_state(ui, icon::FLOW_ARROW, &i18n.tr("game-no-outputs"));
            return;
        }
        let searching = !self.search.trim().is_empty();
        let lanes = Lanes::build(graph, i18n, searching, self.picked.as_ref());
        let look = Look {
            accent: style::accent(ui),
            weak: ui.visuals().weak_text_color(),
            text: ui.visuals().text_color(),
            fill: ui.visuals().faint_bg_color,
            font: FontId::proportional(13.0),
        };
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let width = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(width, (lanes.rows + 1) as f32 * ROW),
                    Sense::hover(),
                );
                let col_w = (width - 2.0 * GAP) / 3.0;
                let xs = [
                    rect.left(),
                    rect.left() + col_w + GAP,
                    rect.left() + 2.0 * (col_w + GAP),
                ];
                let painter = ui.painter_at(rect);
                for (x, key) in
                    xs.iter()
                        .zip(["game-col-inputs", "game-col-outputs", "game-col-actions"])
                {
                    painter.text(
                        Pos2::new(*x + 8.0, rect.top() + ROW / 2.0),
                        Align2::LEFT_CENTER,
                        i18n.tr(key),
                        FontId::proportional(12.0),
                        look.weak,
                    );
                }
                let node = |col: usize, row: usize| {
                    Rect::from_min_size(
                        Pos2::new(xs[col], rect.top() + ROW + row as f32 * ROW + 2.0),
                        Vec2::new(col_w, ROW - 4.0),
                    )
                };
                let visible = ui.clip_rect();
                let stroke = |on: f32| {
                    if on > 0.0 {
                        Stroke::new(2.5, look.accent.gamma_multiply((0.4 + on).min(1.0)))
                    } else {
                        Stroke::new(1.0, line)
                    }
                };
                for (row, name) in &lanes.headers {
                    let r = node(0, *row);
                    painter.text(
                        Pos2::new(rect.left() + 2.0, r.center().y + 2.0),
                        Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(13.0),
                        look.text,
                    );
                    painter.hline(rect.x_range(), r.bottom(), Stroke::new(1.0, line));
                }
                // Game actions the search found that no output drives yet.
                for (row, r) in &lanes.loose {
                    let at = node(2, *row);
                    if at.intersects(visible) {
                        let a = &graph.actions[*r];
                        let v = self.fade(&a.id, now);
                        self.action_node(
                            ui, &painter, i18n, game, state, a, at, v, None, ops, &look,
                        );
                    }
                }
                // One lane per output: its controllers left of it, its game actions right of
                // it, so no wire leaves the lane.
                for (n, lane) in lanes.lanes.iter().enumerate() {
                    let m = lane.output;
                    let o = &graph.outputs[m];
                    let band = Rect::from_min_max(
                        Pos2::new(rect.left(), node(0, lane.top).top() - 1.0),
                        Pos2::new(
                            rect.right(),
                            node(0, lane.top + lane.height - 1).bottom() + 1.0,
                        ),
                    );
                    if !band.intersects(visible) {
                        continue;
                    }
                    if n % 2 == 1 {
                        painter.rect_filled(band, 6.0, look.fill.gamma_multiply(0.45));
                    }
                    let at = node(1, lane.top);
                    let lit = live.outputs.get(m).copied().unwrap_or(0.0);
                    for (k, l) in lane.inputs.iter().enumerate() {
                        let on = live.inputs.get(*l).copied().unwrap_or(0.0).min(lit);
                        wire(
                            &painter,
                            node(0, lane.top + k).right_center(),
                            at.left_center(),
                            stroke(on),
                        );
                    }
                    for (k, r) in lane.actions.iter().enumerate() {
                        let on = lit.max(self.fade(&graph.actions[*r].id, now));
                        wire(
                            &painter,
                            at.right_center(),
                            node(2, lane.top + k).left_center(),
                            stroke(on),
                        );
                    }
                    for (k, l) in lane.inputs.iter().enumerate() {
                        let i = &graph.inputs[*l];
                        let v = live.inputs.get(*l).copied().unwrap_or(0.0);
                        draw_node(
                            &painter,
                            node(0, lane.top + k),
                            &i.label,
                            v,
                            i.axis.then_some(v),
                            look.accent,
                            look.fill,
                            look.text,
                            &look.font,
                        );
                    }
                    let picked = self.picked.as_ref() == Some(o);
                    draw_node(
                        &painter,
                        at,
                        &output_parts(i18n, o).1,
                        lit,
                        None,
                        look.accent,
                        if picked {
                            look.accent.gamma_multiply(0.35)
                        } else {
                            look.fill
                        },
                        if lane.actions.is_empty() && lane.inputs.is_empty() {
                            look.weak
                        } else {
                            look.text
                        },
                        &look.font,
                    );
                    let resp = ui.interact(at, ui.scope_id().with(("game-out", m)), Sense::click());
                    if resp.clicked() {
                        self.picked = if picked { None } else { Some(o.clone()) };
                    }
                    resp.on_hover_text(format!(
                        "{}\n{}",
                        output_name(i18n, o),
                        i18n.tr("game-pick-output-hint")
                    ));
                    for (k, r) in lane.actions.iter().enumerate() {
                        let a = &graph.actions[*r];
                        let v = lit.max(self.fade(&a.id, now));
                        let at = node(2, lane.top + k);
                        self.action_node(
                            ui,
                            &painter,
                            i18n,
                            game,
                            state,
                            a,
                            at,
                            v,
                            Some(o),
                            ops,
                            &look,
                        );
                    }
                }
            });
    }

    /// One game action of the schematic: a click links the picked output to it, the bin
    /// unlinks it from `lane` (the output whose lane it is drawn in).
    #[allow(clippy::too_many_arguments)]
    fn action_node(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        i18n: &I18n,
        game: &str,
        state: &State,
        a: &ActionNode,
        r: Rect,
        lit: f32,
        lane: Option<&GameOutput>,
        ops: &mut Vec<Op>,
        look: &Look,
    ) {
        let label = if a.known {
            a.name.clone()
        } else {
            format!("{}  {}", icon::WARNING, a.name)
        };
        draw_node(
            painter,
            r,
            &label,
            lit,
            None,
            look.accent,
            look.fill,
            if a.bound { look.text } else { look.weak },
            &look.font,
        );
        let salt = (lane.map(GameOutput::to_string), &a.id);
        let resp = ui.interact(r, ui.scope_id().with(("game-act", &salt)), Sense::click());
        let mut tip = if a.category.is_empty() {
            format!("{}\n{}", a.name, a.id)
        } else {
            format!("{}\n{}\n{}", a.name, a.category, a.id)
        };
        if !a.known {
            tip.push('\n');
            tip.push_str(&i18n.tr("game-not-in-catalog"));
        }
        if let Some(o) = self.picked.clone() {
            tip.push('\n');
            tip.push_str(&i18n.tr("game-click-to-link"));
            if resp.clicked() {
                add_ops(game, state, &a.id, &o, None, ops);
                self.picked = None;
                self.find = None;
            }
        }
        if let Some(o) = lane {
            let bin =
                Rect::from_center_size(r.right_center() - Vec2::new(12.0, 0.0), Vec2::splat(18.0));
            if resp.hovered() || ui.rect_contains_pointer(bin) {
                painter.text(
                    bin.center(),
                    Align2::CENTER_CENTER,
                    icon::TRASH,
                    FontId::proportional(13.0),
                    look.weak,
                );
                let b = ui.interact(
                    bin,
                    ui.scope_id().with(("game-unbind", &salt)),
                    Sense::click(),
                );
                if b.on_hover_text(i18n.tr("game-unbind-one")).clicked() {
                    remove_ops(game, state, &a.id, o, ops);
                }
            }
        }
        resp.on_hover_text(tip);
    }
}

/// One output of the schematic with what it connects: the controller inputs writing it and the
/// game actions it drives share its rows.
struct Lane {
    output: usize,
    top: usize,
    height: usize,
    inputs: Vec<usize>,
    actions: Vec<usize>,
}

/// Rows of the schematic: a header per virtual device, then a lane per output.
struct Lanes {
    /// (row, text).
    headers: Vec<(usize, String)>,
    /// (row, action): actions a search found that no output drives.
    loose: Vec<(usize, usize)>,
    lanes: Vec<Lane>,
    rows: usize,
}

impl Lanes {
    /// While `searching`, only the outputs driving a found action and the `picked` one stay.
    fn build(graph: &Graph, i18n: &I18n, searching: bool, picked: Option<&GameOutput>) -> Self {
        let mut row = 0;
        let mut headers = Vec::new();
        let mut loose = Vec::new();
        let free =
            (0..graph.actions.len()).filter(|r| !graph.action_edges.iter().any(|(_, a)| a == r));
        for r in free {
            if loose.is_empty() {
                headers.push((row, i18n.tr("game-found")));
                row += 1;
            }
            loose.push((row, r));
            row += 1;
        }
        let mut lanes = Vec::new();
        let mut device: Option<String> = None;
        for (m, o) in graph.outputs.iter().enumerate() {
            let actions: Vec<usize> = graph
                .action_edges
                .iter()
                .filter(|(output, _)| *output == m)
                .map(|(_, r)| *r)
                .collect();
            if searching && actions.is_empty() && picked != Some(o) {
                continue;
            }
            let inputs: Vec<usize> = graph
                .input_edges
                .iter()
                .filter(|(_, output)| *output == m)
                .map(|(l, _)| *l)
                .collect();
            let name = output_parts(i18n, o).0;
            if device.as_ref() != Some(&name) {
                headers.push((row, name.clone()));
                device = Some(name);
                row += 1;
            }
            let height = inputs.len().max(actions.len()).max(1);
            lanes.push(Lane {
                output: m,
                top: row,
                height,
                inputs,
                actions,
            });
            row += height;
        }
        Self {
            headers,
            loose,
            lanes,
            rows: row,
        }
    }
}

/// Colors and font of the schematic's nodes.
struct Look {
    accent: Color32,
    weak: Color32,
    text: Color32,
    fill: Color32,
    font: FontId,
}

fn wire(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let dx = ((to.x - from.x) / 2.0).max(20.0);
    let shape = egui::epaint::CubicBezierShape::from_points_stroke(
        [from, from + Vec2::new(dx, 0.0), to - Vec2::new(dx, 0.0), to],
        false,
        Color32::TRANSPARENT,
        stroke,
    );
    painter.add(shape);
}

#[allow(clippy::too_many_arguments)]
fn draw_node(
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    lit: f32,
    bar: Option<f32>,
    accent: Color32,
    fill: Color32,
    text: Color32,
    font: &FontId,
) {
    let radius = 6.0;
    painter.rect_filled(rect, radius, fill);
    if let Some(v) = bar {
        let w = rect.width() * v.clamp(0.0, 1.0);
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
            radius,
            accent.gamma_multiply(0.25),
        );
    }
    let stroke = if lit > 0.0 {
        Stroke::new(2.0, accent.gamma_multiply(0.4 + 0.6 * lit.min(1.0)))
    } else {
        Stroke::new(1.0, text.gamma_multiply(0.25))
    };
    painter.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    let galley = painter.layout(label.to_string(), font.clone(), text, rect.width() - 28.0);
    let pos = Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
    painter
        .with_clip_rect(rect.shrink(2.0))
        .galley(pos, galley, text);
}
