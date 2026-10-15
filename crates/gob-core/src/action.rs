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
    /// Each activation (press + release) runs the next sequence; `timeout` > 0 restarts
    /// from the first one after that many idle seconds.
    Chain {
        chains: Vec<Vec<&'a str>>,
        timeout: f64,
    },
    /// `activate_on`: `exclusive` (single waits for the threshold) or `combined`.
    DoubleTap {
        single: Vec<&'a str>,
        double: Vec<&'a str>,
        threshold: f64,
        activate_on: &'a str,
    },
    /// Each hat direction (and the center) acts as a button running its own actions.
    /// `directions` holds 4 or 8 `(group tag, direction, actions)` entries plus the center
    /// (`Center`, `None`), in Gremlin's order.
    HatButtons {
        directions: Vec<(&'static str, Option<u16>, Vec<&'a str>)>,
    },
    MapToMouse(MouseAction),
    /// Keys pressed in order on press and released in reverse on release.
    MapToKeyboard {
        keys: Vec<Key>,
    },
    /// Short press latches the children on, hold acts momentarily.
    SmartToggle {
        children: Vec<&'a str>,
        delay: f64,
    },
    /// Splits an axis at `split`; each half is rescaled to -1..1.
    SplitAxis {
        split: f64,
        lower: Vec<&'a str>,
        upper: Vec<&'a str>,
    },
    /// `Pause`, `Resume` or `Toggle` the whole profile.
    PauseResume {
        operation: &'a str,
    },
    RunCommand {
        executable: &'a str,
        arguments: &'a str,
    },
    /// Sequence of timed steps; `exclusive`/`preemptive` are kept but not executed.
    Macro {
        steps: Vec<MacroStep>,
        repeat: MacroRepeat,
        exclusive: bool,
        preemptive: bool,
    },
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

/// A keyboard key as Gremlin stores it: set 1 scan code plus the E0 prefix flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub scan_code: u16,
    pub extended: bool,
}

/// One `<macro-action>` of a macro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MacroStep {
    /// Seconds.
    Pause(f64),
    /// Set 1 scan code; `extended` adds the E0 prefix.
    Key {
        scan_code: u16,
        extended: bool,
        pressed: bool,
    },
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
    MouseMotion {
        dx: i32,
        dy: i32,
    },
    VjoyButton {
        device: u32,
        id: u32,
        pressed: bool,
    },
    /// `relative` adds `value` to the current position.
    VjoyAxis {
        device: u32,
        id: u32,
        value: f64,
        relative: bool,
    },
    /// Hundredths of a degree, `None` when centered.
    VjoyHat {
        device: u32,
        id: u32,
        direction: Option<u16>,
    },
    /// `joystick` and `logical-device` steps, not executed.
    Other,
}

/// How often a macro runs; delays are seconds between repetitions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MacroRepeat {
    Single,
    Count {
        count: u32,
        delay: f64,
    },
    /// Repeats while the input stays pressed.
    Hold {
        delay: f64,
    },
    /// Starts on one activation, stops on the next.
    Toggle {
        delay: f64,
    },
}

/// hat-buttons group tags, clockwise from north (45 degrees apart).
const HAT_BUTTONS: [&str; 8] = [
    "North",
    "North-East",
    "East",
    "South-East",
    "South",
    "South-West",
    "West",
    "North-West",
];

