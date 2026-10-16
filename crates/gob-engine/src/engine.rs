use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use gob_core::{
    Action, ActionKind, ActivationMode, AxisDirection, Comparator, ConditionSource, InputType,
    Item, Key, MacroRepeat, MacroStep, MergeOp, MouseAction, MouseButton, Profile, VirtualButton,
};

use crate::media::{SpeechQueue, SpeechRequest};
use crate::modes::ModeStack;
use crate::motion::{compass, Motion};
use crate::{InputKey, InputKind, Output, Value};

/// Delay between press and release of a pulse (Gremlin: 50 ms).
const PULSE: Duration = Duration::from_millis(50);
/// Guards against reference cycles in broken profiles.
const MAX_DEPTH: usize = 32;
const VIRTUAL_DEVICE: &str = "virtual";
/// Device of the virtual buttons a hat-buttons action creates, one per direction.
const HAT_BUTTON_DEVICE: &str = "virtual-hat";
/// Gremlin's default pause between macro steps without an explicit pause.
const MACRO_DELAY: f64 = 0.05;
/// Lower bound between macro repetitions so a zero delay cannot spin `tick`.
const MACRO_MIN_REPEAT: Duration = Duration::from_millis(1);

type NodeId = usize;

#[derive(Debug, Clone, Copy)]
enum VjoyTarget {
    Axis(u8),
    Button(u8),
    Hat(u8),
}

#[derive(Debug, Clone, Copy)]
enum ChangeKind {
    Switch,
    Previous,
    Unwind,
    Cycle,
    Temporary,
}

#[derive(Debug, Clone)]
enum Node {
    /// Root: children run in order and share one mutable value.
    Seq(Vec<NodeId>),
    Vjoy {
        device: u32,
        target: VjoyTarget,
        inverted: bool,
        act: ActivationMode,
        /// Relative axis mode: speed (Gremlin's axis-scaling); absolute when `None`.
        relative: Option<f32>,
    },
    ChangeMode {
        kind: ChangeKind,
        targets: Vec<String>,
        act: ActivationMode,
    },
    Curve {
        deadzone: [f32; 4],
        points: Vec<(f32, f32)>,
    },
    Mouse {
        action: MouseAction,
        act: ActivationMode,
    },
    Keys {
        keys: Vec<Key>,
        act: ActivationMode,
    },
    SmartToggle {
        children: Vec<NodeId>,
        delay: Duration,
    },
    Split {
        split: f32,
        lower: Vec<NodeId>,
        upper: Vec<NodeId>,
    },
    Pause(PauseOp),
    Run {
        executable: String,
        arguments: Vec<String>,
        act: ActivationMode,
    },
    Tempo {
        short: Vec<NodeId>,
        long: Vec<NodeId>,
        threshold: Duration,
        on_press: bool,
    },
    Chain {
        chains: Vec<Vec<NodeId>>,
        /// Zero: never restart.
        timeout: Duration,
    },
    DoubleTap {
        single: Vec<NodeId>,
        double: Vec<NodeId>,
        threshold: Duration,
        exclusive: bool,
    },
    /// Hat direction (`None` = center) -> actions run as a button while the hat points exactly there.
    HatButtons(Vec<(Option<u16>, Vec<NodeId>)>),
    LoadProfile {
        file: String,
        act: ActivationMode,
    },
    PlaySound {
        file: String,
        volume: u8,
        act: ActivationMode,
    },
    /// `request.text` still holds `$current_mode` placeholders.
    Speak {
        request: SpeechRequest,
        act: ActivationMode,
    },
    /// Gremlin's condition action: `branches[0]` when true, `branches[1]` when false.
    Condition {
        all: bool,
        conditions: Vec<Cond>,
        branches: [Vec<NodeId>; 2],
        act: ActivationMode,
    },
    /// Radial inner deadzone and square outer limit over two axes, each result to its list.
    DualDeadzone {
        axes: [InputKey; 2],
        inner: f32,
        outer: f32,
        outputs: [Vec<NodeId>; 2],
    },
    /// Accumulates raw axis changes; a pulse whenever they reach `threshold` either way.
    AxisDelta {
        threshold: f32,
        positive: Vec<NodeId>,
        negative: Vec<NodeId>,
    },
    /// The two axes' latest values combined; children see the result.
    Merge {
        axes: [InputKey; 2],
        op: MergeOp,
        children: Vec<NodeId>,
    },
    /// Steps already include the default delay between non-pause steps.
    Macro {
        steps: Vec<MacroStep>,
        repeat: MacroRepeat,
        act: ActivationMode,
    },
    /// Description and action types JoyGob does not execute yet.
    Noop,
}

#[derive(Debug, Clone)]
enum CondSource {
    Current,
    Inputs(Vec<InputKey>),
    Keys(Vec<Key>),
    Vjoy(u32, InputKind, u32),
    /// Logical devices and unknown sources.
    Never,
}

#[derive(Debug, Clone)]
struct Cond {
    source: CondSource,
    comparator: Comparator,
}

