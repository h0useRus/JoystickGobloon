//! Actions as stored in a profile's `<library>`.
//!
//! An action is a generic tree of properties, groups and action references,
//! exactly as Gremlin writes it. [`Action::kind`] decodes the types JoyGob
//! understands into typed values; everything else stays untouched.

use crate::xml::Element;

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Gremlin value type: `string`, `int`, `float`, `bool`, `point2d`, ...
    pub ty: String,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Property(Property),
    /// `<action-id>` reference to another action in the library.
    ActionRef(String),
    /// Named container such as `<deadzone>`, `<short-actions>` or `<macro-action type="key">`.
    Group {
        tag: String,
        attrs: Vec<(String, String)>,
        items: Vec<Item>,
    },
    /// Anything that does not fit the shapes above, kept verbatim.
    Raw(Element),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub id: String,
    /// Gremlin action type, e.g. `map-to-vjoy`.
    pub kind: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationMode {
    Both,
    Press,
    Release,
    Disallowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Axis,
    Button,
    Hat,
    Other,
}

impl InputType {
    pub fn parse(s: &str) -> Self {
        match s {
            "axis" => Self::Axis,
            "button" => Self::Button,
            "hat" => Self::Hat,
            _ => Self::Other,
        }
    }
}

/// Typed view of the action types JoyGob can execute.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionKind<'a> {
    Root {
        children: Vec<&'a str>,
    },
    MapToVjoy {
        device: u32,
        input_type: InputType,
        input_id: u32,
        inverted: bool,
    },
    ChangeMode {
        change_type: &'a str,
        target: Option<&'a str>,
    },
    ResponseCurve {
        curve_type: &'a str,
        deadzone: [f64; 4],
        points: Vec<(f64, f64)>,
    },
    Tempo {
        short: Vec<&'a str>,
        long: Vec<&'a str>,
        threshold: f64,
        activate_on: &'a str,
    },
    Description {
        text: &'a str,
    },
    MapToMouse(MouseAction),
    /// Preserved in the profile but not executed (yet).
    Other,
}

/// Mouse buttons as Gremlin names them in profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Forward,
    Back,
    WheelUp,
    WheelDown,
}

impl MouseButton {
    pub const ALL: [MouseButton; 7] = [
        Self::Left,
        Self::Right,
        Self::Middle,
        Self::Forward,
        Self::Back,
        Self::WheelUp,
        Self::WheelDown,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Middle => "Middle",
            Self::Forward => "Forward",
            Self::Back => "Back",
            Self::WheelUp => "Wheel Up",
            Self::WheelDown => "Wheel Down",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.name() == s.trim())
    }
}

/// What a map-to-mouse action does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseAction {
    Button(MouseButton),
    /// Cursor motion: compass direction in degrees (0 = up, 90 = right), speeds in px/s.
    Motion {
        direction: f32,
        min_speed: f32,
        max_speed: f32,
        time_to_max_speed: f32,
    },
}

/// Action types decoded by [`Action::kind`].
pub const SUPPORTED_KINDS: &[&str] = &[
    "root",
    "map-to-vjoy",
    "change-mode",
    "response-curve",
    "tempo",
    "description",
    "map-to-mouse",
];

impl Action {
    pub fn property(&self, name: &str) -> Option<&Property> {
        find_property(&self.items, name)
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.property(name).map(|p| p.value.as_str())
    }

    pub fn label(&self) -> Option<&str> {
        self.str("action-label")
    }

    pub fn activation_mode(&self) -> ActivationMode {
        match self.str("activation-mode") {
            Some("press") => ActivationMode::Press,
            Some("release") => ActivationMode::Release,
            Some("disallowed") => ActivationMode::Disallowed,
            _ => ActivationMode::Both,
        }
    }

    pub fn group(&self, tag: &str) -> Option<&[Item]> {
        find_group(&self.items, tag)
    }

