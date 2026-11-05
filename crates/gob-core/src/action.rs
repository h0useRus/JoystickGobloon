//! Actions as stored in a profile's `<library>`.
//!
//! An action is a generic tree of properties, groups and action references,
//! exactly as Gremlin writes it. [`Action::kind`] decodes the types Joystick Gobloon
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Typed view of the action types Joystick Gobloon can execute.
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
    /// Joystick Gobloon's own action (not in Gremlin): drives `control` of virtual gamepad
    /// `gamepad` (1-based). `control` is a name like `a`, `left-x`, `lt` or `dpad` (gob-virtual's
    /// `GamepadControl`); `inverted` flips a button or an axis.
    MapToGamepad {
        gamepad: u32,
        control: &'a str,
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
    /// Combines two physical axes (device GUID, axis id) into the value its children see.
    /// Usually the same action is bound to both axes.
    MergeAxis {
        axis1: (&'a str, u32),
        axis2: (&'a str, u32),
        operation: MergeOp,
        children: Vec<&'a str>,
    },
    /// Two axes as one stick: circular `inner` deadzone, square `outer` limit; `first`
    /// gets the first axis' result, `second` the second's.
    DualAxisDeadzone {
        axis1: (&'a str, u32),
        axis2: (&'a str, u32),
        inner: f64,
        outer: f64,
        first: Vec<&'a str>,
        second: Vec<&'a str>,
    },
    /// Writes to a logical device input; `relative` is the axis speed (absolute when `None`).
    MapToLogicalDevice {
        input_type: InputType,
        id: u32,
        inverted: bool,
        relative: Option<f64>,
    },
    /// Switches to another profile file and activates it.
    LoadProfile {
        file: &'a str,
    },
    /// Plays a sound file; `volume` 0..=100.
    PlaySound {
        file: &'a str,
        volume: u8,
    },
    /// Joystick Gobloon's own action (not in Gremlin): sets LED `led` of the joystick
    /// `device` (DirectInput instance GUID) to `color` / `color2` (8-bit RGB) combined by
    /// `blend`. Runs on the input like any action (`activation-mode`), or once when the
    /// profile is activated with `on_activate`.
    LightLed {
        device: &'a str,
        led: u8,
        /// 0: the whole LED; 1, 2: one of two single-color LEDs on that number (VKB base).
        element: u8,
        color: [u8; 3],
        color2: [u8; 3],
        blend: LedBlend,
        mode: LedMode,
        on_activate: bool,
    },
    /// Speaks `text` (`$current_mode` is replaced); rate/pitch -1..1, volume 0..1.
    /// `voice` is a Joystick Gobloon addition (Gremlin ignores it): an installed voice name.
    TextToSpeech {
        text: &'a str,
        voice: Option<&'a str>,
        queue_mode: &'a str,
        rate: f64,
        volume: f64,
        pitch: f64,
    },
    /// Runs `true_actions` when the conditions hold (all of them, or any with `all` false),
    /// otherwise `false_actions`. With no conditions it is true.
    Condition {
        all: bool,
        conditions: Vec<crate::condition::Condition>,
        true_actions: Vec<&'a str>,
        false_actions: Vec<&'a str>,
    },
    /// Pulses `positive` / `negative` each time the axis moved `threshold` that way.
    AxisDelta {
        threshold: f64,
        positive: Vec<&'a str>,
        negative: Vec<&'a str>,
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
    /// Joystick Gobloon's own action: listens for voice commands while the input is held.
    /// `microphone` is an id from gob-voice's list; empty = the system's default one.
    VoiceInput {
        microphone: &'a str,
    },
    /// Joystick Gobloon's own action: buttons move an axis (see [`crate::button_axis`]).
    ButtonAxis(crate::ButtonAxis),
    /// Preserved in the profile but not executed (yet).
    Other,
}

/// How a light-led LED shines (`led-mode` property).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LedMode {
    Off,
    #[default]
    On,
    SlowBlink,
    FastBlink,
    FastestBlink,
}

impl LedMode {
    pub const ALL: [LedMode; 5] = [
        Self::On,
        Self::SlowBlink,
        Self::FastBlink,
        Self::FastestBlink,
        Self::Off,
    ];

    /// Value of the `led-mode` property.
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::SlowBlink => "slow-blink",
            Self::FastBlink => "fast-blink",
            Self::FastestBlink => "fastest-blink",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }
}

/// How a light-led LED's two colors combine (`color-mode` property).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LedBlend {
    /// `color` only.
    #[default]
    First,
    /// `color2` only.
    Second,
    /// `color`, then `color2` (with blinking).
    Alternate,
    /// `color2`, then `color`.
    AlternateReverse,
    /// Both at once.
    Mix,
}

impl LedBlend {
    pub const ALL: [LedBlend; 5] = [
        Self::First,
        Self::Second,
        Self::Alternate,
        Self::AlternateReverse,
        Self::Mix,
    ];

    /// Value of the `color-mode` property.
    pub fn name(self) -> &'static str {
        match self {
            Self::First => "color1",
            Self::Second => "color2",
            Self::Alternate => "alternate",
            Self::AlternateReverse => "alternate-reverse",
            Self::Mix => "mix",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }
}

/// `#RRGGBB` (or `RRGGBB`) to RGB.
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// RGB as `#RRGGBB`, the form light-led stores.
pub fn format_color([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// How merge-axis combines its two axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOp {
    Average,
    Minimum,
    Maximum,
    /// Clamped to -1..1.
    Sum,
    /// `(axis2 - axis1) / 2`: two pedals into one left/right axis.
    Bidirectional,
    /// Axis furthest from center; ties go to the second.
    MaximumDeflection,
    /// Axis closest to center; ties go to the second.
    ClosestToCenter,
}

impl MergeOp {
    pub const ALL: [MergeOp; 7] = [
        Self::Average,
        Self::Minimum,
        Self::Maximum,
        Self::Sum,
        Self::Bidirectional,
        Self::MaximumDeflection,
        Self::ClosestToCenter,
    ];

    /// Value of the `operation` property.
    pub fn name(self) -> &'static str {
        match self {
            Self::Average => "average",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
            Self::Sum => "sum",
            Self::Bidirectional => "bidirectional",
            Self::MaximumDeflection => "maximum-deflection",
            Self::ClosestToCenter => "closest-to-center",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|o| o.name() == s)
    }

    pub fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            Self::Average => (a + b) / 2.0,
            Self::Minimum => a.min(b),
            Self::Maximum => a.max(b),
            Self::Sum => (a + b).clamp(-1.0, 1.0),
            Self::Bidirectional => (b - a) / 2.0,
            Self::MaximumDeflection => {
                if a.abs() > b.abs() {
                    a
                } else {
                    b
                }
            }
            Self::ClosestToCenter => {
                if a.abs() < b.abs() {
                    a
                } else {
                    b
                }
            }
        }
    }
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
#[derive(Debug, Clone, PartialEq)]
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
    /// Joystick Gobloon's own steps (not in Gremlin) for virtual gamepad `gamepad` (1-based);
    /// `control` is a map-to-gamepad control name (`a`, `dpad-up`, ...).
    GamepadButton {
        gamepad: u32,
        control: String,
        pressed: bool,
    },
    /// A stick (-1..1) or trigger (0..1) set to `value`.
    GamepadAxis {
        gamepad: u32,
        control: String,
        value: f64,
    },
    /// The whole d-pad; hundredths of a degree, `None` when centered.
    GamepadDpad {
        gamepad: u32,
        direction: Option<u16>,
    },
    /// Joystick Gobloon's own step: runs the library action `action` (its id) as a short
    /// press of its input would, press and release.
    RunAction {
        action: String,
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

/// How a button-like output (key, mouse button or wheel, virtual stick or gamepad button)
/// follows its input: Joystick Gobloon's `send-mode` property, Gremlin has only [`SendMode::Hold`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum SendMode {
    /// Down while the input is down.
    #[default]
    Hold,
    /// The input presses it; something else releases it.
    Press,
    /// The input releases it.
    Release,
    /// Down for [`ButtonSend::pulse_ms`], however long the input is held.
    Pulse,
    /// Pulses while the input is held: the first at once, then after
    /// [`ButtonSend::delay_ms`], then every [`ButtonSend::period_ms`].
    Repeat,
    /// The first press holds it, the next releases it.
    Toggle,
}

impl SendMode {
    pub const ALL: [SendMode; 6] = [
        Self::Hold,
        Self::Press,
        Self::Release,
        Self::Pulse,
        Self::Repeat,
        Self::Toggle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Press => "press",
            Self::Release => "release",
            Self::Pulse => "pulse",
            Self::Repeat => "repeat",
            Self::Toggle => "toggle",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }
}

/// [`SendMode`] with its times in milliseconds, each at least [`ButtonSend::MIN_MS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonSend {
    pub mode: SendMode,
    /// How long a pulse holds the output down.
    pub pulse_ms: u32,
    /// Repeat: from the first pulse to the second.
    pub delay_ms: u32,
    /// Repeat: between the later pulses.
    pub period_ms: u32,
}