/// Where an event came from and the mode it was processed in.
#[derive(Debug, Clone)]
struct Source {
    key: InputKey,
    mode: String,
    /// Set inside smart-toggle: vJoy presses are not auto-released.
    no_auto_release: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseOp {
    Pause,
    Resume,
    Toggle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum TogglePhase {
    #[default]
    Wait,
    Down,
    Held,
    Latched,
}

#[derive(Debug, Default)]
struct ToggleState {
    phase: TogglePhase,
    timer: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Wait,
    Short,
    Long,
}

#[derive(Debug, Default)]
struct TempoState {
    phase: Phase,
    timer: Option<u64>,
    press: Option<(Value, Source)>,
}

/// Double-tap FSM states (Gremlin names: neutral, p1, p1+t, t, p2+t; +t = timer running).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum TapPhase {
    #[default]
    Neutral,
    Pressed,
    PressedTimed,
    Timed,
    SecondTimed,
}

#[derive(Debug, Default)]
struct TapState {
    phase: TapPhase,
    timer: Option<u64>,
    press: Option<(Value, Source)>,
}

#[derive(Debug, Default)]
struct ChainState {
    index: usize,
    last: Option<Instant>,
}

/// A macro being executed (Gremlin's MacroManager entry, driven by timers instead of a thread).
#[derive(Debug, Default)]
struct MacroRun {
    /// Next step to execute.
    pos: usize,
    /// Completed repetitions.
    rounds: u32,
    /// Waiting out the repeat delay: check whether to go on before the next round.
    between_rounds: bool,
    /// Termination requested; honored between rounds like Gremlin.
    stop: bool,
    /// Activations received while running, started one after another.
    queued: u32,
}

#[derive(Debug)]
enum TimerKind {
    MacroStep(NodeId),
    TapTimeout(NodeId),
    TempoTimeout(NodeId),
    ToggleTimeout(NodeId),
    PulseRelease { nodes: Vec<NodeId>, src: Source },
}

#[derive(Debug)]
struct Timer {
    id: u64,
    at: Instant,
    kind: TimerKind,
}

#[derive(Debug, Clone, PartialEq)]
enum ReleaseAction {
    VjoyButton(u32, u8),
    LeaveTemporary(String),
    TempoRelease(NodeId),
    StopMacro(NodeId),
    TapRelease(NodeId),
    ReleaseKeys(NodeId),
    HatButtons(NodeId),
}

/// Runs when the physical input that registered it is released (Gremlin's ButtonReleaseActions).
#[derive(Debug, Clone, PartialEq)]
struct ReleaseEntry {
    mode: String,
    /// Run in any mode; otherwise only when the mode changed since registration.
    any_mode: bool,
    on_press: bool,
    action: ReleaseAction,
}

#[derive(Debug)]
enum VButton {
    Axis {
        lower: f32,
        upper: f32,
        direction: AxisDirection,
        last: Option<f32>,
        down: bool,
    },
    Hat {
        directions: Vec<Option<u16>>,
        down: bool,
    },
}

impl VButton {
    /// Button states produced by a raw axis or hat value (Gremlin's VirtualAxisButton/VirtualHatButton).
    fn feed(&mut self, value: Value) -> Vec<bool> {
        match (self, value) {
            (
                VButton::Axis {
                    lower,
                    upper,
                    direction,
                    last,
                    down,
                },
                Value::Axis(v),
            ) => {
                let (lower, upper) = (*lower, *upper);
                let Some(prev) = last.replace(v) else {
                    *down = (lower..=upper).contains(&v);
                    return Vec::new();
                };
                let mut states = Vec::new();
                let crossed = (prev < lower && v > upper) || (prev > upper && v < lower);
                if crossed {
                    states.extend([true, false]);
                    *down = false;
                }
                // Gremlin labels increasing values "below" and decreasing ones "above".
                let moving = if prev < v {
                    AxisDirection::Below
                } else if prev > v {
                    AxisDirection::Above
                } else {
                    AxisDirection::Anywhere
                };
                let valid = *direction == AxisDirection::Anywhere || moving == *direction;
                let inside = (lower..=upper).contains(&v);
                if inside && valid {
                    if !*down {
                        *down = true;
                        states = vec![true];
                    }
                } else if *down {
                    *down = false;
                    states = vec![false];
                }
                states
            }
            (VButton::Hat { directions, down }, Value::Hat(d)) => {
                let pressed = directions.contains(&d);
                if pressed == *down {
                    return Vec::new();
                }
                *down = pressed;
                vec![pressed]
            }
            _ => Vec::new(),
        }
    }
}

struct Config {
    root: NodeId,
    /// Runs while paused: the root holds a pause-resume action.
    always: bool,
    vbutton: Option<VButton>,
    virtual_id: u32,
}

pub struct Engine {
    nodes: Vec<Node>,
    tempo: HashMap<NodeId, TempoState>,
    cycle: HashMap<NodeId, usize>,
    sets: Vec<Vec<Config>>,
    lookup: HashMap<InputKey, HashMap<String, usize>>,
    modes: ModeStack,
    releases: HashMap<InputKey, Vec<ReleaseEntry>>,
    timers: Vec<Timer>,
    next_timer: u64,
    axes: HashMap<InputKey, f32>,
    relative: HashMap<(u32, u8), RelativeAxis>,
    /// Cursor motion per (action, input), tagged with the mode it started in.
    motion: Motion<(NodeId, InputKey), String>,
    toggles: HashMap<NodeId, ToggleState>,
    macros: HashMap<NodeId, MacroRun>,
    taps: HashMap<NodeId, TapState>,
    chains: HashMap<NodeId, ChainState>,
    /// Physical button and hat states, for conditions.
    buttons: HashMap<InputKey, bool>,
    hats: HashMap<InputKey, Option<u16>>,
    /// Condition branch (true = 0) chosen on press, reused on release / hat center.
    cond_cache: HashMap<NodeId, usize>,
    /// axis-delta: last raw value and the change accumulated since the last pulse.
    deltas: HashMap<NodeId, (Option<f32>, f32)>,
    /// Pressed state of each hat-buttons direction.
    hat_buttons: HashMap<NodeId, Vec<bool>>,
    /// map-to-keyboard actions whose keys are held.
    keys_down: HashSet<NodeId>,
    /// Profile's `macro-default-delay` in seconds.
    macro_delay: f64,
    /// Last value written to each vJoy axis, for relative macro steps.
    vjoy_axes: HashMap<(u32, u8), f32>,
    /// Paused by a pause-resume action: only bindings containing one still run.
    paused: bool,
    /// Profile device GUID -> attached device GUID, lowercase.
    aliases: HashMap<String, String>,
}

/// A vJoy axis moved by speed rather than position (Gremlin's relative mode).
#[derive(Debug)]
struct RelativeAxis {
    value: f32,
    /// Change per [`RELATIVE_STEP`].
    delta: f32,
    active: bool,
    /// Input is near center: stop after [`RELATIVE_STOP`] without updates.
    stopping: bool,
    last_update: Instant,
    last_step: Instant,
}

/// Relative axes advance in steps of this length (Gremlin's 10 ms thread).
const RELATIVE_STEP: Duration = Duration::from_millis(10);
const RELATIVE_STOP: Duration = Duration::from_secs(1);

impl Engine {
    /// Compiles `profile`. `aliases` maps profile device GUIDs to attached ones
    /// (same stick under another GUID on this PC).
    pub fn new(profile: &Profile, aliases: &HashMap<String, String>) -> Self {
        let index = profile.action_index();
        let start = profile
            .startup_mode()
            .filter(|m| profile.modes.iter().any(|x| &x.name == m))
            .or_else(|| profile.modes.first().map(|m| m.name.clone()))
            .unwrap_or_else(|| "Default".into());
        let mut engine = Self {
            nodes: Vec::new(),
            tempo: HashMap::new(),
            cycle: HashMap::new(),
            sets: Vec::new(),
            lookup: HashMap::new(),
            modes: ModeStack::new(&start),
            releases: HashMap::new(),
            timers: Vec::new(),
            next_timer: 0,
            axes: HashMap::new(),
            relative: HashMap::new(),
            motion: Motion::default(),
            toggles: HashMap::new(),
            macros: HashMap::new(),
            taps: HashMap::new(),
            chains: HashMap::new(),
            hat_buttons: HashMap::new(),
            deltas: HashMap::new(),
            buttons: HashMap::new(),
            hats: HashMap::new(),
            cond_cache: HashMap::new(),
            keys_down: HashSet::new(),
            macro_delay: profile
                .settings
                .child_text("macro-default-delay")
                .and_then(|d| d.trim().parse().ok())
                .filter(|d: &f64| *d >= 0.0)
                .unwrap_or(MACRO_DELAY),
            vjoy_axes: HashMap::new(),
            paused: false,
            aliases: aliases
                .iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v.to_ascii_lowercase()))
                .collect(),
        };

