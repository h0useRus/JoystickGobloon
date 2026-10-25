//! Response curves like Gremlin's `gremlin/spline.py`: piecewise linear, natural cubic
//! spline and cubic Bezier spline, with Gremlin's symmetric editing rules. Joystick Gobloon
//! adds a monotone cubic (Fritsch-Carlson, like PCHIP): smooth through the knots like the
//! cubic spline, but it never overshoots them, so it neither wiggles nor runs past -1 / 1.
//!
//! Points are kept as Gremlin stores them in `<control-points>`: for linear and cubic
//! curves the knots sorted by x; for Bezier the flat list `center, right handle,
//! (left handle, center, right handle)*, left handle, center`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveType {
    PiecewiseLinear,
    CubicSpline,
    CubicBezier,
    /// Joystick Gobloon only.
    MonotoneCubic,
}

impl CurveType {
    pub const ALL: [CurveType; 4] = [
        Self::PiecewiseLinear,
        Self::CubicSpline,
        Self::MonotoneCubic,
        Self::CubicBezier,
    ];

    /// Value of the `curve-type` property.
    pub fn name(self) -> &'static str {
        match self {
            Self::PiecewiseLinear => "PiecewiseLinear",
            Self::CubicSpline => "CubicSpline",
            Self::CubicBezier => "CubicBezierSpline",
            Self::MonotoneCubic => "MonotoneCubicSpline",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "CubicSpline" => Self::CubicSpline,
            "CubicBezierSpline" => Self::CubicBezier,
            "MonotoneCubicSpline" => Self::MonotoneCubic,
            _ => Self::PiecewiseLinear,
        }
    }

    /// Gremlin's points for a new curve of this type (the identity line).
    pub fn default_points(self) -> Vec<(f64, f64)> {
        match self {
            Self::CubicBezier => vec![(-1.0, -1.0), (-0.95, -0.95), (0.95, 0.95), (1.0, 1.0)],
            _ => vec![(-1.0, -1.0), (1.0, 1.0)],
        }
    }
}

/// A curve ready to evaluate (spline coefficients / Bezier lookup precomputed).
#[derive(Debug, Clone)]
pub enum Curve {
    Linear(Vec<(f64, f64)>),
    /// Knots and second derivatives.
    Cubic(Vec<(f64, f64)>, Vec<f64>),
    /// Knots and slopes (cubic Hermite).
    Hermite(Vec<(f64, f64)>, Vec<f64>),
    /// Per segment: 101 samples of (x, y) along t.
    Bezier(Vec<Vec<(f64, f64)>>),
}

const MIN_SEGMENT: f64 = 1e-9;

impl Curve {
    pub fn new(kind: CurveType, points: &[(f64, f64)]) -> Self {
        let sorted = || {
            let mut p = points.to_vec();
            p.sort_by(|a, b| a.0.total_cmp(&b.0));
            p
        };
        match kind {
            CurveType::PiecewiseLinear => Self::Linear(sorted()),
            CurveType::CubicSpline => {
                let p = sorted();
                let z = second_derivatives(&p);
                Self::Cubic(p, z)
            }
            CurveType::CubicBezier => Self::Bezier(bezier_lookup(points)),
            CurveType::MonotoneCubic => {
                let p = sorted();
                let m = monotone_slopes(&p);
                Self::Hermite(p, m)
            }
        }
    }

    /// Output for an input in -1..1, clamped like Gremlin.
    pub fn eval(&self, x: f64) -> f64 {
        let x = x.clamp(-1.0, 1.0);
        match self {
            Self::Linear(p) => linear(p, x),
            Self::Cubic(p, z) => cubic(p, z, x),
            Self::Hermite(p, m) => hermite(p, m, x),
            Self::Bezier(segments) => bezier(segments, x),
        }
    }