/// Action types decoded by [`Action::kind`].
pub const SUPPORTED_KINDS: &[&str] = &[
    "root",
    "map-to-vjoy",
    "change-mode",
    "response-curve",
    "tempo",
    "description",
    "map-to-mouse",
    "map-to-keyboard",
    "chain",
    "double-tap",
    "hat-buttons",
    "smart-toggle",
    "split-axis",
    "pause-resume",
    "run-command",
    "macro",
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
            "chain" => {
                let mut chains: Vec<(u32, Vec<&str>)> = self
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        Item::Group { tag, items, .. } => {
                            let n = tag.strip_prefix("chain-")?.parse().ok()?;
                            let mut refs = Vec::new();
                            collect_refs(items, &mut refs);
                            Some((n, refs))
                        }
                        _ => None,
                    })
                    .collect();
                chains.sort_by_key(|(n, _)| *n);
                ActionKind::Chain {
                    chains: chains.into_iter().map(|(_, c)| c).collect(),
                    timeout: self.parse("timeout").unwrap_or(0.0),
                }
            }
            "double-tap" => ActionKind::DoubleTap {
                single: self.refs_in("single-actions"),
                double: self.refs_in("double-actions"),
                threshold: self.parse("threshold").unwrap_or(0.5),
                activate_on: self.str("activate-on").unwrap_or("exclusive"),
            },
            "hat-buttons" => {
                let eight = self.parse("button-count") == Some(8);
                let mut directions: Vec<_> = HAT_BUTTONS
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| eight || i % 2 == 0)
                    .map(|(i, &name)| (name, Some(i as u16 * 4500), self.refs_in(name)))
                    .collect();
                directions.push(("Center", None, self.refs_in("Center")));
                ActionKind::HatButtons { directions }
            }
            "description" => ActionKind::Description {
                text: self.str("description").unwrap_or_default(),
            },
            "smart-toggle" => ActionKind::SmartToggle {
                children: self.refs_in("actions"),
                delay: self.parse("delay").unwrap_or(0.5),
            },
            "split-axis" => ActionKind::SplitAxis {
                split: self.parse("split-value").unwrap_or(0.0),
                lower: self.refs_in("lower-actions"),
                upper: self.refs_in("upper-actions"),
            },
            "pause-resume" => ActionKind::PauseResume {
                operation: self.str("operation").unwrap_or("Pause"),
            },
            "run-command" => ActionKind::RunCommand {
                executable: self.str("executable").unwrap_or_default(),
                arguments: self.str("arguments").unwrap_or_default(),
            },
            "map-to-keyboard" => ActionKind::MapToKeyboard {
                keys: self
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        Item::Group { tag, items, .. } if tag == "input" => Some(Key {
                            scan_code: find_property(items, "scan-code")?
                                .value
                                .trim()
                                .parse()
                                .ok()?,
                            extended: find_property(items, "is-extended")
                                .is_some_and(|p| p.value.trim().eq_ignore_ascii_case("true")),
                        }),
                        _ => None,
                    })
                    .collect(),
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
            "macro" => {
                let delay = self.parse("repeat-delay").unwrap_or(0.1);
                let repeat = match self
                    .str("repeat-mode")
                    .map(|m| m.trim().to_ascii_lowercase())
                {
                    Some(m) if m == "count" => MacroRepeat::Count {
                        count: self.parse("repeat-count").unwrap_or(1),
                        delay,
                    },
                    Some(m) if m == "hold" => MacroRepeat::Hold { delay },
                    Some(m) if m == "toggle" => MacroRepeat::Toggle { delay },
                    _ => MacroRepeat::Single,
                };
                let steps = self
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        Item::Group { tag, attrs, items } if tag == "macro-action" => {
                            let ty = attrs.iter().find(|(k, _)| k == "type")?.1.as_str();
                            Some(macro_step(ty, items))
                        }
                        _ => None,
                    })
                    .collect();
                ActionKind::Macro {
                    steps,
                    repeat,
                    exclusive: self.bool("is-exclusive"),
                    preemptive: self.bool("is-preemptive"),
                }
            }
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

fn macro_step(ty: &str, items: &[Item]) -> MacroStep {
    let str = |n| find_property(items, n).map(|p| p.value.trim());
    let num = |n| str(n).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let flag = |n| str(n).is_some_and(|v| v.eq_ignore_ascii_case("true"));
    match ty {
        "pause" => MacroStep::Pause(num("duration")),
        "key" => MacroStep::Key {
            scan_code: num("scan-code") as u16,
            extended: flag("is-extended"),
            pressed: flag("is-pressed"),
        },
        "mouse-button" => match str("button").and_then(MouseButton::parse) {
            Some(button) => MacroStep::MouseButton {
                button,
                pressed: flag("is-pressed"),
            },
            None => MacroStep::Other,
        },
        "mouse-motion" => MacroStep::MouseMotion {
            dx: num("dx") as i32,
            dy: num("dy") as i32,
        },
        "vjoy" => {
            let device = num("vjoy-id") as u32;
            let id = num("input-id") as u32;
            match InputType::parse(str("input-type").unwrap_or_default()) {
                InputType::Button => MacroStep::VjoyButton {
                    device,
                    id,
                    pressed: flag("value"),
                },
                InputType::Axis => MacroStep::VjoyAxis {
                    device,
                    id,
                    value: num("value"),
                    relative: str("axis-mode").is_some_and(|m| m.eq_ignore_ascii_case("relative")),
                },
                InputType::Hat => MacroStep::VjoyHat {
                    device,
                    id,
                    direction: str("value").and_then(hat_direction),
                },
                InputType::Other => MacroStep::Other,
            }
        }
        _ => MacroStep::Other,
    }
}

/// Gremlin hat direction name (`north-east`) to hundredths of a degree.
fn hat_direction(name: &str) -> Option<u16> {
    const NAMES: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    let name = name.to_ascii_lowercase();
    NAMES
        .iter()
        .position(|n| *n == name)
        .map(|i| i as u16 * 4500)
}

fn parse_point(s: &str) -> Option<(f64, f64)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}
