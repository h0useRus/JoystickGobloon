//! How a plugin's controls look on its picture, in Gobloon's green, for every program that shows
//! them (mapping, the Input viewer, the Game page): a hat is a pad (four segments split by an X, each with
//! a chevron or its button's number, push in the middle), stick axes a pad with a dot where the
//! stick is inside a ring whose arc shows the twist, one axis an upright gauge, anything else a
//! round tag. Parts light up while they are worked.

use egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Vec2};

use crate::{Control, Input, Inputs, Kind};

/// A hat's parts in the order its pad draws them.
pub const HAT_PARTS: [&str; 5] = ["up", "right", "down", "left", "push"];

/// Hat and stick pad radius, in screen points.
pub const RADIUS: f32 = 32.0;
/// The hat's middle (push) radius.
const MIDDLE: f32 = 12.5;
/// Gauge size.
pub const GAUGE: Vec2 = Vec2::new(14.0, 36.0);

/// One part of a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Nothing on it yet.
    Empty,
    Mapped,
    /// Worked now.
    Lit,
}

pub struct Pal {
    pub accent: Color32,
    pub strong: Color32,
    pub fill: Color32,
    pub weak: Color32,
}

impl Pal {
    pub fn new(visuals: &egui::Visuals, accent: Color32) -> Self {
        Self {
            accent,
            strong: visuals.strong_text_color(),
            fill: visuals.extreme_bg_color,
            weak: visuals.weak_text_color(),
        }
    }

    /// A part's fill, outline and ink: dark with a green outline and light text when it has
    /// an input, dark and grey when not, green while worked or under a dragged input.
    fn part(&self, part: Part, hot: bool) -> (Color32, Color32, Color32) {
        match (part, hot) {
            (_, true) | (Part::Lit, _) => (self.accent, self.accent, self.fill),
            (Part::Mapped, _) => (
                self.accent.gamma_multiply(0.28),
                self.accent.gamma_multiply(0.85),
                self.strong,
            ),
            (Part::Empty, _) => (self.fill, self.weak.gamma_multiply(0.6), self.weak),
        }
    }
}

/// The dark plate under a pad, its rim bright when `strong` (selected or hovered) and a glow
/// while it is worked.
fn plate(painter: &egui::Painter, center: Pos2, radius: f32, strong: bool, lit: bool, pal: &Pal) {
    if lit {
        painter.circle_filled(center, radius + 7.0, pal.accent.gamma_multiply(0.3));
    }
    painter.circle_filled(center, radius + 2.5, Color32::BLACK.gamma_multiply(0.5));
    let ring = if strong || lit {
        Stroke::new(2.0, pal.strong)
    } else {
        Stroke::new(1.3, pal.accent.gamma_multiply(0.7))
    };
    painter.circle(center, radius, pal.fill, ring);
}

/// One part of a hat pad: its state and what it shows (the button's number, or nothing).
pub struct PadPart {
    pub part: Part,
    pub label: String,
}

/// A hat as a pad: four segments (up, right, down, left) split by an X around a middle (push,
/// when the hat has one), each with its button's number; `over` = the part a dragged input
/// would go to.
pub fn hat(
    painter: &egui::Painter,
    center: Pos2,
    parts: &[PadPart],
    over: Option<usize>,
    strong: bool,
    pal: &Pal,
) {
    let lit = parts.iter().any(|p| p.part == Part::Lit);
    plate(painter, center, RADIUS, strong, lit, pal);
    let font = FontId::proportional(12.0);
    if parts.len() < 5 {
        // No push: a plain middle.
        let (fill, edge, _) = pal.part(Part::Empty, false);
        painter.circle(center, MIDDLE, fill, Stroke::new(1.2, edge));
    }
    for (n, p) in parts.iter().enumerate().take(5) {
        let (fill, edge, ink) = pal.part(p.part, over == Some(n));
        if n < 4 {
            let outline = segment(center, n);
            painter.add(segment_mesh(&outline, fill));
            painter.add(egui::Shape::closed_line(outline, Stroke::new(1.2, edge)));
            // Its button's number, or a chevron while it has none (or is a hat's direction).
            let dir = direction(n);
            let at = center + dir * (MIDDLE + 2.5 + RADIUS - 3.0) / 2.0;
            if p.label.is_empty() {
                chevron(painter, at, dir, 4.0, ink);
            } else {
                painter.text(at, Align2::CENTER_CENTER, &p.label, font.clone(), ink);
            }
        } else {
            painter.circle(center, MIDDLE, fill, Stroke::new(1.2, edge));
            painter.text(center, Align2::CENTER_CENTER, &p.label, font.clone(), ink);
        }
    }
}

