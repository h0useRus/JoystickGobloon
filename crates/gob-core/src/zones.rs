//! Axis zones (`axis-zones`, Joystick Gobloon's own action): points split an axis into zones, and
//! each zone runs its own actions with the value stretched to the full travel, as it is, or a
//! fixed value. For detents (afterburner, reverse), a throttle with a gate, or a different curve
//! on each stretch of the travel.
//!
//! ```xml
//! <action id="..." type="axis-zones">
//!     <property type="string"><name>points</name><value>-0.5 0.8</value></property>
//!     <zone-0><property type="string"><name>output</name><value>rescale</value></property>
//!         <action-id>...</action-id></zone-0>
//!     <zone-1>...</zone-1>
//!     <zone-2><property type="string"><name>output</name><value>fixed</value></property>
//!         <property type="float"><name>value</name><value>1.0</value></property></zone-2>
//! </action>
//! ```

use crate::action::{Action, Item, Property};
use crate::profile::Profile;

/// Action kind name.
pub const AXIS_ZONES: &str = "axis-zones";

/// Most points (zones = points + 1).
pub const MAX_POINTS: usize = 7;

/// Closest two points may be, so every zone keeps some travel.
pub const MIN_GAP: f64 = 0.01;

/// What a zone passes to its actions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoneOutput {
    /// The zone's stretch of travel, stretched to -1..1.
    Rescale,
    /// The axis value as it is.
    Pass,
    /// This value, wherever the axis is in the zone.
    Fixed(f64),
}

impl ZoneOutput {
    pub fn key(self) -> &'static str {
        match self {
            Self::Rescale => "rescale",
            Self::Pass => "pass",
            Self::Fixed(_) => "fixed",
        }
    }
}

/// One zone: what it outputs and the actions it runs.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisZone<'a> {
    pub output: ZoneOutput,
    pub children: Vec<&'a str>,
}

/// `points` property: numbers in -1..1, sorted, at least [`MIN_GAP`] apart, [`MAX_POINTS`] at most.
pub fn parse_points(s: &str) -> Vec<f64> {
    let mut points: Vec<f64> = s
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter_map(|t| t.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(-1.0, 1.0))
        .collect();
    points.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for p in points {
        if out.last().is_none_or(|&l| p - l >= MIN_GAP) && out.len() < MAX_POINTS {
            out.push(p);
        }
    }
    out
}