    /// Every action id referenced anywhere inside this action.
    pub fn child_ids(&self) -> Vec<&str> {
        let mut out = Vec::new();
        collect_refs(&self.items, &mut out);
        out
    }

    pub fn kind(&self) -> ActionKind<'_> {
        match self.kind.as_str() {
            "root" => ActionKind::Root {
                children: self.refs_in("actions"),
            },
            "map-to-vjoy" => ActionKind::MapToVjoy {
                device: self.parse("vjoy-device-id").unwrap_or(1),
                input_type: InputType::parse(self.str("vjoy-input-type").unwrap_or_default()),
                input_id: self.parse("vjoy-input-id").unwrap_or(1),
                inverted: self.bool("button-inverted"),
            },
            "change-mode" => ActionKind::ChangeMode {
                change_type: self.str("change-type").unwrap_or("Switch"),
                target: self
                    .group("target-mode")
                    .and_then(|g| find_property(g, "name"))
                    .map(|p| p.value.as_str()),
            },
            "response-curve" => {
                let dz = |n, d| {
                    self.group("deadzone")
                        .and_then(|g| find_property(g, n))
                        .and_then(|p| p.value.parse().ok())
                        .unwrap_or(d)
                };
                let points = self
                    .group("control-points")
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|i| match i {
                        Item::Property(p) => parse_point(&p.value),
                        _ => None,
                    })
                    .collect();
                ActionKind::ResponseCurve {
                    curve_type: self.str("curve-type").unwrap_or("PiecewiseLinear"),
                    deadzone: [
                        dz("low", -1.0),
                        dz("center-low", 0.0),
                        dz("center-high", 0.0),
                        dz("high", 1.0),
                    ],
                    points,
                }
            }
            "tempo" => ActionKind::Tempo {
                short: self.refs_in("short-actions"),
                long: self.refs_in("long-actions"),
                threshold: self.parse("threshold").unwrap_or(0.5),
                activate_on: self.str("activate-on").unwrap_or("release"),
            },
            "description" => ActionKind::Description {
                text: self.str("description").unwrap_or_default(),
            },
            "map-to-mouse" => ActionKind::MapToMouse(match self.str("mode").map(str::trim) {
                Some("Motion") => MouseAction::Motion {
                    direction: self.parse("direction").unwrap_or(0.0),
                    min_speed: self.parse("min-speed").unwrap_or(50.0),
                    max_speed: self.parse("max-speed").unwrap_or(250.0),
                    time_to_max_speed: self.parse("time-to-max-speed").unwrap_or(1.0),
                },
                _ => MouseAction::Button(
                    self.str("button")
                        .and_then(MouseButton::parse)
                        .unwrap_or(MouseButton::Left),
                ),
            }),
            _ => ActionKind::Other,
        }
    }

    fn parse<T: std::str::FromStr>(&self, name: &str) -> Option<T> {
        self.str(name)?.trim().parse().ok()
    }

    fn bool(&self, name: &str) -> bool {
        self.str(name)
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    }

    fn refs_in(&self, tag: &str) -> Vec<&str> {
        let mut out = Vec::new();
        if let Some(items) = self.group(tag) {
            collect_refs(items, &mut out);
        }
        out
    }
}

fn find_property<'a>(items: &'a [Item], name: &str) -> Option<&'a Property> {
    items.iter().find_map(|i| match i {
        Item::Property(p) if p.name == name => Some(p),
        _ => None,
    })
}

fn find_group<'a>(items: &'a [Item], tag: &str) -> Option<&'a [Item]> {
    items.iter().find_map(|i| match i {
        Item::Group { tag: t, items, .. } if t == tag => Some(items.as_slice()),
        _ => None,
    })
}

fn collect_refs<'a>(items: &'a [Item], out: &mut Vec<&'a str>) {
    for item in items {
        match item {
            Item::ActionRef(id) => out.push(id),
            Item::Group { items, .. } => collect_refs(items, out),
            _ => {}
        }
    }
}

fn parse_point(s: &str) -> Option<(f64, f64)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}