/// Which part of a hat pad `offset` from its middle is on: 0-3 = up, right, down, left,
/// 4 = push (when the hat has one); `None` outside the pad.
pub fn hat_part_at(offset: Vec2, push: bool) -> Option<usize> {
    let d = offset.length();
    if d > RADIUS + 3.0 {
        return None;
    }
    if push && d <= MIDDLE + 1.0 {
        return Some(4);
    }
    Some(if offset.y.abs() >= offset.x.abs() {
        if offset.y < 0.0 { 0 } else { 2 }
    } else if offset.x > 0.0 {
        1
    } else {
        3
    })
}

/// Stick axes: a pad with a chevron per side and a dot where the stick is (`at` = x, y, -1 to
/// 1), inside a ring for the twist with a dot where the grip is turned.
#[allow(clippy::too_many_arguments)]
pub fn stick(
    painter: &egui::Painter,
    center: Pos2,
    at: Option<[f32; 2]>,
    twist: Option<f32>,
    xy: Part,
    turn: Part,
    strong: bool,
    pal: &Pal,
) {
    let lit = xy == Part::Lit || turn == Part::Lit;
    plate(painter, center, RADIUS, strong, lit, pal);
    // The twist ring: a thin track with a mark at its top (the grip straight); the twist runs
    // from the mark toward where the grip is turned and ends in a knob.
    let (_, edge, _) = pal.part(turn, false);
    let band = RADIUS - 5.0;
    let track = match turn {
        Part::Empty => pal.weak.gamma_multiply(0.25),
        _ => pal.accent.gamma_multiply(0.25),
    };
    painter.circle_stroke(center, band, Stroke::new(3.0, track));
    let top = -std::f32::consts::FRAC_PI_2;
    let up = Vec2::angled(top);
    painter.line_segment(
        [center + up * (band - 4.0), center + up * (band + 4.0)],
        Stroke::new(1.5, edge),
    );
    if let Some(twist) = twist {
        let turn_by = twist.clamp(-1.0, 1.0) * std::f32::consts::FRAC_PI_2 * 1.2;
        let ink = if turn == Part::Lit {
            pal.accent
        } else {
            pal.accent.gamma_multiply(0.85)
        };
        let points: Vec<Pos2> = (0..=16)
            .map(|k| center + Vec2::angled(top + turn_by * k as f32 / 16.0) * band)
            .collect();
        painter.line(points, Stroke::new(3.0, ink));
        painter.circle(
            center + Vec2::angled(top + turn_by) * band,
            3.8,
            ink,
            Stroke::new(1.5, pal.fill),
        );
    }
    // The pad.
    let inner = band - 6.0;
    let (fill, edge, ink) = pal.part(xy, false);
    painter.circle(center, inner, fill, Stroke::new(1.2, edge));
    for n in 0..4 {
        let dir = direction(n);
        chevron(painter, center + dir * (inner - 4.5), dir, 2.8, ink);
    }
    let [x, y] = at.unwrap_or([0.0, 0.0]).map(|v| v.clamp(-1.0, 1.0));
    let dot = center + Vec2::new(x, y) * (inner - 8.0);
    painter.circle(dot, 3.8, pal.accent, Stroke::new(1.2, pal.fill));
}