        let mut virtual_id = 0;
        for b in &profile.inputs {
            let kind = match b.input_type() {
                InputType::Axis => InputKind::Axis,
                InputType::Button => InputKind::Button,
                InputType::Hat => InputKind::Hat,
                InputType::Other => continue,
            };
            let Ok(id) = b.input_id.trim().parse() else {
                continue;
            };
            let key = InputKey {
                device: engine.device(&b.device_id),
                kind,
                id,
            };
            for cfg in &b.configs {
                let root = engine.compile(&index, &cfg.root_action, 0);
                virtual_id += 1;
                let vbutton = match cfg.virtual_button(b.input_type()) {
                    Some(VirtualButton::Axis {
                        lower,
                        upper,
                        direction,
                    }) => Some(VButton::Axis {
                        lower,
                        upper,
                        direction,
                        last: None,
                        down: false,
                    }),
                    Some(VirtualButton::Hat { directions }) => Some(VButton::Hat {
                        directions,
                        down: false,
                    }),
                    None => None,
                };
                let modes = engine.lookup.entry(key.clone()).or_default();
                let set = *modes.entry(b.mode.clone()).or_insert_with(|| {
                    engine.sets.push(Vec::new());
                    engine.sets.len() - 1
                });
                // pause-resume and load-profile (Gremlin's AlwaysExecute) also run while paused.
                let always = matches!(&engine.nodes[root], Node::Seq(c) if c.iter().any(|&n| matches!(engine.nodes[n], Node::Pause(_) | Node::LoadProfile { .. })));
                engine.sets[set].push(Config {
                    always,
                    root,
                    vbutton,
                    virtual_id,
                });
            }
        }
        engine.inherit_modes(profile);
        engine
    }

    /// Attached device for a profile device GUID.
    fn device(&self, guid: &str) -> String {
        let guid = guid.trim().to_ascii_lowercase();
        self.aliases.get(&guid).cloned().unwrap_or(guid)
    }

    /// Child modes use their parent's binding (the very same one) unless they override it.
    fn inherit_modes(&mut self, profile: &Profile) {
        let names: Vec<&str> = profile.modes.iter().map(|m| m.name.as_str()).collect();
        let mut queue: VecDeque<&str> = profile
            .modes
            .iter()
            .filter(|m| m.parent.as_deref().is_none_or(|p| !names.contains(&p)))
            .map(|m| m.name.as_str())
            .collect();
        while let Some(parent) = queue.pop_front() {
            for child in profile
                .modes
                .iter()
                .filter(|m| m.parent.as_deref() == Some(parent))
            {
                for modes in self.lookup.values_mut() {
                    if let Some(&set) = modes.get(parent) {
                        modes.entry(child.name.clone()).or_insert(set);
                    }
                }
                queue.push_back(&child.name);
            }
        }
    }

    fn compile(&mut self, index: &HashMap<&str, &Action>, id: &str, depth: usize) -> NodeId {
        let node = match index.get(id).filter(|_| depth < MAX_DEPTH) {
            None => Node::Noop,
            Some(action) => match action.kind() {
                ActionKind::Root { children } => Node::Seq(
                    children
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                ),
                ActionKind::MapToVjoy {
                    device,
                    input_type,
                    input_id,
                    inverted,
                } => {
                    let id = input_id.min(255) as u8;
                    let target = match input_type {
                        InputType::Axis => Some(VjoyTarget::Axis(id)),
                        InputType::Button => Some(VjoyTarget::Button(id)),
                        InputType::Hat => Some(VjoyTarget::Hat(id)),
                        InputType::Other => None,
                    };
                    let relative = (action.str("axis-mode") == Some("relative")).then(|| {
                        action
                            .str("axis-scaling")
                            .and_then(|s| s.trim().parse().ok())
                            .unwrap_or(1.0)
                    });
                    target.map_or(Node::Noop, |target| Node::Vjoy {
                        device,
                        target,
                        inverted,
                        act: action.activation_mode(),
                        relative,
                    })
                }
                ActionKind::ChangeMode { change_type, .. } => Node::ChangeMode {
                    kind: match change_type {
                        "Previous" => ChangeKind::Previous,
                        "Unwind" => ChangeKind::Unwind,
                        "Cycle" => ChangeKind::Cycle,
                        "Temporary" => ChangeKind::Temporary,
                        _ => ChangeKind::Switch,
                    },
                    targets: target_modes(action),
                    act: action.activation_mode(),
                },
                ActionKind::ResponseCurve {
                    deadzone, points, ..
                } => {
                    let mut points: Vec<(f32, f32)> =
                        points.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
                    points.sort_by(|a, b| a.0.total_cmp(&b.0));
                    Node::Curve {
                        deadzone: deadzone.map(|d| d as f32),
                        points,
                    }
                }
                ActionKind::Tempo {
                    short,
                    long,
                    threshold,
                    activate_on,
                } => Node::Tempo {
                    short: short
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    long: long
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    threshold: Duration::from_secs_f64(threshold.max(0.0)),
                    on_press: activate_on == "press",
                },
                ActionKind::Chain { chains, timeout } => Node::Chain {
                    chains: chains
                        .iter()
                        .map(|c| {
                            c.iter()
                                .map(|a| self.compile(index, a, depth + 1))
                                .collect()
                        })
                        .collect(),
                    timeout: Duration::from_secs_f64(timeout.max(0.0)),
                },
                ActionKind::DoubleTap {
                    single,
                    double,
                    threshold,
                    activate_on,
                } => Node::DoubleTap {
                    single: single
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    double: double
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    threshold: Duration::from_secs_f64(threshold.max(0.0)),
                    exclusive: activate_on != "combined",
                },
                ActionKind::HatButtons { directions } => Node::HatButtons(
                    directions
                        .into_iter()
                        .map(|(_, d, ids)| {
                            (
                                d,
                                ids.iter()
                                    .map(|c| self.compile(index, c, depth + 1))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
                ActionKind::LoadProfile { file } => Node::LoadProfile {
                    file: file.to_string(),
                    act: action.activation_mode(),
                },
                ActionKind::PlaySound { file, volume } => Node::PlaySound {
                    file: file.to_string(),
                    volume,
                    act: action.activation_mode(),
                },
                ActionKind::TextToSpeech {
                    text,
                    queue_mode,
                    rate,
                    volume,
                    pitch,
                } => Node::Speak {
                    request: SpeechRequest {
                        text: text.to_string(),
                        rate: rate as f32,
                        volume: volume as f32,
                        pitch: pitch as f32,
                        queue: SpeechQueue::parse(queue_mode),
                    },
                    act: action.activation_mode(),
                },
                ActionKind::Condition {
                    all,
                    conditions,
                    true_actions,
                    false_actions,
                } => Node::Condition {
                    all,
                    conditions: conditions
                        .into_iter()
                        .map(|c| Cond {
                            source: match c.source {
                                ConditionSource::CurrentInput => CondSource::Current,
                                ConditionSource::Joystick(inputs) => CondSource::Inputs(
                                    inputs
                                        .into_iter()
                                        .filter_map(|(device, ty, id)| {
                                            let kind = match ty {
                                                InputType::Axis => InputKind::Axis,
                                                InputType::Button => InputKind::Button,
                                                InputType::Hat => InputKind::Hat,
                                                InputType::Other => return None,
                                            };
                                            Some(InputKey {
                                                device: self.device(&device),
                                                kind,
                                                id,
                                            })
                                        })
                                        .collect(),
                                ),
                                ConditionSource::Keyboard(keys) => CondSource::Keys(keys),
                                ConditionSource::Vjoy {
                                    device,
                                    input_type,
                                    id,
                                } => match input_type {
                                    InputType::Axis => {
                                        CondSource::Vjoy(device, InputKind::Axis, id)
                                    }
                                    InputType::Hat => CondSource::Vjoy(device, InputKind::Hat, id),
                                    _ => CondSource::Vjoy(device, InputKind::Button, id),
                                },
                                ConditionSource::Other(_) => CondSource::Never,
                            },
                            comparator: c.comparator,
                        })
                        .collect(),
                    branches: [true_actions, false_actions].map(|ids| {
                        ids.iter()
                            .map(|c| self.compile(index, c, depth + 1))
                            .collect()
                    }),
                    act: action.activation_mode(),
                },
                ActionKind::DualAxisDeadzone {
                    axis1,
                    axis2,
                    inner,
                    outer,
                    first,
                    second,
                } => Node::DualDeadzone {
                    axes: [axis1, axis2].map(|(device, id)| InputKey {
                        device: self.device(device),
                        kind: InputKind::Axis,
                        id,
                    }),
                    inner: inner as f32,
                    outer: outer as f32,
                    outputs: [first, second].map(|ids| {
                        ids.iter()
                            .map(|c| self.compile(index, c, depth + 1))
                            .collect()
                    }),
                },
                ActionKind::AxisDelta {
                    threshold,
                    positive,
                    negative,
                } => Node::AxisDelta {
                    threshold: threshold.max(0.0001) as f32,
                    positive: positive
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    negative: negative
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                },
                ActionKind::MergeAxis {
                    axis1,
                    axis2,
                    operation,
                    children,
                } => Node::Merge {
                    axes: [axis1, axis2].map(|(device, id)| InputKey {
                        device: self.device(device),
                        kind: InputKind::Axis,
                        id,
                    }),
                    op: operation,
                    children: children
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                },
                ActionKind::MapToKeyboard { keys } => Node::Keys {
                    keys,
                    act: action.activation_mode(),
                },
                ActionKind::MapToMouse(action_kind) => Node::Mouse {
                    action: action_kind,
                    act: action.activation_mode(),
                },
                ActionKind::SmartToggle { children, delay } => Node::SmartToggle {
                    children: children
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    delay: Duration::from_secs_f64(delay.max(0.0)),
                },
                ActionKind::SplitAxis {
                    split,
                    lower,
                    upper,
                } => Node::Split {
                    split: split as f32,
                    lower: lower
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    upper: upper
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                },
                ActionKind::PauseResume { operation } => Node::Pause(match operation.trim() {
                    "Resume" => PauseOp::Resume,
                    "Toggle" => PauseOp::Toggle,
                    _ => PauseOp::Pause,
                }),
                ActionKind::RunCommand {
                    executable,
                    arguments,
                } => Node::Run {
                    executable: executable.to_string(),
                    arguments: split_command(arguments),
                    act: action.activation_mode(),
                },
                ActionKind::Macro { steps, repeat, .. } => Node::Macro {
                    steps: with_default_delay(&steps, self.macro_delay),
                    repeat,
                    act: action.activation_mode(),
                },
                ActionKind::Description { .. } | ActionKind::Other => Node::Noop,
            },
        };
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn current_mode(&self) -> &str {
        self.modes.current()
    }

    /// vJoy device ids the profile writes to.
    pub fn vjoy_devices(&self) -> BTreeSet<u32> {
        self.nodes
            .iter()
            .flat_map(|n| match n {
                Node::Vjoy { device, .. } => vec![*device],
                Node::Macro { steps, .. } => steps
                    .iter()
                    .filter_map(|s| match *s {
                        MacroStep::VjoyButton { device, .. }
                        | MacroStep::VjoyAxis { device, .. }
                        | MacroStep::VjoyHat { device, .. } => Some(device),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .collect()
    }

    /// Earliest pending timer; call [`Engine::tick`] at that time.
    pub fn next_deadline(&self) -> Option<Instant> {
        let relative = self
            .relative
            .values()
            .filter(|r| r.active)
            .map(|r| r.last_step + RELATIVE_STEP)
            .min();
        self.timers
            .iter()
            .map(|t| t.at)
            .chain(relative)
            .chain(self.motion.next_deadline())
            .min()
    }

    /// Records an input's state without running anything (initial positions for conditions).
    pub fn seed(&mut self, key: &InputKey, value: Value) {
        match value {
            Value::Axis(v) => {
                self.axes.insert(key.clone(), v);
            }
            Value::Button(b) => {
                self.buttons.insert(key.clone(), b);
            }
            Value::Hat(d) => {
                self.hats.insert(key.clone(), d);
            }
        }
    }

    /// Handles one physical input change.
    pub fn process(&mut self, key: &InputKey, value: Value, now: Instant, out: &mut dyn Output) {
        self.seed(key, value);
        let before = self.modes.current().to_string();
        self.dispatch(key, value, now, out);
        self.after_mode_change(&before, now, out);
    }

    /// Fires due timers (tempo thresholds, pulse releases).
    pub fn tick(&mut self, now: Instant, out: &mut dyn Output) {
        let before = self.modes.current().to_string();
        while let Some(pos) = self.timers.iter().position(|t| t.at <= now) {
            let timer = self.timers.remove(pos);
            match timer.kind {
                TimerKind::MacroStep(node) => self.macro_advance(node, now, out),
                TimerKind::TapTimeout(node) => self.tap_timeout(node, now, out),
                TimerKind::TempoTimeout(node) => self.tempo_timeout(node, now, out),
                TimerKind::ToggleTimeout(node) => {
                    let state = self.toggles.entry(node).or_default();
                    state.timer = None;
                    if state.phase == TogglePhase::Down {
                        state.phase = TogglePhase::Held;
                    }
                }
                TimerKind::PulseRelease { nodes, src } => {
                    for n in nodes {
                        self.run(n, &src, &mut Value::Button(false), now, out, 0);
                    }
                }
            }
        }
        self.step_relative(now, out);
        if let Some((dx, dy)) = self.motion.step(now) {
            if dx != 0 || dy != 0 {
                out.mouse_move(dx, dy);
            }
        }
        self.after_mode_change(&before, now, out);
    }

    fn step_relative(&mut self, now: Instant, out: &mut dyn Output) {
        for (&(device, axis), r) in self.relative.iter_mut().filter(|(_, r)| r.active) {
            let steps = now.saturating_duration_since(r.last_step).as_secs_f32()
                / RELATIVE_STEP.as_secs_f32();
            if steps < 1.0 {
                continue;
            }
            r.value = (r.value + r.delta * steps.floor()).clamp(-1.0, 1.0);
            r.last_step += RELATIVE_STEP.mul_f32(steps.floor());
            out.axis(device, axis, r.value);
            self.vjoy_axes.insert((device, axis), r.value);
            if r.stopping && now.saturating_duration_since(r.last_update) > RELATIVE_STOP {
                r.active = false;
            }
        }
    }

    /// Re-sends current axis positions in the new mode (Gremlin's refresh on mode change).
    fn after_mode_change(&mut self, before: &str, now: Instant, out: &mut dyn Output) {
        if self.modes.current() == before {
            return;
        }
        // Stop cursor motion whose input is bound differently in the new mode.
        let (lookup, current) = (&self.lookup, self.modes.current().to_string());
        self.motion.retain(|(_, key), mode| {
            lookup
                .get(key)
                .is_some_and(|m| m.get(mode).is_some() && m.get(mode) == m.get(&current))
        });
        let axes: Vec<(InputKey, f32)> = self.axes.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for (key, v) in axes {
            self.dispatch(&key, Value::Axis(v), now, out);
        }
    }

    fn dispatch(&mut self, key: &InputKey, value: Value, now: Instant, out: &mut dyn Output) {
        let src = Source {
            key: key.clone(),
            mode: self.modes.current().to_string(),
            no_auto_release: false,
        };
        if let Some(&set) = self.lookup.get(key).and_then(|m| m.get(&src.mode)) {
            for ci in 0..self.sets[set].len() {
                if self.paused && !self.sets[set][ci].always {
                    continue;
                }
                let (root, virtual_id) = (self.sets[set][ci].root, self.sets[set][ci].virtual_id);
                let states = match self.sets[set][ci].vbutton.as_mut() {
                    None => {
                        self.run(root, &src, &mut value.clone(), now, out, 0);
                        continue;
                    }
                    Some(vb) => vb.feed(value),
                };
                let vsrc = Source {
                    key: InputKey {
                        device: VIRTUAL_DEVICE.into(),
                        kind: InputKind::Button,
                        id: virtual_id,
                    },
                    mode: src.mode.clone(),
                    no_auto_release: false,
                };
                for pressed in states {
                    self.run(root, &vsrc, &mut Value::Button(pressed), now, out, 0);
                    self.process_release(&vsrc.key, pressed, now, out);
                }
            }
        }
        match value {
            Value::Button(pressed) => self.process_release(key, pressed, now, out),
            Value::Hat(None) => self.process_release(key, false, now, out),
            _ => {}
        }
    }

    fn run(
        &mut self,
        node: NodeId,
        src: &Source,
        value: &mut Value,
        now: Instant,
        out: &mut dyn Output,
        depth: usize,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        match self.nodes[node].clone() {
            Node::Seq(children) => {
                for c in children {
                    self.run(c, src, value, now, out, depth + 1);
                }
            }
            Node::Vjoy {
                device,
                target,
                inverted,
                act,
                relative,
            } => {
                if !should_execute(act, *value) {
                    return;
                }
                if let (VjoyTarget::Axis(id), Some(speed), Value::Axis(v)) =
                    (target, relative, *value)
                {
                    let r = self.relative.entry((device, id)).or_insert(RelativeAxis {
                        value: 0.0,
                        delta: 0.0,
                        active: false,
                        stopping: false,
                        last_update: now,
                        last_step: now,
                    });
                    r.delta = v * speed * 0.001;
                    r.stopping = v.abs() < 0.05;
                    r.last_update = now;
                    if !r.active {
                        r.active = true;
                        r.last_step = now;
                    }
                    return;
                }
                match target {
                    VjoyTarget::Axis(id) => match *value {
                        Value::Axis(v) => self.set_axis(device, id, v, out),
                        Value::Button(b) => {
                            self.set_axis(device, id, if b { 1.0 } else { 0.0 }, out)
                        }
                        Value::Hat(_) => {}
                    },
                    VjoyTarget::Button(id) => {
                        let pressed = match *value {
                            Value::Button(b) => b,
                            Value::Axis(v) => v != 0.0,
                            Value::Hat(d) => d.is_some(),
                        } != inverted;
                        out.button(device, id, pressed);
                        if pressed && !src.no_auto_release {
                            self.register(
                                &src.key,
                                ReleaseEntry {
                                    mode: src.mode.clone(),
                                    any_mode: false,
                                    on_press: inverted,
                                    action: ReleaseAction::VjoyButton(device, id),
                                },
                            );
                        }
                    }
                    VjoyTarget::Hat(id) => {
                        if let Value::Hat(d) = *value {
                            out.hat(device, id, d);
                        }
                    }
                }
            }
            Node::ChangeMode { kind, targets, act } => {
                if !should_execute(act, *value) && !matches!(kind, ChangeKind::Temporary) {
                    return;
                }
                let first = targets.first().cloned();
                match kind {
                    ChangeKind::Switch => {
                        if let Some(t) = first {
                            self.modes.switch_to(&t, false);
                        }
                    }
                    ChangeKind::Previous => self.modes.previous(),
                    ChangeKind::Unwind => self.modes.unwind(),
                    ChangeKind::Cycle => {
                        if !targets.is_empty() {
                            let i = self.cycle.entry(node).or_insert(usize::MAX);
                            *i = i.wrapping_add(1) % targets.len();
                            let target = targets[*i].clone();
                            self.modes.switch_to(&target, false);
                        }
                    }
                    ChangeKind::Temporary => {
                        let Some(t) = first else { return };
                        if value.truthy() {
                            self.modes.switch_to(&t, true);
                            self.register(
                                &src.key,
                                ReleaseEntry {
                                    mode: src.mode.clone(),
                                    any_mode: true,
                                    on_press: false,
                                    action: ReleaseAction::LeaveTemporary(t),
                                },
                            );
                        } else {
                            self.leave_temporary(&t);
                        }
                    }
                }
            }
            Node::Curve { deadzone, points } => {
                if let Value::Axis(v) = value {
                    *v = curve(&points, apply_deadzone(*v, deadzone));
                }
            }
            Node::Tempo { .. } => self.tempo_event(node, src, *value, now, out),
            Node::DoubleTap { .. } => self.tap_event(node, src, *value, now, out),
            Node::Chain { chains, timeout } => {
                if chains.is_empty() {
                    return;
                }
                let state = self.chains.entry(node).or_default();
                if !timeout.is_zero() {
                    if state.last.is_some_and(|t| t + timeout < now) {
                        state.index = 0;
                    }
                    state.last = Some(now);
                }
                let index = state.index.min(chains.len() - 1);
                for &c in &chains[index] {
                    self.run(c, src, value, now, out, depth + 1);
                }
                if !value.truthy() {
                    self.chains.entry(node).or_default().index = (index + 1) % chains.len();
                }
            }
            Node::SmartToggle { children, delay } => {
                if let Value::Button(pressed) = *value {
                    self.smart_toggle(node, &children, delay, src, pressed, now, out);
                }
            }
            Node::Split {
                split,
                lower,
                upper,
            } => {
                if let Value::Axis(v) = *value {
                    let (scaled, nodes) = if v < split {
                        (-axis_interpolation(v, -1.0, split), lower)
                    } else {
                        (axis_interpolation(v, split, 1.0), upper)
                    };
                    *value = Value::Axis(scaled);
                    for n in nodes {
                        self.run(n, src, value, now, out, depth + 1);
                    }
                }
            }
            Node::Pause(op) => {
                if value.truthy() {
                    self.paused = match op {
                        PauseOp::Pause => true,
                        PauseOp::Resume => false,
                        PauseOp::Toggle => !self.paused,
                    };
                }
            }
            Node::Run {
                executable,
                arguments,
                act,
            } => {
                if should_execute(act, *value) && !executable.trim().is_empty() {
                    out.run_command(&executable, &arguments);
                }
            }
            Node::Mouse { action, act } => {
                if should_execute(act, *value) {
                    self.mouse(node, action, src, *value, now, out);
                }
            }
            Node::Keys { keys, act } => {
                if !should_execute(act, *value) {
                    return;
                }
                if value.truthy() {
                    for k in &keys {
                        out.key(k.scan_code, k.extended, true);
                    }
                    self.keys_down.insert(node);
                    // Not in Gremlin: release the keys if the input is let go in another mode.
                    self.register(
                        &src.key,
                        ReleaseEntry {
                            mode: src.mode.clone(),
                            any_mode: false,
                            on_press: false,
                            action: ReleaseAction::ReleaseKeys(node),
                        },
                    );
                } else {
                    self.release_keys(node, out);
                }
            }
            Node::LoadProfile { file, act } => {
                if should_execute(act, *value) && !file.is_empty() {
                    log::info!("load-profile action: {file}");
                    out.load_profile(&file);
                }
            }
            Node::PlaySound { file, volume, act } => {
                if should_execute(act, *value) && !file.is_empty() {
                    out.play_sound(&file, volume);
                }
            }
            Node::Speak { mut request, act } => {
                if should_execute(act, *value) && !request.text.trim().is_empty() {
                    let mode = self.modes.current();
                    request.text = request
                        .text
                        .replace("${current_mode}", mode)
                        .replace("$current_mode", mode);
                    out.speak(request);
                }
            }
            Node::Condition {
                all,
                conditions,
                branches,
                act,
            } => {
                if !should_execute(act, *value) {
                    return;
                }
                let truth = |e: &Self, value: Value| {
                    let mut outcomes = conditions.iter().map(|c| e.cond_holds(c, value, out));
                    usize::from(!if all {
                        outcomes.all(|o| o)
                    } else {
                        outcomes.any(|o| o)
                    })
                };
                let branch = match *value {
                    Value::Axis(_) => truth(self, *value),
                    Value::Button(true) => {
                        let b = truth(self, *value);
                        self.cond_cache.insert(node, b);
                        b
                    }
                    // Release and hat center go where the press went (Gremlin's functor cache).
                    Value::Button(false) | Value::Hat(None) => self
                        .cond_cache
                        .remove(&node)
                        .unwrap_or_else(|| truth(self, *value)),
                    Value::Hat(Some(_)) => {
                        let b = truth(self, *value);
                        if let Some(prev) = self.cond_cache.insert(node, b) {
                            if prev != b {
                                for &c in &branches[prev] {
                                    self.run(c, src, &mut Value::Hat(None), now, out, depth + 1);
                                }
                            }
                        }
                        b
                    }
                };
                for &c in &branches[branch] {
                    self.run(c, src, value, now, out, depth + 1);
                }
            }
            Node::DualDeadzone {
                axes,
                inner,
                outer,
                outputs,
            } => {
                let [x, y] = axes.map(|k| self.axes.get(&k).copied().unwrap_or(0.0));
                if let Some(results) = dual_deadzone(x, y, inner, outer) {
                    for (v, nodes) in results.into_iter().zip(outputs) {
                        let mut v = Value::Axis(v);
                        for n in nodes {
                            self.run(n, src, &mut v, now, out, depth + 1);
                        }
                    }
                }
            }
            Node::AxisDelta {
                threshold,
                positive,
                negative,
            } => {
                // Gremlin reads the raw input, not the value earlier actions changed.
                let raw = match (self.axes.get(&src.key), *value) {
                    (Some(&v), _) | (None, Value::Axis(v)) => v,
                    _ => return,
                };
                let state = self.deltas.entry(node).or_insert((None, 0.0));
                let Some(last) = state.0 else {
                    state.0 = Some(raw);
                    return;
                };
                // Gremlin skips an exact 0.0 (Python falsiness) without updating the last value.
                if raw == 0.0 {
                    return;
                }
                state.0 = Some(raw);
                state.1 += raw - last;
                let nodes = if state.1 >= threshold {
                    positive
                } else if state.1 <= -threshold {
                    negative
                } else {
                    return;
                };
                state.1 = 0.0;
                let psrc = Source {
                    no_auto_release: true,
                    ..src.clone()
                };
                self.pulse(&nodes, Value::Button(true), psrc, now, out);
            }
            Node::Merge { axes, op, children } => {
                let [a, b] = axes.map(|k| self.axes.get(&k).copied().unwrap_or(0.0));
                *value = Value::Axis(op.apply(a, b));
                for c in children {
                    self.run(c, src, value, now, out, depth + 1);
                }
            }
            Node::HatButtons(buttons) => {
                if let Value::Hat(d) = *value {
                    self.hat_buttons_event(node, &buttons, src, d, now, out, depth);
                }
            }
            Node::Macro { repeat, act, .. } => {
                if should_execute(act, *value) {
                    self.macro_trigger(node, repeat, src, now, out);
                }
            }
            Node::Noop => {}
        }
    }

    /// Releases a map-to-keyboard action's keys in reverse order, once per press.
    fn release_keys(&mut self, node: NodeId, out: &mut dyn Output) {
        if !self.keys_down.remove(&node) {
            return;
        }
        if let Node::Keys { keys, .. } = &self.nodes[node] {
            for k in keys.iter().rev() {
                out.key(k.scan_code, k.extended, false);
            }
        }
    }

    fn set_axis(&mut self, device: u32, axis: u8, value: f32, out: &mut dyn Output) {
        out.axis(device, axis, value);
        self.vjoy_axes.insert((device, axis), value);
    }

    /// Gremlin's MacroFunctor + MacroManager.queue_macro.
    fn macro_trigger(
        &mut self,
        node: NodeId,
        repeat: MacroRepeat,
        src: &Source,
        now: Instant,
        out: &mut dyn Output,
    ) {
        match self.macros.get_mut(&node) {
            // A second activation stops a running toggle macro.
            Some(run) if matches!(repeat, MacroRepeat::Toggle { .. }) => {
                run.stop = true;
                run.queued = 0;
                return;
            }
            Some(run) => run.queued += 1,
            None => {
                self.macros.insert(node, MacroRun::default());
                self.macro_advance(node, now, out);
            }
        }
        if matches!(repeat, MacroRepeat::Hold { .. }) {
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: true,
                    on_press: false,
                    action: ReleaseAction::StopMacro(node),
                },
            );
        }
    }

    /// Executes steps until the next pause, the end of the macro or its termination.
    fn macro_advance(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let Node::Macro { steps, repeat, .. } = &self.nodes[node] else {
            return;
        };
        let (steps, repeat) = (steps.clone(), *repeat);
        loop {
            let Some(run) = self.macros.get_mut(&node) else {
                return;
            };
            if run.between_rounds {
                run.between_rounds = false;
                let more = match repeat {
                    MacroRepeat::Single => false,
                    MacroRepeat::Count { count, .. } => run.rounds < count,
                    MacroRepeat::Hold { .. } | MacroRepeat::Toggle { .. } => true,
                };
                if run.stop || !more {
                    self.macro_finish(node, now, out);
                    return;
                }
            }
            let Some(&step) = steps.get(run.pos) else {
                let delay = match repeat {
                    MacroRepeat::Single => {
                        self.macro_finish(node, now, out);
                        return;
                    }
                    MacroRepeat::Count { delay, .. }
                    | MacroRepeat::Hold { delay }
                    | MacroRepeat::Toggle { delay } => delay,
                };
                run.pos = 0;
                run.rounds += 1;
                run.between_rounds = true;
                let wait = Duration::from_secs_f64(delay.max(0.0)).max(MACRO_MIN_REPEAT);
                self.add_timer(now + wait, TimerKind::MacroStep(node));
                return;
            };
            run.pos += 1;
            if let MacroStep::Pause(d) = step {
                self.add_timer(
                    now + Duration::from_secs_f64(d.max(0.0)),
                    TimerKind::MacroStep(node),
                );
                return;
            }
            self.macro_step(step, out);
        }
    }

    /// Ends a run and starts the next queued activation, if any.
    fn macro_finish(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let queued = self.macros.remove(&node).map_or(0, |r| r.queued);
        if queued > 0 {
            self.macros.insert(
                node,
                MacroRun {
                    queued: queued - 1,
                    ..MacroRun::default()
                },
            );
            self.macro_advance(node, now, out);
        }
    }

    fn macro_step(&mut self, step: MacroStep, out: &mut dyn Output) {
        let id = |n: u32| n.min(255) as u8;
        match step {
            MacroStep::Key {
                scan_code,
                extended,
                pressed,
            } => out.key(scan_code, extended, pressed),
            // Wheel steps scroll one notch whatever their pressed flag says.
            MacroStep::MouseButton {
                button: MouseButton::WheelUp,
                ..
            } => out.mouse_wheel(1),
            MacroStep::MouseButton {
                button: MouseButton::WheelDown,
                ..
            } => out.mouse_wheel(-1),
            MacroStep::MouseButton { button, pressed } => out.mouse_button(button, pressed),
            MacroStep::MouseMotion { dx, dy } => out.mouse_move(dx, dy),
            MacroStep::VjoyButton {
                device,
                id: n,
                pressed,
            } => out.button(device, id(n), pressed),
            MacroStep::VjoyAxis {
                device,
                id: n,
                value,
                relative,
            } => {
                let mut v = value as f32;
                if relative {
                    v += self.vjoy_axes.get(&(device, id(n))).copied().unwrap_or(0.0);
                }
                self.set_axis(device, id(n), v.clamp(-1.0, 1.0), out);
            }
            MacroStep::VjoyHat {
                device,
                id: n,
                direction,
            } => out.hat(device, id(n), direction),
            MacroStep::Pause(_) | MacroStep::Other => {}
        }
    }

    fn mouse(
        &mut self,
        node: NodeId,
        action: MouseAction,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let MouseAction::Motion {
            direction,
            min_speed,
            max_speed,
            time_to_max_speed,
        } = action
        else {
            if let MouseAction::Button(button) = action {
                match button {
                    MouseButton::WheelUp if value.truthy() => out.mouse_wheel(1),
                    MouseButton::WheelDown if value.truthy() => out.mouse_wheel(-1),
                    MouseButton::WheelUp | MouseButton::WheelDown => {}
                    b => out.mouse_button(b, value.truthy()),
                }
            }
            return;
        };
        let key = (node, src.key.clone());
        match value {
            Value::Axis(v) => {
                // Gremlin maps the "vertical" heading 0 to 180 for axes: positive values move down.
                let heading = if direction == 0.0 { 180.0 } else { direction };
                let speed = if v.abs() < 1e-3 {
                    0.0
                } else {
                    (min_speed + v.abs() * (max_speed - min_speed)).copysign(v)
                };
                let (x, y) = compass(heading);
                self.motion
                    .set_velocity(key, (x * speed, y * speed), src.mode.clone(), now);
            }
            Value::Hat(Some(d)) => {
                let dir = compass(f32::from(d) / 100.0);
                self.motion.set_accelerated(
                    key,
                    dir,
                    min_speed,
                    max_speed,
                    time_to_max_speed,
                    src.mode.clone(),
                    now,
                );
            }
            Value::Button(true) => {
                let dir = compass(direction);
                self.motion.set_accelerated(
                    key,
                    dir,
                    min_speed,
                    max_speed,
                    time_to_max_speed,
                    src.mode.clone(),
                    now,
                );
            }
            Value::Hat(None) | Value::Button(false) => self.motion.clear(&key),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn smart_toggle(
        &mut self,
        node: NodeId,
        children: &[NodeId],
        delay: Duration,
        src: &Source,
        pressed: bool,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let inner = Source {
            no_auto_release: true,
            ..src.clone()
        };
        let phase = self.toggles.entry(node).or_default().phase;
        let (next, run) = match (phase, pressed) {
            (TogglePhase::Wait, true) => {
                let timer = self.add_timer(now + delay, TimerKind::ToggleTimeout(node));
                self.toggles.entry(node).or_default().timer = Some(timer);
                (TogglePhase::Down, true)
            }
            (TogglePhase::Down, false) => (TogglePhase::Latched, false),
            (TogglePhase::Held | TogglePhase::Latched, false) => (TogglePhase::Wait, true),
            (p, _) => (p, false),
        };
        self.toggles.entry(node).or_default().phase = next;
        if run {
            self.run_all(children, &inner, Value::Button(pressed), now, out);
        }
    }

    fn run_all(
        &mut self,
        nodes: &[NodeId],
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        for &n in nodes {
            self.run(n, src, &mut value.clone(), now, out, 0);
        }
    }

    fn leave_temporary(&mut self, mode: &str) {
        if self.modes.current() == mode && self.modes.is_temporary() {
            self.modes.unwind();
        }
    }

    fn tempo_event(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Value::Button(pressed) = value else {
            return;
        };
        let Node::Tempo {
            short,
            long,
            threshold,
            on_press,
        } = self.nodes[node].clone()
        else {
            return;
        };
        if pressed {
            self.tempo.entry(node).or_default().press = Some((value, src.clone()));
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: true,
                    on_press: false,
                    action: ReleaseAction::TempoRelease(node),
                },
            );
        }
        let phase = self.tempo.entry(node).or_default().phase;
        match (phase, pressed) {
            (Phase::Wait, true) => {
                let timer = self.add_timer(now + threshold, TimerKind::TempoTimeout(node));
                let state = self.tempo.entry(node).or_default();
                state.phase = Phase::Short;
                state.timer = Some(timer);
                if on_press {
                    self.run_all(&short, src, value, now, out);
                }
            }
            (Phase::Wait, false) => {}
            (Phase::Short, false) => {
                self.cancel_tempo_timer(node);
                self.tempo.entry(node).or_default().phase = Phase::Wait;
                if on_press {
                    self.run_all(&short, src, value, now, out);
                } else {
                    self.short_pulse(node, &short, now, out);
                }
            }
            (Phase::Long, false) => {
                self.tempo.entry(node).or_default().phase = Phase::Wait;
                self.run_all(&long, src, value, now, out);
                if on_press {
                    self.run_all(&short, src, value, now, out);
                }
            }
            // A press while already pressed: invalid transition, reset like Gremlin.
            (Phase::Short | Phase::Long, true) => self.tempo_reset(node, src, value, now, out),
        }
    }

    fn tempo_timeout(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let Node::Tempo { long, .. } = self.nodes[node].clone() else {
            return;
        };
        let state = self.tempo.entry(node).or_default();
        state.timer = None;
        if state.phase != Phase::Short {
            return;
        }
        state.phase = Phase::Long;
        if let Some((value, src)) = state.press.clone() {
            self.run_all(&long, &src, value, now, out);
        }
    }

    fn short_pulse(&mut self, node: NodeId, short: &[NodeId], now: Instant, out: &mut dyn Output) {
        if let Some((value, src)) = self.tempo.get(&node).and_then(|s| s.press.clone()) {
            self.pulse(short, value, src, now, out);
        }
    }

    /// Runs `nodes` with the press value now and with a release [`PULSE`] later.
    fn pulse(
        &mut self,
        nodes: &[NodeId],
        value: Value,
        src: Source,
        now: Instant,
        out: &mut dyn Output,
    ) {
        self.run_all(nodes, &src, value, now, out);
        self.add_timer(
            now + PULSE,
            TimerKind::PulseRelease {
                nodes: nodes.to_vec(),
                src,
            },
        );
    }

    /// One condition of a condition action (Gremlin's comparators on the source states).
    fn cond_holds(&self, cond: &Cond, value: Value, out: &dyn Output) -> bool {
        let states: Vec<Value> = match &cond.source {
            CondSource::Current => vec![value],
            CondSource::Inputs(keys) => keys
                .iter()
                .map(|k| match k.kind {
                    InputKind::Axis => Value::Axis(self.axes.get(k).copied().unwrap_or(0.0)),
                    InputKind::Button => {
                        Value::Button(self.buttons.get(k).copied().unwrap_or(false))
                    }
                    InputKind::Hat => Value::Hat(self.hats.get(k).copied().flatten()),
                })
                .collect(),
            CondSource::Keys(keys) => keys
                .iter()
                .map(|k| Value::Button(out.key_pressed(k.scan_code, k.extended)))
                .collect(),
            CondSource::Vjoy(device, kind, id) => {
                vec![out.vjoy_state(*device, *kind, *id).unwrap_or(match kind {
                    InputKind::Axis => Value::Axis(0.0),
                    InputKind::Button => Value::Button(false),
                    InputKind::Hat => Value::Hat(None),
                })]
            }
            CondSource::Never => return false,
        };
        compare(&cond.comparator, &states)
    }

    /// Gremlin's HatButtonsFunctor: every direction is a button pressed while the hat
    /// points exactly there (4-button mode ignores diagonals), centered presses `Center`.
    #[allow(clippy::too_many_arguments)]
    fn hat_buttons_event(
        &mut self,
        node: NodeId,
        buttons: &[(Option<u16>, Vec<NodeId>)],
        src: &Source,
        direction: Option<u16>,
        now: Instant,
        out: &mut dyn Output,
        depth: usize,
    ) {
        for (i, (dir, children)) in buttons.iter().enumerate() {
            let pressed = *dir == direction;
            let state = self
                .hat_buttons
                .entry(node)
                .or_insert_with(|| vec![false; buttons.len()]);
            if state[i] == pressed {
                continue;
            }
            state[i] = pressed;
            let vsrc = Source {
                key: hat_button_key(node, i),
                mode: src.mode.clone(),
                no_auto_release: src.no_auto_release,
            };
            for &c in children {
                self.run(c, &vsrc, &mut Value::Button(pressed), now, out, depth + 1);
            }
            self.process_release(&vsrc.key, pressed, now, out);
        }
        // Not in Gremlin: the hat centered in a mode where it is bound differently
        // still releases what the directions pressed.
        if direction.is_some() {
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: false,
                    on_press: false,
                    action: ReleaseAction::HatButtons(node),
                },
            );
        }
    }

    /// Gremlin's DoubleTapFunctor state machine.
    fn tap_event(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Value::Button(pressed) = value else {
            return;
        };
        let Node::DoubleTap {
            single,
            double,
            threshold,
            exclusive,
        } = self.nodes[node].clone()
        else {
            return;
        };
        if pressed {
            self.taps.entry(node).or_default().press = Some((value, src.clone()));
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: true,
                    on_press: false,
                    action: ReleaseAction::TapRelease(node),
                },
            );
        }
        use TapPhase::*;
        let phase = self.taps.entry(node).or_default().phase;
        let next = match (exclusive, phase, pressed) {
            (_, Neutral, true) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                let timer = self.add_timer(now + threshold, TimerKind::TapTimeout(node));
                self.taps.entry(node).or_default().timer = Some(timer);
                PressedTimed
            }
            (true, Neutral, false) => Neutral,
            (_, PressedTimed, false) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                Timed
            }
            (_, Pressed, false) => {
                self.run_all(&single, src, value, now, out);
                Neutral
            }
            (_, Timed, true) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                self.run_all(&double, src, value, now, out);
                SecondTimed
            }
            (_, SecondTimed, false) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                self.run_all(&double, src, value, now, out);
                Neutral
            }
            // Invalid transition: reset like Gremlin.
            _ => {
                self.tap_reset(node, src, value, now, out);
                return;
            }
        };
        self.taps.entry(node).or_default().phase = next;
    }

    fn tap_timeout(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let Node::DoubleTap {
            single, exclusive, ..
        } = self.nodes[node].clone()
        else {
            return;
        };
        let state = self.taps.entry(node).or_default();
        state.timer = None;
        let press = state.press.clone();
        // Timeouts without a transition are stale and ignored.
        let next = match state.phase {
            TapPhase::PressedTimed => {
                if let (true, Some((value, src))) = (exclusive, press) {
                    self.run_all(&single, &src, value, now, out);
                }
                TapPhase::Pressed
            }
            TapPhase::Timed => {
                if let (true, Some((value, src))) = (exclusive, press) {
                    self.pulse(&single, value, src, now, out);
                }
                TapPhase::Neutral
            }
            phase => phase,
        };
        self.taps.entry(node).or_default().phase = next;
    }

    fn tap_reset(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Node::DoubleTap { single, double, .. } = self.nodes[node].clone() else {
            return;
        };
        let state = self.taps.entry(node).or_default();
        state.phase = TapPhase::Neutral;
        if let Some(id) = state.timer.take() {
            self.timers.retain(|t| t.id != id);
        }
        let all: Vec<NodeId> = single.into_iter().chain(double).collect();
        self.run_all(&all, src, value, now, out);
    }

    fn tempo_reset(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Node::Tempo { short, long, .. } = self.nodes[node].clone() else {
            return;
        };
        self.cancel_tempo_timer(node);
        self.tempo.entry(node).or_default().phase = Phase::Wait;
        let all: Vec<NodeId> = short.into_iter().chain(long).collect();
        self.run_all(&all, src, value, now, out);
    }

    fn cancel_tempo_timer(&mut self, node: NodeId) {
        if let Some(id) = self.tempo.get_mut(&node).and_then(|s| s.timer.take()) {
            self.timers.retain(|t| t.id != id);
        }
    }

    fn add_timer(&mut self, at: Instant, kind: TimerKind) -> u64 {
        self.next_timer += 1;
        self.timers.push(Timer {
            id: self.next_timer,
            at,
            kind,
        });
        self.next_timer
    }

    fn register(&mut self, key: &InputKey, entry: ReleaseEntry) {
        let entries = self.releases.entry(key.clone()).or_default();
        // Gremlin keeps unfired entries forever; skip duplicates so the list stays bounded.
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }

    fn process_release(
        &mut self,
        key: &InputKey,
        pressed: bool,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Some(entries) = self.releases.remove(key) else {
            return;
        };
        let current = self.modes.current().to_string();
        let (fire, keep): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|e| (e.any_mode || e.mode != current) && pressed == e.on_press);
        if !keep.is_empty() {
            self.releases
                .entry(key.clone())
                .or_default()
                .splice(0..0, keep);
        }
        for entry in fire {
            match entry.action {
                ReleaseAction::VjoyButton(device, button) => out.button(device, button, false),
                ReleaseAction::LeaveTemporary(mode) => self.leave_temporary(&mode),
                ReleaseAction::ReleaseKeys(node) => self.release_keys(node, out),
                ReleaseAction::HatButtons(node) => {
                    let Some(state) = self.hat_buttons.get_mut(&node) else {
                        continue;
                    };
                    let held: Vec<usize> = (0..state.len()).filter(|&i| state[i]).collect();
                    state.fill(false);
                    for i in held {
                        self.process_release(&hat_button_key(node, i), false, now, out);
                    }
                }
                ReleaseAction::TapRelease(node) => {
                    // Fallback when the release went elsewhere: only states that accept a press are valid.
                    let phase = self.taps.get(&node).map(|s| s.phase).unwrap_or_default();
                    if !matches!(phase, TapPhase::Neutral | TapPhase::Timed) {
                        let src = Source {
                            key: key.clone(),
                            mode: current.clone(),
                            no_auto_release: false,
                        };
                        self.tap_reset(node, &src, Value::Button(false), now, out);
                    }
                }
                ReleaseAction::StopMacro(node) => {
                    if let Some(run) = self.macros.get_mut(&node) {
                        run.stop = true;
                        run.queued = 0;
                    }
                }
                ReleaseAction::TempoRelease(node) => {
                    let phase = self.tempo.get(&node).map(|s| s.phase).unwrap_or_default();
                    if phase != Phase::Wait {
                        let src = Source {
                            key: key.clone(),
                            mode: current.clone(),
                            no_auto_release: false,
                        };
                        self.tempo_reset(node, &src, Value::Button(false), now, out);
                    }
                }
            }
        }
    }
}