    /// Whether the output only rises or only falls over the input range (flat parts allowed),
    /// so it crosses zero, or any other level, at most once. A Bezier curve is followed as
    /// drawn: it must not loop back in x either.
    pub fn monotonic(&self) -> bool {
        const STEPS: u32 = 400;
        const EPS: f64 = 1e-6;
        let (x, y): (Vec<f64>, Vec<f64>) = match self {
            Self::Bezier(segments) => segments.iter().flatten().copied().unzip(),
            _ => (0..=STEPS)
                .map(|i| {
                    let x = -1.0 + 2.0 * f64::from(i) / f64::from(STEPS);
                    (x, self.eval(x))
                })
                .unzip(),
        };
        let steps = |v: &[f64]| v.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
        let (dx, dy) = (steps(&x), steps(&y));
        dx.iter().all(|&d| d >= -EPS)
            && (dy.iter().all(|&d| d >= -EPS) || dy.iter().all(|&d| d <= EPS))
    }
}

fn monotonic(kind: CurveType, points: &[(f64, f64)]) -> bool {
    Curve::new(kind, points).monotonic()
}

fn linear(p: &[(f64, f64)], x: f64) -> f64 {
    let (Some(first), Some(last)) = (p.first(), p.last()) else {
        return x;
    };
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    p.windows(2)
        .find(|w| w[0].0 <= x && x < w[1].0)
        .map_or(last.1, |w| {
            w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0).max(MIN_SEGMENT)
        })
}

/// Gremlin's CubicSpline.fit: natural spline, z[0] = z[n] = 0.
fn second_derivatives(p: &[(f64, f64)]) -> Vec<f64> {
    let mut z = vec![0.0; p.len()];
    if p.len() < 3 {
        return z;
    }
    let n = p.len() - 1;
    let (mut h, mut b, mut u, mut v) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for i in 0..n {
        h[i] = (p[i + 1].0 - p[i].0).max(MIN_SEGMENT);
        b[i] = (p[i + 1].1 - p[i].1) / h[i];
    }
    u[1] = 2.0 * (h[0] + h[1]);
    v[1] = 6.0 * (b[1] - b[0]);
    for i in 2..n {
        u[i] = 2.0 * (h[i] + h[i - 1]) - h[i - 1].powi(2) / u[i - 1];
        v[i] = 6.0 * (b[i] - b[i - 1]) - h[i - 1] * v[i - 1] / u[i - 1];
    }
    for i in (1..n).rev() {
        z[i] = (v[i] - h[i] * z[i + 1]) / u[i];
    }
    z
}

fn cubic(p: &[(f64, f64)], z: &[f64], x: f64) -> f64 {
    if p.len() < 2 {
        return x;
    }
    let i = (0..p.len() - 1)
        .find(|&i| p[i].0 <= x && x <= p[i + 1].0)
        .unwrap_or(p.len() - 2);
    let h = (p[i + 1].0 - p[i].0).max(MIN_SEGMENT);
    let dx = x - p[i].0;
    let mut t = z[i] / 2.0 + dx * (z[i + 1] - z[i]) / (6.0 * h);
    t = -(h / 6.0) * (z[i + 1] + 2.0 * z[i]) + (p[i + 1].1 - p[i].1) / h + dx * t;
    (p[i].1 + dx * t).clamp(-1.0, 1.0)
}