/// One axis: an upright bar filled from its middle to `value` (-1 to 1).
pub fn gauge(
    painter: &egui::Painter,
    center: Pos2,
    value: Option<f32>,
    part: Part,
    strong: bool,
    pal: &Pal,
) {
    let rect = Rect::from_center_size(center, GAUGE);
    if part == Part::Lit {
        painter.rect_filled(rect.expand(6.0), 8, pal.accent.gamma_multiply(0.3));
    }
    // Selected or hovered: a green rim (no white outline).
    let ring = if strong {
        Stroke::new(2.0, pal.accent)
    } else {
        Stroke::new(1.0, Color32::BLACK.gamma_multiply(0.6))
    };
    painter.rect(
        rect.expand(2.0),
        CornerRadius::same((GAUGE.x / 2.0 + 2.0) as u8),
        Color32::from_rgb(0x14, 0x16, 0x18),
        ring,
        egui::StrokeKind::Inside,
    );
    let (fill, _, _) = pal.part(part, false);
    let inner = rect.shrink(2.0);
    painter.rect_filled(
        inner,
        CornerRadius::same((inner.width() / 2.0) as u8),
        pal.accent.gamma_multiply(0.28),
    );
    if let Some(v) = value {
        let v = v.clamp(-1.0, 1.0);
        let y = inner.center().y - v * inner.height() / 2.0;
        let bar = Rect::from_x_y_ranges(
            inner.x_range(),
            y.min(inner.center().y)..=y.max(inner.center().y),
        );
        painter.rect_filled(bar, 3, fill);
    }
}

fn direction(n: usize) -> Vec2 {
    match n {
        0 => Vec2::new(0.0, -1.0),
        1 => Vec2::new(1.0, 0.0),
        2 => Vec2::new(0.0, 1.0),
        _ => Vec2::new(-1.0, 0.0),
    }
}

/// A small chevron at `at` pointing `dir`.
fn chevron(painter: &egui::Painter, at: Pos2, dir: Vec2, size: f32, color: Color32) {
    let side = dir.rot90();
    let tip = at + dir * size * 0.6;
    let back = at - dir * size * 0.6;
    painter.line(
        vec![back + side * size, tip, back - side * size],
        Stroke::new(1.6, color),
    );
}

/// The outline of segment `n`: outer arc, then inner arc back, a gap to its neighbours (the
/// X between the segments).
fn segment(center: Pos2, n: usize) -> Vec<Pos2> {
    let mid = direction(n).angle();
    let (outer, inner) = (RADIUS, MIDDLE + 2.5);
    let gap_out = 1.8 / outer;
    let gap_in = 1.8 / inner;
    let quarter = std::f32::consts::FRAC_PI_4;
    let steps = 12;
    let mut points = Vec::with_capacity(2 * (steps + 1));
    for k in 0..=steps {
        let a = mid - quarter + gap_out + (2.0 * (quarter - gap_out)) * k as f32 / steps as f32;
        points.push(center + Vec2::angled(a) * outer);
    }
    for k in (0..=steps).rev() {
        let a = mid - quarter + gap_in + (2.0 * (quarter - gap_in)) * k as f32 / steps as f32;
        points.push(center + Vec2::angled(a) * inner);
    }
    points
}

/// A filled segment: the outline is not convex, so it is drawn as a strip of quads.
fn segment_mesh(outline: &[Pos2], fill: Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    let half = outline.len() / 2;
    for p in outline {
        mesh.colored_vertex(*p, fill);
    }
    for k in 0..half - 1 {
        let (o0, o1) = (k as u32, k as u32 + 1);
        // The inner arc runs backwards: outer point k pairs with inner point len-1-k.
        let (i0, i1) = ((2 * half - 1 - k) as u32, (2 * half - 2 - k) as u32);
        mesh.add_triangle(o0, o1, i1);
        mesh.add_triangle(o0, i1, i0);
    }
    mesh
}