impl ButtonSend {
    /// Shorter pulses are missed by games that poll slowly, and shorter periods flood them.
    pub const MIN_MS: u32 = 10;
    pub const MAX_MS: u32 = 60_000;
    pub const DEFAULT: Self = Self {
        mode: SendMode::Hold,
        pulse_ms: 50,
        delay_ms: 500,
        period_ms: 100,
    };
}

impl Default for ButtonSend {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// hat-buttons group tags, clockwise from north (45 degrees apart).
pub(crate) const HAT_BUTTONS: [&str; 8] = [
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
    "map-to-gamepad",
    "change-mode",
    "response-curve",
    "tempo",
    "description",
    "map-to-mouse",
    "map-to-keyboard",
    "chain",
    "double-tap",
    "hat-buttons",
    "merge-axis",
    "axis-delta",
    "dual-axis-deadzone",
    "condition",
    "load-profile",
    "map-to-logical-device",
    "play-sound",
    "light-led",
    "text-to-speech",
    "smart-toggle",
    "split-axis",
    "pause-resume",
    "run-command",
    "macro",
    "voice-input",
    crate::button_axis::BUTTON_AXIS,
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

    /// How the action's button output follows its input (`send-*` properties).
    pub fn button_send(&self) -> ButtonSend {
        let d = ButtonSend::DEFAULT;
        let ms = |name: &str, default: u32| {
            self.parse(name)
                .unwrap_or(default)
                .clamp(ButtonSend::MIN_MS, ButtonSend::MAX_MS)
        };
        ButtonSend {
            mode: self
                .str("send-mode")
                .and_then(SendMode::parse)
                .unwrap_or_default(),
            pulse_ms: ms("send-pulse-ms", d.pulse_ms),
            delay_ms: ms("send-delay-ms", d.delay_ms),
            period_ms: ms("send-period-ms", d.period_ms),
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

    /// Ids of the actions a macro's run-action steps run (not its children: they are shown
    /// and edited where they are bound).
    pub fn run_targets(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter_map(|i| match i {
                Item::Group { tag, attrs, items }
                    if tag == "macro-action"
                        && attrs.iter().any(|(k, v)| k == "type" && v == "action") =>
                {
                    find_property(items, "action-id").map(|p| p.value.trim())
                }
                _ => None,
            })
            .filter(|id| !id.is_empty())
            .collect()
    }

    /// Every action this one uses: its children and the actions its run-action steps run.
    pub fn reference_ids(&self) -> Vec<&str> {
        let mut out = self.child_ids();
        out.extend(self.run_targets());
        out
    }

    /// Whether a button press means something to the action, so a run-action macro step
    /// (a short press) can run it. Axis-only and hat-only actions cannot.
    pub fn runs_on_button(&self) -> bool {
        match self.kind() {
            ActionKind::MapToVjoy { input_type, .. } => input_type != InputType::Hat,
            _ => !matches!(
                self.kind.as_str(),
                "root"
                    | "description"
                    | "response-curve"
                    | "split-axis"
                    | "merge-axis"
                    | "axis-delta"
                    | "dual-axis-deadzone"
                    | "hat-buttons"
            ),
        }
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
            "map-to-gamepad" => ActionKind::MapToGamepad {
                gamepad: self.parse("gamepad-id").unwrap_or(1),
                control: self.str("control").unwrap_or_default().trim(),
                inverted: self.bool("inverted"),
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
            "map-to-logical-device" => ActionKind::MapToLogicalDevice {
                input_type: InputType::parse(
                    self.str("logical-input-type").unwrap_or_default().trim(),
                ),
                id: self.parse("logical-input-id").unwrap_or(1),
                inverted: self.bool("button-inverted"),
                relative: (self.str("axis-mode").map(str::trim) == Some("relative"))
                    .then(|| self.parse("axis-scaling").unwrap_or(1.0)),
            },
            "load-profile" => ActionKind::LoadProfile {
                file: self.str("load-profile").unwrap_or_default().trim(),
            },
            "play-sound" => ActionKind::PlaySound {
                file: self.str("filename").unwrap_or_default().trim(),
                volume: self
                    .parse::<f64>("volume")
                    .unwrap_or(50.0)
                    .clamp(0.0, 100.0) as u8,
            },
            "light-led" => ActionKind::LightLed {
                device: self.str("device-guid").unwrap_or_default().trim(),
                led: self.parse("led-id").unwrap_or(0),
                element: self.parse("led-element").unwrap_or(0),
                color: self.str("color").and_then(parse_color).unwrap_or([0, 0, 0]),
                color2: self
                    .str("color2")
                    .and_then(parse_color)
                    .unwrap_or([0, 0, 0]),
                blend: self
                    .str("color-mode")
                    .and_then(LedBlend::parse)
                    .unwrap_or_default(),
                mode: self
                    .str("led-mode")
                    .and_then(LedMode::parse)
                    .unwrap_or_default(),
                on_activate: self.bool("on-activate"),
            },
            "text-to-speech" => ActionKind::TextToSpeech {
                text: self.str("text").unwrap_or_default(),
                voice: self.str("voice").map(str::trim).filter(|v| !v.is_empty()),
                queue_mode: self.str("queue-mode").unwrap_or("queue-back").trim(),
                rate: self.parse("playback-rate").unwrap_or(0.0),
                volume: self.parse("playback-volume").unwrap_or(1.0),
                pitch: self.parse("playback-pitch").unwrap_or(0.0),
            },
            "condition" => ActionKind::Condition {
                all: !self
                    .str("logical-operator")
                    .is_some_and(|o| o.trim().eq_ignore_ascii_case("any")),
                conditions: self
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        Item::Group { tag, items, .. } if tag == "condition" => {
                            Some(crate::condition::Condition::parse(items))
                        }
                        _ => None,
                    })
                    .collect(),
                true_actions: self.refs_in("true-actions"),
                false_actions: self.refs_in("false-actions"),
            },
            "dual-axis-deadzone" => ActionKind::DualAxisDeadzone {
                axis1: self.axis_ref(1),
                axis2: self.axis_ref(2),
                inner: self.parse("inner-deadzone").unwrap_or(0.0),
                outer: self.parse("outer-deadzone").unwrap_or(1.0),
                first: self.refs_in("output1-actions"),
                second: self.refs_in("output2-actions"),
            },
            "axis-delta" => ActionKind::AxisDelta {
                threshold: self.parse("threshold").unwrap_or(0.1),
                positive: self.refs_in("positive-actions"),
                negative: self.refs_in("negative-actions"),
            },
            "merge-axis" => ActionKind::MergeAxis {
                axis1: self.axis_ref(1),
                axis2: self.axis_ref(2),
                operation: self
                    .str("operation")
                    .and_then(MergeOp::parse)
                    .unwrap_or(MergeOp::Average),
                children: self.refs_in("actions"),
            },
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
            "voice-input" => ActionKind::VoiceInput {
                microphone: self.str("microphone").unwrap_or_default().trim(),
            },
            crate::button_axis::BUTTON_AXIS => {
                crate::ButtonAxis::of(self).map_or(ActionKind::Other, ActionKind::ButtonAxis)
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

    /// `axis{n}-guid` / `axis{n}-axis` of merge-axis and dual-axis-deadzone.
    fn axis_ref(&self, n: u8) -> (&str, u32) {
        (
            self.str(&format!("axis{n}-guid"))
                .unwrap_or_default()
                .trim(),
            self.parse(&format!("axis{n}-axis")).unwrap_or(0),
        )
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
        "gamepad" => {
            let gamepad = (num("gamepad-id") as u32).max(1);
            let control = str("control").unwrap_or_default();
            let value = str("value").unwrap_or_default();
            if control == "dpad" {
                MacroStep::GamepadDpad {
                    gamepad,
                    direction: hat_direction(value),
                }
            } else if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
                MacroStep::GamepadButton {
                    gamepad,
                    control: control.to_string(),
                    pressed: flag("value"),
                }
            } else {
                MacroStep::GamepadAxis {
                    gamepad,
                    control: control.to_string(),
                    value: num("value"),
                }
            }
        }
        "action" => MacroStep::RunAction {
            action: str("action-id").unwrap_or_default().to_string(),
        },
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