/// Fritsch-Carlson slopes (as PCHIP): zero at a local extremum of the knots, else a weighted
/// harmonic mean of the neighbouring secants, which keeps every segment within its knots.
fn monotone_slopes(p: &[(f64, f64)]) -> Vec<f64> {
    let n = p.len();
    if n < 2 {
        return vec![0.0; n];
    }
    let h: Vec<f64> = p
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).max(MIN_SEGMENT))
        .collect();
    let d: Vec<f64> = p
        .windows(2)
        .zip(&h)
        .map(|(w, h)| (w[1].1 - w[0].1) / h)
        .collect();
    if n == 2 {
        return vec![d[0]; 2];
    }
    let mut m = vec![0.0; n];
    for i in 1..n - 1 {
        if d[i - 1] * d[i] > 0.0 {
            let (w1, w2) = (2.0 * h[i] + h[i - 1], h[i] + 2.0 * h[i - 1]);
            m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
        }
    }
    // Ends: three-point estimate, kept from overshooting.
    let end = |h0: f64, h1: f64, d0: f64, d1: f64| {
        let s = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
        if s * d0 <= 0.0 {
            0.0
        } else if d0 * d1 <= 0.0 && s.abs() > 3.0 * d0.abs() {
            3.0 * d0
        } else {
            s
        }
    };
    m[0] = end(h[0], h[1], d[0], d[1]);
    m[n - 1] = end(h[n - 2], h[n - 3], d[n - 2], d[n - 3]);
    m
}

fn hermite(p: &[(f64, f64)], m: &[f64], x: f64) -> f64 {
    if p.len() < 2 {
        return x;
    }
    let i = (0..p.len() - 1)
        .find(|&i| p[i].0 <= x && x <= p[i + 1].0)
        .unwrap_or(p.len() - 2);
    let h = (p[i + 1].0 - p[i].0).max(MIN_SEGMENT);
    let t = ((x - p[i].0) / h).clamp(0.0, 1.0);
    let (t2, t3) = (t * t, t * t * t);
    let y = (2.0 * t3 - 3.0 * t2 + 1.0) * p[i].1
        + (t3 - 2.0 * t2 + t) * h * m[i]
        + (-2.0 * t3 + 3.0 * t2) * p[i + 1].1
        + (t3 - t2) * h * m[i + 1];
    y.clamp(-1.0, 1.0)
}

/// Splits the flat Bezier list into segments `[center, right, left, center]`.
pub fn bezier_segments(points: &[(f64, f64)]) -> Vec<[(f64, f64); 4]> {
    if points.len() < 4 {
        return Vec::new();
    }
    let mut segs = Vec::new();
    let mut i = 0;
    while i + 3 < points.len() {
        segs.push([points[i], points[i + 1], points[i + 2], points[i + 3]]);
        i += 3;
    }
    segs
}

fn bezier_at(s: &[(f64, f64); 4], t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    let (a, b, c, d) = (mt.powi(3), 3.0 * mt * mt * t, 3.0 * mt * t * t, t.powi(3));
    (
        s[0].0 * a + s[1].0 * b + s[2].0 * c + s[3].0 * d,
        s[0].1 * a + s[1].1 * b + s[2].1 * c + s[3].1 * d,
    )
}

fn bezier_lookup(points: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
    bezier_segments(points)
        .iter()
        .map(|s| {
            (0..=100)
                .map(|i| bezier_at(s, f64::from(i) * 0.01))
                .collect()
        })
        .collect()
}

/// Gremlin's CubicBezierSpline.__call__: segment by knot x, then the lookup table.
fn bezier(segments: &[Vec<(f64, f64)>], x: f64) -> f64 {
    let Some(first) = segments.first() else {
        return x;
    };
    let index = if x < first[0].0 {
        0
    } else {
        segments
            .iter()
            .position(|s| s[0].0 <= x && x <= s[s.len() - 1].0)
            .unwrap_or(segments.len() - 1)
    };
    let s = &segments[index];
    let (mut lo, mut hi) = (0, s.len() - 1);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if s[mid].0 < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (a, b) = (s[lo], s[hi]);
    let dx = b.0 - a.0;
    let y = if dx.abs() < MIN_SEGMENT {
        a.1
    } else {
        a.1 + (x - a.0) * (b.1 - a.1) / dx
    };
    y.clamp(-1.0, 1.0)
}

/// Editable points: knots, plus for Bezier the handles beside each knot. Edits return the
/// points as the profile stores them (4 decimals), so a curve kept monotonic stays so once saved.
pub struct Editable;