/// Gremlin's comparators. Python compares bools and floats as numbers (True == 1.0).
fn compare(comparator: &Comparator, states: &[Value]) -> bool {
    match comparator {
        Comparator::Range { lower, upper } => {
            let v = match states.first() {
                Some(Value::Axis(v)) => f64::from(*v),
                Some(Value::Button(b)) => f64::from(u8::from(*b)),
                _ => return false,
            };
            (*lower..=*upper).contains(&v)
        }
        Comparator::Pressed(p) => states.iter().all(|s| match *s {
            Value::Button(b) => b == *p,
            Value::Axis(v) => v == f32::from(u8::from(*p)),
            Value::Hat(_) => false,
        }),
        Comparator::Direction(dirs) => {
            matches!(states.first(), Some(Value::Hat(d)) if dirs.contains(d))
        }
    }
}

/// Gremlin's DualAxisDeadzoneFunctor: the inner deadzone is a circle (its x/y share
/// follows the stick angle), the outer one a square. `None` when the limits coincide
/// (Gremlin's ZeroDivisionError: nothing is sent).
fn dual_deadzone(x: f32, y: f32, inner: f32, outer: f32) -> Option<[f32; 2]> {
    let alpha = y.atan2(x);
    let (lx, ly) = ((alpha.cos() * inner).abs(), (alpha.sin() * inner).abs());
    let (dx, dy) = (outer - lx, outer - ly);
    if dx == 0.0 || dy == 0.0 {
        return None;
    }
    let scale = |v: f32, low: f32, d: f32| ((v.abs() - low) / d).clamp(0.0, 1.0).copysign(v);
    Some([scale(x, lx, dx), scale(y, ly, dy)])
}

