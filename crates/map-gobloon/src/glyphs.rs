//! How placed controls look on the picture, after the device picture callouts of Joystick
//! Gobloon's archived Device tab: a hat is a compass (an arrow per direction, a dot for push),
//! stick axes a ring with a dot where the stick is and an arc for its twist, one axis an upright
//! gauge; each carries its number in a small badge. Parts light up while they are worked.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Vec2};

/// Compass and ring radius, in screen points.
pub const RADIUS: f32 = 17.0;
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
    pub fill: Color32,
    pub weak: Color32,
    pub strong: Color32,
}

impl Pal {
    pub fn new(visuals: &egui::Visuals, accent: Color32) -> Self {
        Self {
            accent,
            fill: visuals.extreme_bg_color,
            weak: visuals.weak_text_color(),
            strong: visuals.strong_text_color(),
        }
    }

    fn ink(&self, part: Part) -> Color32 {
        match part {
            Part::Lit => self.accent,
            Part::Mapped => self.accent.gamma_multiply(0.75),
            Part::Empty => self.weak.gamma_multiply(0.7),
        }
    }
}

/// The dark disc under a glyph; its ring is bright when `strong` (selected, hovered or lit).
fn disc(painter: &egui::Painter, center: Pos2, radius: f32, strong: bool, pal: &Pal) {
    painter.circle_filled(center, radius + 2.0, Color32::BLACK.gamma_multiply(0.45));
    let ring = if strong {
        Stroke::new(2.0, pal.strong)
    } else {
        Stroke::new(1.3, pal.accent.gamma_multiply(0.6))
    };
    painter.circle(center, radius, pal.fill, ring);
}

/// A hat: `parts` = up, right, down, left and, when it has one, push.
pub fn hat(painter: &egui::Painter, center: Pos2, parts: &[Part], strong: bool, pal: &Pal) {
    let size = RADIUS * 2.0;
    let lit = parts.contains(&Part::Lit);
    if lit {
        painter.circle_filled(center, RADIUS + 7.0, pal.accent.gamma_multiply(0.35));
    }
    disc(painter, center, RADIUS, strong || lit, pal);
    for (n, part) in parts.iter().enumerate().take(5) {
        let ink = pal.ink(*part);
        if n < 4 {
            painter.add(egui::Shape::convex_polygon(
                arrow_points(center, size, n),
                ink,
                Stroke::NONE,
            ));
        } else if *part == Part::Empty {
            painter.circle_stroke(center, size * 0.12, Stroke::new(1.3, ink));
        } else {
            painter.circle_filled(center, size * 0.12, ink);
        }
    }
}

/// Stick axes: a ring with arrows, a dot where the stick is (`at` = x, y, -1 to 1) and the
/// twist as an arc along the rim.
pub fn stick(
    painter: &egui::Painter,
    center: Pos2,
    at: Option<[f32; 2]>,
    twist: Option<f32>,
    lit: bool,
    strong: bool,
    pal: &Pal,
) {
    let r = RADIUS;
    if lit {
        painter.circle_filled(center, r + 7.0, pal.accent.gamma_multiply(0.35));
    }
    disc(painter, center, r, strong || lit, pal);
    painter.circle_stroke(
        center,
        r * 0.58,
        Stroke::new(1.0, pal.accent.gamma_multiply(0.4)),
    );
    let ink = if lit {
        pal.accent
    } else {
        pal.accent.gamma_multiply(0.7)
    };
    for n in 0..4 {
        painter.add(egui::Shape::convex_polygon(
            arrow_points(center, r * 2.6, n),
            ink,
            Stroke::NONE,
        ));
    }
    if let Some(twist) = twist {
        let turn = twist.clamp(-1.0, 1.0) * std::f32::consts::FRAC_PI_2;
        let top = -std::f32::consts::FRAC_PI_2;
        let points: Vec<Pos2> = (0..=12)
            .map(|k| center + Vec2::angled(top + turn * k as f32 / 12.0) * (r - 3.0))
            .collect();
        painter.line(points, Stroke::new(3.0, ink));
        painter.circle_filled(center + Vec2::angled(top + turn) * (r - 3.0), 2.5, ink);
    }
    let [x, y] = at.unwrap_or([0.0, 0.0]).map(|v| v.clamp(-1.0, 1.0));
    let dot = center + Vec2::new(x, y) * r * 0.58;
    painter.line_segment(
        [center, dot],
        Stroke::new(1.0, pal.accent.gamma_multiply(0.5)),
    );
    painter.circle(
        dot,
        r * 0.15,
        ink,
        Stroke::new(1.0, Color32::BLACK.gamma_multiply(0.5)),
    );
}

/// One axis: an upright bar filled from its middle to `value` (-1 to 1).
pub fn gauge(
    painter: &egui::Painter,
    center: Pos2,
    value: Option<f32>,
    lit: bool,
    strong: bool,
    pal: &Pal,
) {
    let rect = Rect::from_center_size(center, GAUGE);
    if lit {
        painter.rect_filled(rect.expand(6.0), 8, pal.accent.gamma_multiply(0.35));
    }
    painter.rect_filled(rect.expand(2.0), 8, Color32::BLACK.gamma_multiply(0.45));
    let ring = if strong || lit {
        Stroke::new(2.0, pal.strong)
    } else {
        Stroke::new(1.3, pal.accent.gamma_multiply(0.6))
    };
    painter.rect(
        rect,
        CornerRadius::same((GAUGE.x / 2.0) as u8),
        pal.fill,
        ring,
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(3.5);
    painter.line_segment(
        [inner.left_center(), inner.right_center()],
        Stroke::new(1.0, pal.accent.gamma_multiply(0.4)),
    );
    if let Some(v) = value {
        let v = v.clamp(-1.0, 1.0);
        let y = inner.center().y - v * inner.height() / 2.0;
        let fill = Rect::from_x_y_ranges(
            inner.x_range(),
            y.min(inner.center().y)..=y.max(inner.center().y),
        );
        painter.rect_filled(
            fill,
            2,
            pal.accent.gamma_multiply(if lit { 1.0 } else { 0.7 }),
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

/// Where a glyph's number badge goes: its top left.
pub fn number_at(center: Pos2, tall: bool) -> Pos2 {
    if tall {
        center + Vec2::new(-GAUGE.x / 2.0 - 6.0, -GAUGE.y / 2.0)
    } else {
        center + Vec2::splat(-RADIUS * 0.85)
    }
}

/// A small triangle pointing direction `n` (0 up, 1 right, 2 down, 3 left), near the rim of a
/// glyph `size` across.
fn arrow_points(center: Pos2, size: f32, n: usize) -> Vec<Pos2> {
    let (dir, side) = match n {
        0 => (Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0)),
        1 => (Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)),
        2 => (Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0)),
        _ => (Vec2::new(-1.0, 0.0), Vec2::new(0.0, -1.0)),
    };
    let tip = center + dir * size * 0.40;
    let base = center + dir * size * 0.24;
    vec![tip, base + side * size * 0.12, base - side * size * 0.12]
}