fn stored(mut p: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let round = |v: f64| (v * 10_000.0).round() / 10_000.0;
    for q in &mut p {
        *q = (round(q.0), round(q.1));
    }
    p
}

impl Editable {
    /// Indices of the knots (centers) in the stored list.
    pub fn knots(kind: CurveType, len: usize) -> Vec<usize> {
        match kind {
            CurveType::CubicBezier => (0..len).step_by(3).collect(),
            _ => (0..len).collect(),
        }
    }

    /// Mirror (through the origin) of stored point `i` in a symmetric curve.
    pub fn mirror(len: usize, i: usize) -> usize {
        len - 1 - i
    }

    /// Mirrors the first half onto the second and puts a knot at the origin (Gremlin's
    /// `_enforce_symmetry`). Returns the new points.
    pub fn symmetric(kind: CurveType, points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let mut p = points.to_vec();
        match kind {
            CurveType::CubicBezier => {
                let mut knots: Vec<[(f64, f64); 3]> = Vec::new();
                // (left, center, right); end knots get a mirrored handle where missing.
                let k = Self::knots(kind, p.len());
                for (n, &c) in k.iter().enumerate() {
                    let left = if n == 0 { p[c] } else { p[c - 1] };
                    let right = if n == k.len() - 1 { p[c] } else { p[c + 1] };
                    knots.push([left, p[c], right]);
                }
                let count = knots.len();
                for i in 0..count / 2 {
                    let [l, c, r] = knots[i];
                    knots[count - 1 - i] = [(-r.0, -r.1), (-c.0, -c.1), (-l.0, -l.1)];
                }
                if count % 2 == 1 {
                    let mid = count / 2;
                    let r = knots[mid][2];
                    let (dx, dy) = (r.0 - knots[mid][1].0, r.1 - knots[mid][1].1);
                    knots[mid] = [(-dx, -dy), (0.0, 0.0), (dx, dy)];
                } else {
                    knots.insert(count / 2, [(-0.05, 0.0), (0.0, 0.0), (0.05, 0.0)]);
                }
                p.clear();
                let last = knots.len() - 1;
                for (n, [l, c, r]) in knots.into_iter().enumerate() {
                    if n > 0 {
                        p.push(l);
                    }
                    p.push(c);
                    if n < last {
                        p.push(r);
                    }
                }
            }
            _ => {
                p.sort_by(|a, b| a.0.total_cmp(&b.0));
                let count = p.len();
                for i in 0..count / 2 {
                    p[count - 1 - i] = (-p[i].0, -p[i].1);
                }
                if count % 2 == 1 {
                    p[count / 2] = (0.0, 0.0);
                } else {
                    p.insert(count / 2, (0.0, 0.0));
                }
            }
        }
        stored(p)
    }

    /// Adds a knot at (x, y) (and its mirror when symmetric); returns the new points and
    /// the stored index of the added knot.
    pub fn add(
        kind: CurveType,
        points: &[(f64, f64)],
        x: f64,
        y: f64,
        symmetric: bool,
    ) -> (Vec<(f64, f64)>, usize) {
        let mut p = points.to_vec();
        let insert = |p: &mut Vec<(f64, f64)>, x: f64, y: f64| -> usize {
            match kind {
                CurveType::CubicBezier => {
                    let k = Self::knots(kind, p.len());
                    let after = k.iter().rposition(|&c| p[c].0 < x).unwrap_or(0);
                    let at = k[after] + 2; // after the right handle of the knot before
                    p.splice(at..at, [(x - 0.05, y), (x, y), (x + 0.05, y)]);
                    at + 1
                }
                _ => {
                    let at = p.iter().position(|q| q.0 > x).unwrap_or(p.len());
                    p.insert(at, (x, y));
                    at
                }
            }
        };
        let mut idx = insert(&mut p, x, y);
        if symmetric && x.abs() > 1e-6 {
            let m = insert(&mut p, -x, -y);
            if m <= idx {
                idx += if kind == CurveType::CubicBezier { 3 } else { 1 };
            }
        }
        (stored(p), idx)
    }

