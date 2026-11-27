use super::*;

#[test]
fn presets_are_symmetric_monotonic_and_end_at_the_corners() {
    for preset in CurvePreset::ALL {
        let (kind, points) = preset.curve();
        let curve = Curve::new(kind, &points);
        assert!(curve.monotonic(), "{preset:?}");
        assert_eq!(Editable::symmetric(kind, &points), points, "{preset:?}");
        assert!(
            (curve.eval(1.0) + curve.eval(-1.0)).abs() < 1e-9,
            "{preset:?}"
        );
        assert!(curve.eval(0.0).abs() < 1e-9, "{preset:?}");
        let full = !matches!(preset, CurvePreset::HalfRange | CurvePreset::Inverted);
        assert_eq!((curve.eval(1.0) - 1.0).abs() < 1e-9, full, "{preset:?}");
    }
    let soft = Curve::new(
        CurvePreset::SoftCenter.curve().0,
        &CurvePreset::SoftCenter.curve().1,
    );
    assert!(soft.eval(0.5) < 0.3, "calm around the center");
    let quick = CurvePreset::QuickCenter.curve();
    assert!(Curve::new(quick.0, &quick.1).eval(0.5) > 0.6);
}

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