fn hat_button_key(node: NodeId, direction: usize) -> InputKey {
    InputKey {
        device: HAT_BUTTON_DEVICE.into(),
        kind: InputKind::Button,
        id: (node * 9 + direction) as u32,
    }
}

/// Inserts `delay` seconds between consecutive steps that are not pauses
/// (Gremlin's MacroManager._preprocess_macro).
fn with_default_delay(steps: &[MacroStep], delay: f64) -> Vec<MacroStep> {
    let pause = |s: &MacroStep| matches!(s, MacroStep::Pause(_));
    let mut out = Vec::with_capacity(steps.len() * 2);
    for (i, &step) in steps.iter().enumerate() {
        if i > 0 && !pause(&step) && !pause(&steps[i - 1]) {
            out.push(MacroStep::Pause(delay));
        }
        out.push(step);
    }
    out
}

fn should_execute(act: ActivationMode, value: Value) -> bool {
    match act {
        ActivationMode::Both | ActivationMode::Disallowed => true,
        ActivationMode::Press => value.truthy(),
        ActivationMode::Release => !value.truthy(),
    }
}

/// Every `<target-mode>` name of a change-mode action (several for Cycle).
fn target_modes(action: &Action) -> Vec<String> {
    action
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Group { tag, items, .. } if tag == "target-mode" => Some(items),
            _ => None,
        })
        .flatten()
        .filter_map(|item| match item {
            Item::Property(p) if p.name == "name" => Some(p.value.clone()),
            _ => None,
        })
        .collect()
}