    /// Removes knot at stored index `i` (and its mirror); end knots and the symmetric
    /// center stay.
    pub fn remove(
        kind: CurveType,
        points: &[(f64, f64)],
        i: usize,
        symmetric: bool,
    ) -> Vec<(f64, f64)> {
        let mut p = points.to_vec();
        let k = Self::knots(kind, p.len());
        let Some(n) = k.iter().position(|&c| c == i) else {
            return p;
        };
        if n == 0 || n == k.len() - 1 || (symmetric && n == k.len() / 2 && k.len() % 2 == 1) {
            return p;
        }
        let mut drop = vec![n];
        if symmetric {
            drop.push(k.len() - 1 - n);
        }
        drop.sort_unstable();
        drop.dedup();
        for &n in drop.iter().rev() {
            let c = k[n];
            match kind {
                CurveType::CubicBezier => {
                    p.drain(c - 1..=c + 1);
                }
                _ => {
                    p.remove(c);
                }
            }
        }
        p
    }

    /// Moves stored point `i` to (x, y). A knot drags its handles along; in symmetric mode
    /// the mirrored point moves the opposite way and the center knot stays at the origin.
    /// Knots keep their x order and the end knots stay at x = -1 / 1.
    pub fn set(
        kind: CurveType,
        points: &[(f64, f64)],
        i: usize,
        x: f64,
        y: f64,
        symmetric: bool,
    ) -> Vec<(f64, f64)> {
        let mut p = points.to_vec();
        let mirror = Self::mirror(p.len(), i);
        if i >= p.len() || (symmetric && mirror == i) {
            return p;
        }
        let k = Self::knots(kind, p.len());
        let bezier = kind == CurveType::CubicBezier;
        // Moves stored point `j` by (dx, dy), with its handles when it is a Bezier knot.
        let shift = |p: &mut Vec<(f64, f64)>, j: usize, dx: f64, dy: f64| {
            p[j] = (p[j].0 + dx, p[j].1 + dy);
            if bezier && k.contains(&j) {
                for h in [j.wrapping_sub(1), j + 1] {
                    if h < p.len() {
                        p[h] = (p[h].0 + dx, p[h].1 + dy);
                    }
                }
            }
        };
        let (mut x, y) = (x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0));
        if let Some(n) = k.iter().position(|&c| c == i) {
            x = if n == 0 {
                -1.0
            } else if n == k.len() - 1 {
                1.0
            } else {
                let (lo, hi) = (p[k[n - 1]].0 + 0.001, p[k[n + 1]].0 - 0.001);
                x.clamp(lo.min(hi), hi.max(lo))
            };
        }
        let (dx, dy) = (x - p[i].0, y - p[i].1);
        shift(&mut p, i, dx, dy);
        if symmetric {
            let (dx, dy) = (-p[i].0 - p[mirror].0, -p[i].1 - p[mirror].1);
            shift(&mut p, mirror, dx, dy);
        }
        stored(p)
    }

    /// [`Editable::set`] that keeps a monotonic curve monotonic: the point stops where the
    /// curve would start to turn back, and slides along that limit toward the pointer instead
    /// of sticking there. A curve that is not monotonic already is
    /// edited freely.
    pub fn set_monotonic(
        kind: CurveType,
        points: &[(f64, f64)],
        i: usize,
        x: f64,
        y: f64,
        symmetric: bool,
    ) -> Vec<(f64, f64)> {
        let new = Self::set(kind, points, i, x, y, symmetric);
        if i >= points.len() || monotonic(kind, &new) || !monotonic(kind, points) {
            return new;
        }
        let from = points[i];
        // Bisects the move from `from` to (tx, ty) for the farthest point that keeps the
        // curve monotonic.
        let farthest = |tx: f64, ty: f64| {
            let at = |t: f64| {
                let (px, py) = (from.0 + (tx - from.0) * t, from.1 + (ty - from.1) * t);
                Self::set(kind, points, i, px, py, symmetric)
            };
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..16 {
                let mid = (lo + hi) / 2.0;
                if monotonic(kind, &at(mid)) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            at(lo)
        };
        // Straight to the pointer, or turned up to 80 degrees either way (along the limit):
        // the result that ends nearest the pointer wins.
        let (dx, dy) = (x - from.0, y - from.1);
        let off = |p: &[(f64, f64)]| (p[i].0 - x).hypot(p[i].1 - y);
        [0.0f64, 20.0, -20.0, 40.0, -40.0, 60.0, -60.0, 80.0, -80.0]
            .into_iter()
            .map(|deg| {
                let (sin, cos) = deg.to_radians().sin_cos();
                farthest(from.0 + dx * cos - dy * sin, from.1 + dx * sin + dy * cos)
            })
            .min_by(|a, b| off(a).total_cmp(&off(b)))
            .unwrap_or_else(|| points.to_vec())
    }

    /// [`Editable::add`] that keeps a monotonic curve monotonic: a knot that would make it turn
    /// back goes onto the curve at that x instead, or is not added (`None`).
    pub fn add_monotonic(
        kind: CurveType,
        points: &[(f64, f64)],
        x: f64,
        y: f64,
        symmetric: bool,
    ) -> Option<(Vec<(f64, f64)>, usize)> {
        let added = Self::add(kind, points, x, y, symmetric);
        if monotonic(kind, &added.0) || !monotonic(kind, points) {
            return Some(added);
        }
        let on_curve = Curve::new(kind, points).eval(x);
        let added = Self::add(kind, points, x, on_curve, symmetric);
        monotonic(kind, &added.0).then_some(added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Symmetric knots at (+-0.75, +-0.25) or (+-0.25, +-0.75): the natural cubic spline turns
    /// back or runs past 1 (clamped flat); the monotone cubic stays between its knots.
    #[test]
    fn monotone_cubic_never_overshoots() {
        for (a, b) in [(0.75, 0.25), (0.25, 0.75)] {
            let p = [(-1.0, -1.0), (-a, -b), (0.0, 0.0), (a, b), (1.0, 1.0)];
            let c = Curve::new(CurveType::MonotoneCubic, &p);
            assert!(c.monotonic(), "{p:?}");
            for w in p.windows(2) {
                for i in 1..20 {
                    let x = w[0].0 + (w[1].0 - w[0].0) * f64::from(i) / 20.0;
                    let y = c.eval(x);
                    assert!(y > w[0].1 && y < w[1].1, "{p:?} at {x}: {y}");
                }
            }
            for &(x, y) in &p {
                assert!((c.eval(x) - y).abs() < 1e-9, "through the knot {x}");
            }
        }
        let spline = |a: f64, b: f64| {
            let p = [(-1.0, -1.0), (-a, -b), (0.0, 0.0), (a, b), (1.0, 1.0)];
            Curve::new(CurveType::CubicSpline, &p)
        };
        assert!(
            !spline(0.75, 0.25).monotonic(),
            "the natural spline turns back"
        );
        assert_eq!(
            spline(0.25, 0.75).eval(0.9),
            1.0,
            "and is clamped flat before x = 1"
        );
    }

    #[test]
    fn identity_for_every_type() {
        for kind in CurveType::ALL {
            let c = Curve::new(kind, &kind.default_points());
            for x in [-1.0, -0.4, 0.0, 0.3, 1.0] {
                assert!(
                    (c.eval(x) - x).abs() < 0.02,
                    "{kind:?} at {x}: {}",
                    c.eval(x)
                );
            }
        }
    }

    #[test]
    fn cubic_passes_through_knots() {
        let p = [(-1.0, -1.0), (0.0, 0.5), (1.0, 1.0)];
        let c = Curve::new(CurveType::CubicSpline, &p);
        assert!((c.eval(0.0) - 0.5).abs() < 1e-9);
        assert!(c.eval(-0.5) > -0.5, "bends above the line like the knot");
    }

    #[test]
    fn symmetric_linear_mirrors_and_centers() {
        let p = [(-1.0, -0.8), (-0.5, -0.2), (1.0, 1.0)];
        let s = Editable::symmetric(CurveType::PiecewiseLinear, &p);
        assert_eq!(s, [(-1.0, -0.8), (0.0, 0.0), (1.0, 0.8)]);
        let moved = Editable::set(CurveType::PiecewiseLinear, &s, 0, -1.0, -0.6, true);
        assert_eq!(moved[2], (1.0, 0.6));
        let (added, i) = Editable::add(CurveType::PiecewiseLinear, &moved, 0.5, 0.3, true);
        assert_eq!(added.len(), 5);
        assert_eq!(added[i], (0.5, 0.3));
        assert_eq!(added[1], (-0.5, -0.3));
        let removed = Editable::remove(CurveType::PiecewiseLinear, &added, 1, true);
        assert_eq!(removed.len(), 3);
    }

    #[test]
    fn bezier_add_and_symmetry_keep_the_layout() {
        let kind = CurveType::CubicBezier;
        let (p, i) = Editable::add(kind, &kind.default_points(), 0.2, 0.4, false);
        assert_eq!(p.len(), 7);
        assert_eq!(p[i], (0.2, 0.4));
        let s = Editable::symmetric(kind, &p);
        assert_eq!(s.len() % 3, 1);
        let k = Editable::knots(kind, s.len());
        assert_eq!(s[k[k.len() / 2]], (0.0, 0.0));
        let c = Curve::new(kind, &s);
        assert!(c.eval(0.0).abs() < 0.02);
    }

    #[test]
    fn monotonic_edits_stop_before_the_curve_turns_back() {
        for kind in CurveType::ALL {
            let (p, _) = Editable::add(kind, &kind.default_points(), 0.0, 0.0, false);
            assert!(Curve::new(kind, &p).monotonic(), "{kind:?}");
            // Pull the right end below zero: the curve would go up and then down.
            let last = p.len() - 1;
            let free = Editable::set(kind, &p, last, 1.0, -0.5, false);
            assert!(!Curve::new(kind, &free).monotonic(), "{kind:?}");
            let kept = Editable::set_monotonic(kind, &p, last, 1.0, -0.5, false);
            assert!(Curve::new(kind, &kept).monotonic(), "{kind:?}");
            assert!(kept[last].1 < 0.9, "{kind:?}: moved toward the target");
            assert!(kept[last].1 > -0.5, "{kind:?}: stopped before turning back");
        }
        // A curve that turns back already is edited freely.
        let wavy = [(-1.0, -1.0), (0.0, 0.8), (1.0, -0.5)];
        assert!(!Curve::new(CurveType::PiecewiseLinear, &wavy).monotonic());
        let moved = Editable::set_monotonic(CurveType::PiecewiseLinear, &wavy, 2, 1.0, -0.9, false);
        assert_eq!(moved[2], (1.0, -0.9));
        // Inverted curves (falling) count as monotonic.
        let falling = [(-1.0, 1.0), (1.0, -1.0)];
        assert!(Curve::new(CurveType::CubicSpline, &falling).monotonic());
    }

    /// At the limit the point slides along it instead of sticking. Symmetric linear curve: the
    /// knot at x = 0.5 must stay at y >= 0 (the center knot).
    #[test]
    fn monotonic_edits_slide_along_the_limit() {
        let kind = CurveType::PiecewiseLinear;
        let p = Editable::symmetric(kind, &kind.default_points());
        let (p, i) = Editable::add(kind, &p, 0.5, 0.5, true);
        // Straight there it would stop at y = 0 on the way; then it follows the pointer
        // along the limit, also when every step straight to the pointer is forbidden.
        let mut moved = p.clone();
        for _ in 0..20 {
            moved = Editable::set_monotonic(kind, &moved, i, 0.8, -1.0, true);
            assert!(Curve::new(kind, &moved).monotonic());
        }
        let (x, y) = moved[i];
        assert!((x - 0.8).abs() < 0.01 && y.abs() < 0.01, "({x}, {y})");
    }

    /// Fast drags stop the point right at the limit; the profile keeps 4 decimals, and the
    /// rounded points must not end up past the limit (the curve would then be edited freely).
    #[test]
    fn monotonic_edits_survive_rounding() {
        let round = |p: Vec<(f64, f64)>| -> Vec<(f64, f64)> {
            let r = |v: f64| (v * 10_000.0).round() / 10_000.0;
            p.into_iter().map(|(x, y)| (r(x), r(y))).collect()
        };
        for kind in [CurveType::CubicSpline, CurveType::CubicBezier] {
            for (tx, ty) in [(0.32, -0.41), (0.7, -0.9), (0.2, -1.0), (0.9, 0.05)] {
                let p = Editable::symmetric(kind, &kind.default_points());
                let (mut p, i) = Editable::add(kind, &p, 0.5, 0.5, true);
                let i = if kind == CurveType::CubicBezier {
                    i - 1
                } else {
                    i
                };
                for step in 1..=8 {
                    let t = f64::from(step) / 8.0;
                    let (x, y) = (0.5 + (tx - 0.5) * t, 0.5 + (ty - 0.5) * t);
                    p = round(Editable::set_monotonic(kind, &p, i, x, y, true));
                    assert!(Curve::new(kind, &p).monotonic(), "{kind:?} to ({tx}, {ty})");
                }
            }
        }
    }

    /// The center handle of a symmetric Bezier pulled back past the center (the curve would
    /// dip and jump there) stops before that.
    #[test]
    fn bezier_center_handle_stops() {
        let kind = CurveType::CubicBezier;
        let p = Editable::symmetric(kind, &kind.default_points());
        let right = p.len() / 2 + 1;
        assert_eq!(p[right - 1], (0.0, 0.0), "center knot");
        let free = Editable::set(kind, &p, right, 0.41, -0.55, true);
        assert!(!Curve::new(kind, &free).monotonic());
        let kept = Editable::set_monotonic(kind, &p, right, 0.41, -0.55, true);
        assert!(Curve::new(kind, &kept).monotonic());
    }

    /// A Bezier handle that makes the curve loop back in x is caught, although y(x) read from
    /// the curve looks fine.
    #[test]
    fn bezier_loops_are_not_monotonic() {
        let ok = [(-1.0, -1.0), (-0.5, -1.0), (0.5, 1.0), (1.0, 1.0)];
        assert!(Curve::new(CurveType::CubicBezier, &ok).monotonic());
        let looped = [(-1.0, -1.0), (1.6, -1.0), (-0.6, 1.0), (1.0, 1.0)];
        assert!(!Curve::new(CurveType::CubicBezier, &looped).monotonic());
    }

    #[test]
    fn monotonic_add_puts_the_knot_on_the_curve() {
        let kind = CurveType::PiecewiseLinear;
        let p = kind.default_points();
        let (added, i) = Editable::add_monotonic(
            kind,
            &[(-1.0, -1.0), (0.5, 0.5), (1.0, 1.0)],
            0.8,
            -0.5,
            false,
        )
        .unwrap();
        assert!(Curve::new(kind, &added).monotonic());
        assert!((added[i].1 - 0.8).abs() < 1e-9);
        let (added, i) = Editable::add_monotonic(kind, &p, 0.5, 0.2, false).unwrap();
        assert_eq!(added[i], (0.5, 0.2));
    }
}
