//! Typed view of the `<condition>` entries of a Gremlin condition action.
//!
//! Each condition has a source (what is read) and a comparator (what it must be).
//! Unknown sources (logical devices) are kept as [`Source::Other`] and never rewritten.

use crate::action::{InputType, Item, Key, Property};
use crate::profile::{hat_direction, hat_direction_name};

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// The value of the input the action is bound to.
    CurrentInput,
    /// Physical inputs: (device GUID, type, id). All must satisfy the comparator.
    Joystick(Vec<(String, InputType, u32)>),
    /// Keys held at the same time.
    Keyboard(Vec<Key>),
    /// The last value Joystick Gobloon sent to a vJoy input.
    Vjoy {
        device: u32,
        input_type: InputType,
        id: u32,
    },
    /// A logical device input.
    Logical { input_type: InputType, id: u32 },
    /// Anything newer; evaluates to false.
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Comparator {
    /// Axis value within `lower..=upper`.
    Range { lower: f64, upper: f64 },
    /// Every button (or key) is in this state.
    Pressed(bool),
    /// Hat points in one of these directions (`None` = center).
    Direction(Vec<Option<u16>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub source: Source,
    pub comparator: Comparator,
}

impl Condition {
    /// The comparator Gremlin creates for an input type.
    pub fn default_comparator(input_type: InputType) -> Comparator {
        match input_type {
            InputType::Axis => Comparator::Range {
                lower: -1.0,
                upper: 1.0,
            },
            InputType::Hat => Comparator::Direction(Vec::new()),
            _ => Comparator::Pressed(true),
        }
    }

    pub fn parse(items: &[Item]) -> Self {
        let kind = prop(items, "condition-type").unwrap_or_default();
        let comparator = group(items, "comparator").map_or(Comparator::Pressed(true), |c| {
            match prop(c, "comparator-type").unwrap_or_default() {
                "range" => {
                    let mut lower = num(c, "lower-limit", -1.0);
                    let mut upper = num(c, "upper-limit", 1.0);
                    if lower > upper {
                        std::mem::swap(&mut lower, &mut upper);
                    }
                    Comparator::Range { lower, upper }
                }
                "direction" => Comparator::Direction(
                    props(c, "direction")
                        .filter_map(|d| hat_direction(&d.to_ascii_lowercase()))
                        .collect(),
                ),
                _ => Comparator::Pressed(flag(c, "is-pressed", true)),
            }
        });
        let inputs = || {
            items.iter().filter_map(|i| match i {
                Item::Group { tag, items, .. } if tag == "input" => Some(items.as_slice()),
                _ => None,
            })
        };
        let source = match kind {
            "current_input" => Source::CurrentInput,
            "joystick" => Source::Joystick(
                inputs()
                    .map(|g| {
                        (
                            prop(g, "device-guid").unwrap_or_default().to_string(),
                            InputType::parse(prop(g, "input-type").unwrap_or_default()),
                            num(g, "input-id", 0.0) as u32,
                        )
                    })
                    .collect(),
            ),
            "keyboard" => Source::Keyboard(
                inputs()
                    .map(|g| Key {
                        scan_code: num(g, "scan-code", 0.0) as u16,
                        extended: flag(g, "is-extended", false),
                    })
                    .collect(),
            ),
            "vjoy" => Source::Vjoy {
                device: num(items, "vjoy-id", 1.0) as u32,
                input_type: InputType::parse(prop(items, "input-type").unwrap_or_default()),
                id: num(items, "input-id", 1.0) as u32,
            },
            "logical_device" => Source::Logical {
                input_type: InputType::parse(prop(items, "input-type").unwrap_or_default()),
                id: num(items, "input-id", 1.0) as u32,
            },
            other => Source::Other(other.to_string()),
        };
        Self { source, comparator }
    }

    /// Items of the `<condition>` element in Gremlin's order.
    pub fn to_items(&self) -> Vec<Item> {
        let kind = match &self.source {
            Source::CurrentInput => "current_input",
            Source::Joystick(_) => "joystick",
            Source::Keyboard(_) => "keyboard",
            Source::Vjoy { .. } => "vjoy",
            Source::Logical { .. } => "logical_device",
            Source::Other(k) => k,
        };
        let mut items = vec![p("string", "condition-type", kind)];
        let mut cmp = Vec::new();
        match &self.comparator {
            Comparator::Range { lower, upper } => {
                cmp.push(p("string", "comparator-type", "range"));
                cmp.push(p("float", "lower-limit", &format!("{lower:?}")));
                cmp.push(p("float", "upper-limit", &format!("{upper:?}")));
            }
            Comparator::Pressed(b) => {
                cmp.push(p("string", "comparator-type", "pressed"));
                cmp.push(p("bool", "is-pressed", py_bool(*b)));
            }
            Comparator::Direction(dirs) => {
                cmp.push(p("string", "comparator-type", "direction"));
                for d in dirs {
                    cmp.push(p("hat_direction", "direction", hat_direction_name(*d)));
                }
            }
        }
        items.push(g("comparator", cmp));
        match &self.source {
            Source::Joystick(inputs) => {
                for (device, ty, id) in inputs {
                    items.push(g(
                        "input",
                        vec![
                            p("uuid", "device-guid", device),
                            p("input_type", "input-type", input_type_name(*ty)),
                            p("int", "input-id", &id.to_string()),
                        ],
                    ));
                }
            }
            Source::Keyboard(keys) => {
                for k in keys {
                    items.push(g(
                        "input",
                        vec![
                            p("int", "scan-code", &k.scan_code.to_string()),
                            p("bool", "is-extended", py_bool(k.extended)),
                        ],
                    ));
                }
            }
            Source::Vjoy {
                device,
                input_type,
                id,
            } => {
                items.push(p("int", "vjoy-id", &device.to_string()));
                items.push(p("input_type", "input-type", input_type_name(*input_type)));
                items.push(p("int", "input-id", &id.to_string()));
            }
            Source::Logical { input_type, id } => {
                items.push(p("input_type", "input-type", input_type_name(*input_type)));
                items.push(p("int", "input-id", &id.to_string()));
            }
            Source::CurrentInput | Source::Other(_) => {}
        }
        items
    }
}

fn input_type_name(t: InputType) -> &'static str {
    match t {
        InputType::Axis => "axis",
        InputType::Hat => "hat",
        _ => "button",
    }
}