/// Maps `value` from `low..=high` to -1..=1 (Gremlin's linear_axis_value_interpolation).
fn axis_interpolation(value: f32, low: f32, high: f32) -> f32 {
    if high - low <= f32::EPSILON {
        return 0.0;
    }
    (value.clamp(low, high) - low) / (high - low) * 2.0 - 1.0
}

/// Splits command-line arguments on whitespace, keeping double-quoted parts together.
fn split_command(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in args.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Gremlin's deadzone: `[low, center_low, center_high, high]`.
fn apply_deadzone(value: f32, [low, center_low, center_high, high]: [f32; 4]) -> f32 {
    if value >= 0.0 {
        ((value - center_high) / (high - center_high).abs().max(0.01)).clamp(0.0, 1.0)
    } else {
        ((value - center_low) / (low - center_low).abs().max(0.01)).clamp(-1.0, 0.0)
    }
}

/// Piecewise linear interpolation through sorted control points.
fn curve(points: &[(f32, f32)], x: f32) -> f32 {
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
        return x;
    };
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    points.windows(2).find(|w| x <= w[1].0).map_or(x, |w| {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x1 - x0 <= f32::EPSILON {
            y1
        } else {
            y0 + (x - x0) * (y1 - y0) / (x1 - x0)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_axis_deadzone() {
        let r =
            |x, y| dual_deadzone(x, y, 0.2, 0.9).map(|v| v.map(|a| (a * 1000.0).round() / 1000.0));
        assert_eq!(r(0.1, 0.1), Some([0.0, 0.0]), "inside the circle");
        assert_eq!(r(0.55, 0.0), Some([0.5, 0.0]));
        assert_eq!(r(-1.0, 0.95), Some([-1.0, 1.0]), "beyond the square");
        assert_eq!(dual_deadzone(1.0, 0.0, 0.5, 0.5), None);
    }

    #[test]
    fn deadzone_and_curve() {
        assert_eq!(apply_deadzone(0.5, [-1.0, 0.0, 0.0, 1.0]), 0.5);
        assert_eq!(apply_deadzone(0.05, [-1.0, -0.1, 0.1, 1.0]), 0.0);
        assert!((apply_deadzone(0.55, [-1.0, -0.1, 0.1, 1.0]) - 0.5).abs() < 1e-6);
        let pts = [(-1.0, -1.0), (0.0, 0.2), (1.0, 1.0)];
        assert!((curve(&pts, 0.0) - 0.2).abs() < 1e-6);
        assert!((curve(&pts, 0.5) - 0.6).abs() < 1e-6);
        assert_eq!(curve(&pts, 2.0), 1.0);
    }

    #[test]
    fn virtual_axis_button() {
        let mut vb = VButton::Axis {
            lower: 0.5,
            upper: 1.0,
            direction: AxisDirection::Anywhere,
            last: None,
            down: false,
        };
        assert!(vb.feed(Value::Axis(0.0)).is_empty());
        assert_eq!(vb.feed(Value::Axis(0.7)), [true]);
        assert!(vb.feed(Value::Axis(0.8)).is_empty());
        assert_eq!(vb.feed(Value::Axis(0.1)), [false]);
    }
}