fn format_points(points: &[f64]) -> String {
    points
        .iter()
        .map(|p| format!("{p:.3}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The zones of an axis-zones action: one more than its points, in order; a zone the file lacks
/// stretches to the full travel and runs nothing.
pub fn zones(action: &Action) -> (Vec<f64>, Vec<AxisZone<'_>>) {
    let points = parse_points(action.str("points").unwrap_or_default());
    let zones = (0..=points.len())
        .map(|n| {
            let Some(items) = action.group(&group_tag(n)) else {
                return AxisZone {
                    output: ZoneOutput::Rescale,
                    children: Vec::new(),
                };
            };
            let prop = |name: &str| {
                items.iter().find_map(|i| match i {
                    Item::Property(p) if p.name == name => Some(p.value.trim()),
                    _ => None,
                })
            };
            let output = match prop("output") {
                Some("pass") => ZoneOutput::Pass,
                Some("fixed") => ZoneOutput::Fixed(
                    prop("value")
                        .and_then(|v| v.parse::<f64>().ok())
                        .filter(|v| v.is_finite())
                        .unwrap_or(0.0)
                        .clamp(-1.0, 1.0),
                ),
                _ => ZoneOutput::Rescale,
            };
            let children = items
                .iter()
                .filter_map(|i| match i {
                    Item::ActionRef(id) => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            AxisZone { output, children }
        })
        .collect();
    (points, zones)
}

/// Child group of zone `n`.
pub fn group_tag(n: usize) -> String {
    format!("zone-{n}")
}

/// The zone `v` is in: the number of points at or below it.
pub fn zone_of(points: &[f64], v: f64) -> usize {
    points.iter().take_while(|&&p| p <= v).count()
}

/// The value zone `n` passes on for the axis at `v`.
pub fn output(points: &[f64], n: usize, output: ZoneOutput, v: f64) -> f64 {
    match output {
        ZoneOutput::Pass => v,
        ZoneOutput::Fixed(x) => x,
        ZoneOutput::Rescale => {
            let lo = if n == 0 { -1.0 } else { points[n - 1] };
            let hi = points.get(n).copied().unwrap_or(1.0);
            if hi <= lo {
                return 0.0;
            }
            ((v - lo) / (hi - lo) * 2.0 - 1.0).clamp(-1.0, 1.0)
        }
    }
}

fn zone_props(output: ZoneOutput) -> Vec<Item> {
    let mut items = vec![Item::Property(Property {
        ty: "string".into(),
        name: "output".into(),
        value: output.key().into(),
    })];
    if let ZoneOutput::Fixed(v) = output {
        items.push(Item::Property(Property {
            ty: "float".into(),
            name: "value".into(),
            value: format!("{v:?}"),
        }));
    }
    items
}

/// Items of a new axis-zones action: one point (a detent at 0.8), both zones stretched.
pub fn new_items() -> Vec<Item> {
    vec![
        Item::Property(Property {
            ty: "string".into(),
            name: "points".into(),
            value: "0.800".into(),
        }),
        Item::Group {
            tag: group_tag(0),
            attrs: Vec::new(),
            items: zone_props(ZoneOutput::Rescale),
        },
        Item::Group {
            tag: group_tag(1),
            attrs: Vec::new(),
            items: zone_props(ZoneOutput::Rescale),
        },
    ]
}

impl Profile {
    /// Adds a point halfway between the last one and the top (a new zone at the top, stretched).
    pub fn add_zone_point(&mut self, id: &str) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let mut points = parse_points(a.str("points").unwrap_or_default());
        if points.len() >= MAX_POINTS {
            return;
        }
        let last = points.last().copied().unwrap_or(-1.0);
        if 1.0 - last < MIN_GAP * 2.0 {
            return;
        }
        points.push((last + 1.0) / 2.0);
        a.set("points", "string", format_points(&points));
        let tag = group_tag(points.len());
        a.group_mut(&tag).clear();
        a.group_mut(&tag).extend(zone_props(ZoneOutput::Rescale));
    }

    /// Removes point `index`: the zone above it goes, with its actions; the zones above that move
    /// down one.
    pub fn remove_zone_point(&mut self, id: &str, index: usize) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let mut points = parse_points(a.str("points").unwrap_or_default());
        if index >= points.len() {
            return;
        }
        points.remove(index);
        a.set("points", "string", format_points(&points));
        let gone = group_tag(index + 1);
        a.items
            .retain(|i| !matches!(i, Item::Group { tag, .. } if *tag == gone));
        for n in index + 2..=points.len() + 1 {
            let from = group_tag(n);
            let to = group_tag(n - 1);
            for item in &mut a.items {
                if let Item::Group { tag, .. } = item
                    && *tag == from
                {
                    *tag = to.clone();
                }
            }
        }
        self.collect_garbage();
    }

    /// Moves point `index` to `value`, kept between its neighbours.
    pub fn set_zone_point(&mut self, id: &str, index: usize, value: f64) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let mut points = parse_points(a.str("points").unwrap_or_default());
        if index >= points.len() {
            return;
        }
        let lo = if index == 0 {
            -1.0
        } else {
            points[index - 1] + MIN_GAP
        };
        let hi = points.get(index + 1).map_or(1.0, |p| p - MIN_GAP);
        points[index] = value.clamp(lo, hi.max(lo));
        a.set("points", "string", format_points(&points));
    }

    /// Sets what zone `n` passes to its actions; its actions stay.
    pub fn set_zone_output(&mut self, id: &str, n: usize, output: ZoneOutput) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let group = a.group_mut(&group_tag(n));
        group
            .retain(|i| !matches!(i, Item::Property(p) if p.name == "output" || p.name == "value"));
        let props = zone_props(output);
        for (k, item) in props.into_iter().enumerate() {
            group.insert(k, item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_are_cleaned() {
        assert_eq!(parse_points("0.9, -0.5 0.905 2 x"), vec![-0.5, 0.9, 1.0]);
        assert_eq!(parse_points(""), Vec::<f64>::new());
        assert_eq!(
            parse_points("0 0.1 0.2 0.3 0.4 0.5 0.6 0.7 0.8").len(),
            MAX_POINTS
        );
    }

    #[test]
    fn points_added_moved_and_removed_keep_the_other_zones() {
        use crate::edit::NewAction;
        let mut p = Profile::default();
        let root = p.bind("dev", "axis", "1", "Default");
        let z = p
            .add_child(&root, "actions", Action::new(&NewAction::AxisZones))
            .unwrap();
        p.add_zone_point(&z);
        p.add_zone_point(&z);
        let (points, zones) = super::zones(p.action(&z).unwrap());
        assert_eq!(points, vec![0.8, 0.9, 0.95]);
        assert_eq!(zones.len(), 4);
        // An action in the top zone, a fixed middle zone.
        let top = p
            .add_child(&z, &group_tag(3), Action::new(&NewAction::Description))
            .unwrap();
        p.set_zone_output(&z, 2, ZoneOutput::Fixed(0.25));
        p.set_zone_point(&z, 0, 2.0);
        assert_eq!(super::zones(p.action(&z).unwrap()).0[0], 0.89);
        p.remove_zone_point(&z, 0);
        let (points, zones) = super::zones(p.action(&z).unwrap());
        assert_eq!(points, vec![0.9, 0.95]);
        assert_eq!(zones[1].output, ZoneOutput::Fixed(0.25));
        assert_eq!(zones[2].children, vec![top.as_str()]);
    }

    #[test]
    fn zones_and_outputs() {
        let points = [-0.5, 0.8];
        assert_eq!(zone_of(&points, -1.0), 0);
        assert_eq!(zone_of(&points, -0.5), 1);
        assert_eq!(zone_of(&points, 0.79), 1);
        assert_eq!(zone_of(&points, 1.0), 2);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(output(&points, 0, ZoneOutput::Rescale, -1.0), -1.0));
        assert!(close(output(&points, 0, ZoneOutput::Rescale, -0.75), 0.0));
        assert!(close(output(&points, 1, ZoneOutput::Rescale, 0.15), 0.0));
        assert!(close(output(&points, 2, ZoneOutput::Rescale, 1.0), 1.0));
        assert!(close(output(&points, 2, ZoneOutput::Rescale, 0.9), 0.0));
        assert!(close(output(&points, 1, ZoneOutput::Pass, 0.3), 0.3));
        assert!(close(output(&points, 2, ZoneOutput::Fixed(0.5), 0.9), 0.5));
    }
}