fn py_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

fn p(ty: &str, name: &str, value: &str) -> Item {
    Item::Property(Property {
        ty: ty.into(),
        name: name.into(),
        value: value.into(),
    })
}

fn g(tag: &str, items: Vec<Item>) -> Item {
    Item::Group {
        tag: tag.into(),
        attrs: Vec::new(),
        items,
    }
}

fn props<'a>(items: &'a [Item], name: &'a str) -> impl Iterator<Item = String> + 'a {
    items.iter().filter_map(move |i| match i {
        Item::Property(p) if p.name == name => Some(p.value.trim().to_string()),
        _ => None,
    })
}

fn prop<'a>(items: &'a [Item], name: &str) -> Option<&'a str> {
    items.iter().find_map(|i| match i {
        Item::Property(p) if p.name == name => Some(p.value.trim()),
        _ => None,
    })
}

fn num(items: &[Item], name: &str, default: f64) -> f64 {
    prop(items, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn flag(items: &[Item], name: &str, default: bool) -> bool {
    prop(items, name).map_or(default, |v| v.eq_ignore_ascii_case("true"))
}

fn group<'a>(items: &'a [Item], tag: &str) -> Option<&'a [Item]> {
    items.iter().find_map(|i| match i {
        Item::Group { tag: t, items, .. } if t == tag => Some(items.as_slice()),
        _ => None,
    })
}