/// Round tag radius, in screen points.
pub const TAG: f32 = 12.0;

/// A tag: round for buttons, square for axes (still waiting to be placed), with its number or
/// label; a glow while it is worked, a bright rim when `strong`.
#[allow(clippy::too_many_arguments)]
pub fn tag(
    painter: &egui::Painter,
    pal: &Pal,
    at: Pos2,
    label: &str,
    lit: bool,
    strong: bool,
    square: bool,
) {
    let ring = if strong {
        Stroke::new(2.5, pal.strong)
    } else {
        Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 200))
    };
    if square {
        let rect = Rect::from_center_size(at, Vec2::splat(TAG * 2.0));
        if lit {
            painter.rect_filled(rect.expand(7.0), 6.0, pal.accent.gamma_multiply(0.45));
        }
        let fill = Color32::from_rgb(0x4a, 0xa8, 0xe0);
        painter.rect(rect, 4.0, fill, ring, egui::StrokeKind::Middle);
    } else {
        if lit {
            painter.circle_filled(at, TAG + 7.0, pal.accent.gamma_multiply(0.45));
        }
        painter.circle_filled(at, TAG, pal.accent);
        painter.circle_stroke(at, TAG, ring);
    }
    painter.text(
        at,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::from_rgb(0x10, 0x12, 0x14),
    );
}

/// A caption on a dark plate, centered at `at`.
pub fn caption(painter: &egui::Painter, pal: &Pal, at: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(13.0), pal.strong);
    let rect = Rect::from_center_size(at, galley.size() + Vec2::new(12.0, 6.0));
    painter.rect_filled(rect, 4.0, pal.fill.gamma_multiply(0.92));
    painter.galley(rect.min + Vec2::new(6.0, 3.0), galley, pal.strong);
}

/// A control drawn off its spot: a line from the edge of its glyph at `glyph` to a dot on the
/// spot, bright while the control is worked.
pub fn leader(painter: &egui::Painter, pal: &Pal, spot: Pos2, glyph: Pos2, reach: f32, lit: bool) {
    let color = if lit {
        pal.accent
    } else {
        pal.accent.gamma_multiply(0.7)
    };
    let away = glyph - spot;
    if away.length() > reach + 4.0 {
        let start = glyph - away.normalized() * (reach + 2.0);
        painter.line_segment(
            [start, spot],
            Stroke::new(3.0, Color32::BLACK.gamma_multiply(0.5)),
        );
        painter.line_segment(
            [start, spot],
            Stroke::new(if lit { 2.0 } else { 1.3 }, color),
        );
    }
    painter.circle(spot, 4.5, pal.fill, Stroke::new(2.0, color));
    if lit {
        painter.circle_filled(spot, 2.0, pal.accent);
    }
}

/// The line of control `c` when it is drawn off its spot (`to_screen` maps picture units onto
/// the screen); nothing when it sits on its spot.
pub fn leader_of(
    painter: &egui::Painter,
    pal: &Pal,
    c: &Control,
    inputs: &Inputs,
    to_screen: &dyn Fn([f32; 2]) -> Pos2,
    live: &dyn Fn(Input) -> Live,
) {
    if let Some(glyph) = c.glyph {
        let lit = c
            .parts()
            .iter()
            .filter_map(|p| inputs.get(&c.id, p))
            .any(|i| live(i).lit);
        leader(
            painter,
            pal,
            to_screen(c.at),
            to_screen(glyph),
            reach(c),
            lit,
        );
    }
}

