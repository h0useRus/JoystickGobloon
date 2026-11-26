//! How placed controls look on the picture, in Gobloon's green: a hat is a pad (four segments
//! split by an X, each with a chevron and its button's number, push in the middle), stick
//! axes a pad with a dot where the stick is inside a ring whose dot shows the twist, one axis an
//! upright gauge; each carries its number in a small badge. Parts light up while they are
//! worked.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Vec2};

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
    fill: Color32,
    weak: Color32,
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
    // The twist ring: a band with a mark at its top; the twist fills it from the mark toward
    // where the grip is turned, arrows either side of the mark show the two ways.
    let (_, edge, _) = pal.part(turn, false);
    let band = RADIUS - 5.0;
    let track = match turn {
        Part::Empty => pal.weak.gamma_multiply(0.25),
        _ => pal.accent.gamma_multiply(0.22),
    };
    painter.circle_stroke(center, band, Stroke::new(6.0, track));
    let top = -std::f32::consts::FRAC_PI_2;
    let up = Vec2::angled(top);
    painter.line_segment(
        [center + up * (band - 3.5), center + up * (band + 3.5)],
        Stroke::new(1.5, edge),
    );
    for side in [-1.0f32, 1.0] {
        let a = top + side * 0.75;
        let tangent = Vec2::angled(a + side * std::f32::consts::FRAC_PI_2);
        chevron(painter, center + Vec2::angled(a) * band, tangent, 2.6, edge);
    }
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
        painter.line(points, Stroke::new(4.0, ink));
        painter.circle(
            center + Vec2::angled(top + turn_by) * band,
            3.6,
            ink,
            Stroke::new(1.2, pal.fill),
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
    let ring = if strong {
        Stroke::new(2.0, pal.strong)
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
        painter.line_segment(
            [Pos2::new(inner.left(), y), Pos2::new(inner.right(), y)],
            Stroke::new(2.0, Color32::WHITE),
        );
    }
}

/// A control's number in a small badge at `at`.
pub fn number(painter: &egui::Painter, at: Pos2, n: usize, strong: bool, pal: &Pal) {
    let r = 8.5;
    painter.circle_filled(at, r, pal.accent);
    let ring = if strong {
        Stroke::new(2.0, pal.strong)
    } else {
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 180))
    };
    painter.circle_stroke(at, r, ring);
    painter.text(
        at,
        Align2::CENTER_CENTER,
        n.to_string(),
        FontId::proportional(10.0),
        Color32::from_rgb(0x10, 0x12, 0x14),
    );
}

/// Where a gauge's number badge goes: its top left.
pub fn gauge_number_at(center: Pos2) -> Pos2 {
    center + Vec2::new(-GAUGE.x / 2.0 - 6.0, -GAUGE.y / 2.0)
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