/// The picture units a view's controls need besides the picture itself (`size`): the picture
/// grown to every glyph moved off its spot, with room for the glyph (`margin` units).
pub fn extent<'a>(
    controls: impl IntoIterator<Item = &'a Control>,
    size: Vec2,
    margin: f32,
) -> Rect {
    let mut rect = Rect::from_min_size(Pos2::ZERO, size);
    for c in controls {
        if let Some([x, y]) = c.glyph {
            rect = rect.union(Rect::from_center_size(
                Pos2::new(x, y),
                Vec2::splat(margin * 2.0),
            ));
        }
    }
    rect
}

/// How far from its spot a control is still under the pointer.
pub fn reach(c: &Control) -> f32 {
    match c.kind {
        Kind::Hat | Kind::Stick => RADIUS,
        Kind::Axis => GAUGE.y / 2.0,
        _ => TAG + 2.0,
    }
}

/// What the device does with one input now.
#[derive(Debug, Clone, Copy, Default)]
pub struct Live {
    /// Held, or an axis moving.
    pub lit: bool,
    /// Where an axis is, -1 to 1.
    pub value: Option<f32>,
}

/// Control `c` at `at` by its kind, its parts' inputs from `inputs` and what each does now from
/// `live`; `badge` = the number on a button's tag (none: the button's own
/// number); `over` = the hat part a dragged input would go to.
#[allow(clippy::too_many_arguments)]
pub fn control(
    painter: &egui::Painter,
    pal: &Pal,
    c: &Control,
    inputs: &Inputs,
    at: Pos2,
    badge: Option<usize>,
    strong: bool,
    over: Option<&str>,
    live: &dyn Fn(Input) -> Live,
) {
    let state = |input: Option<Input>| match input {
        Some(i) if live(i).lit => Part::Lit,
        Some(_) => Part::Mapped,
        None => Part::Empty,
    };
    let value = |input: Option<Input>| input.and_then(|i| live(i).value);
    match c.kind {
        Kind::Hat => {
            let push = c.parts().contains(&"push");
            let shown = if push {
                &HAT_PARTS[..]
            } else {
                &HAT_PARTS[..4]
            };
            let parts: Vec<PadPart> = shown
                .iter()
                .map(|p| {
                    let input = inputs.get(&c.id, p);
                    // A button shows its number, a hat direction (or nothing) its chevron.
                    let label = match input {
                        Some(Input::Button(b)) => b.to_string(),
                        _ => String::new(),
                    };
                    PadPart {
                        part: state(input),
                        label,
                    }
                })
                .collect();
            let over = over.and_then(|o| HAT_PARTS.iter().position(|p| *p == o));
            hat(painter, at, &parts, over, strong, pal);
        }
        Kind::Stick => {
            let x = inputs.get(&c.id, "x");
            let y = inputs.get(&c.id, "y");
            let twist = inputs.get(&c.id, "twist");
            let xy = value(x)
                .or(value(y))
                .map(|_| [value(x).unwrap_or(0.0), value(y).unwrap_or(0.0)]);
            let pad = match (state(x), state(y)) {
                (Part::Lit, _) | (_, Part::Lit) => Part::Lit,
                (Part::Empty, Part::Empty) => Part::Empty,
                _ => Part::Mapped,
            };
            stick(
                painter,
                at,
                xy,
                value(twist),
                pad,
                state(twist),
                strong,
                pal,
            );
        }
        Kind::Axis => {
            let input = inputs.get(&c.id, "");
            // The gauge shows the axis itself: no number of its own.
            gauge(painter, at, value(input), state(input), strong, pal);
        }
        _ => {
            let own: Vec<Input> = c
                .parts()
                .iter()
                .filter_map(|p| inputs.get(&c.id, p))
                .collect();
            let lit = own.iter().any(|i| live(*i).lit);
            let label = match (badge, own.as_slice()) {
                (Some(n), _) => n.to_string(),
                (None, [Input::Button(b)]) => b.to_string(),
                _ => String::new(),
            };
            tag(painter, pal, at, &label, lit, strong, false);
        }
    }
}
