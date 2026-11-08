use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gob_core::{
    Action, ActionKind, ActivationMode, AxisDirection, AxisMotion, AxisOutput, ButtonAxis,
    ButtonSend, Comparator, ConditionSource, InputType, Item, Key, Layer, MacroRepeat, MacroStep,
    MergeOp, MouseAction, MouseButton, Profile, ProfileEvent, Selection, SendMode, VOICE_GUID,
    VOICE_INPUT_TYPE, VirtualButton,
};

use crate::media::{SpeechQueue, SpeechRequest};
use crate::modes::ModeStack;
use crate::motion::{Motion, compass};
use crate::{GamepadControl, InputKey, InputKind, Light, LogicalValues, Output, Value};
use gob_script::{
    EventValue, InputKind as ScriptKind, LoadedScript, ScriptApi, ScriptError, ScriptHost,
    ScriptInput,
};
use gob_virtual::ControlKind;

/// Delay between press and release of a pulse (Gremlin: 50 ms).
const PULSE: Duration = Duration::from_millis(50);
/// Guards against reference cycles in broken profiles.
const MAX_DEPTH: usize = 32;
const VIRTUAL_DEVICE: &str = "virtual";
/// Device of the virtual buttons a hat-buttons action creates, one per direction.
const HAT_BUTTON_DEVICE: &str = "virtual-hat";
/// Device of the button a run-action macro step presses, one per action it runs.
const RUN_ACTION_DEVICE: &str = "virtual-run";
/// Gremlin's default pause between macro steps without an explicit pause.
const MACRO_DELAY: f64 = 0.05;

/// The input of voice command `id`.
fn voice_key(id: u32) -> InputKey {
    InputKey::new(VOICE_GUID, InputKind::Button, id)
}

/// The input of a profile event.
fn event_key(event: ProfileEvent) -> InputKey {
    InputKey::new(gob_core::EVENTS_GUID, InputKind::Button, event.id())
}

/// How long a profile event holds its button down, like a short press.
const EVENT_HOLD: Duration = Duration::from_millis(100);
/// Mode changes made by mode enter and leave actions themselves that still run their own
/// events; a loop of modes switching each other stops there.
const MAX_MODE_EVENTS: u8 = 8;
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
        /// How a button target follows the input.
        send: ButtonSend,
    },
    /// voice-input: listens on `microphone` while held.
    Voice {
        microphone: String,
        act: ActivationMode,
    },
    /// map-to-gamepad.
    Gamepad {
        gamepad: u32,
        control: GamepadControl,
        inverted: bool,
        act: ActivationMode,
        /// How a button control follows the input.
        send: ButtonSend,
    },
    ChangeMode {
        kind: ChangeKind,
        targets: Vec<String>,
        act: ActivationMode,
    },
    Curve {
        deadzone: [f32; 4],
        /// Gremlin's three curve types, precomputed.
        curve: gob_core::curve::Curve,
    },
    Mouse {
        action: MouseAction,
        act: ActivationMode,
        /// How a button or the wheel follows the input.
        send: ButtonSend,
    },
    Keys {
        keys: Vec<Key>,
        act: ActivationMode,
        send: ButtonSend,
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
    /// Writes to a logical device input, which then runs its own bindings.
    Logical {
        key: InputKey,
        inverted: bool,
        /// Relative axis speed (Gremlin's axis-scaling); absolute when `None`.
        relative: Option<f32>,
        act: ActivationMode,
    },
    LoadProfile {
        file: String,
        act: ActivationMode,
    },
    PlaySound {
        file: String,
        volume: u8,
        act: ActivationMode,
    },
    /// light-led run by its input; the ones set on profile activation are in
    /// [`Config::lights`] instead.
    LightLed {
        light: Light,
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
        /// The actions its run-action steps may run, by id (see [`Engine::action_nodes`]).
        targets: Vec<String>,
    },
    /// button-axis: `up` for the input it is bound to, not for its decrease input.
    ButtonAxis {
        axis: ButtonAxis,
        target: AxisOut,
        up: bool,
    },
    /// Description and action types Joystick Gobloon does not execute yet.
    Noop,
}

/// An axis a button-axis action moves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AxisOut {
    Vjoy(u32, u8),
    Pad(u32, GamepadControl),
    Logical(InputKey),
}

impl AxisOut {
    /// Where an encoder's spring takes it back to without a start value: the center, or a
    /// trigger let go.
    fn rest(&self) -> f32 {
        match self {
            AxisOut::Pad(_, c) if c.kind() == ControlKind::Trigger => -1.0,
            _ => 0.0,
        }
    }
}

/// An axis moved by button-axis actions, kept per axis: a mode change does not lose it.
#[derive(Debug)]
struct AxisState {
    value: f32,
    /// The node whose settings move it now (the last one pressed).
    node: NodeId,
    /// Velocity: its up and down buttons held.
    held: [bool; 2],
    speed: f32,
    /// Smooth: where it heads.
    target: f32,
    /// Set: the value before the press, for going back on release.
    before: Option<f32>,
    last_press: Option<Instant>,
    /// Encoder: when the spring takes it back.
    spring_at: Option<Instant>,
    /// Velocity and smooth: moving, last moved at `last_step`.
    moving: bool,
    last_step: Instant,
}

/// How often moving button-axis axes are written.
const AXIS_STEP: Duration = Duration::from_millis(5);

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
    mode: Arc<str>,
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
    PulseRelease {
        nodes: Vec<NodeId>,
        src: Source,
    },
    /// A recognized voice command's button goes up.
    VoiceRelease(u32),
    /// A profile event's button (by its id) goes up.
    EventRelease(u32),
    /// The action a run-action macro step pressed is released.
    RunRelease {
        node: NodeId,
        src: Source,
    },
    /// A pulse of a send mode ends.
    SendUp(NodeId),
    /// The next pulse of a repeat.
    SendNext(NodeId),
}

/// What a button-like action presses.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Press {
    Vjoy(u32, u8),
    Pad(u32, GamepadControl),
    /// The keys of a map-to-keyboard action.
    Keys(NodeId),
    Mouse(MouseButton),
}

/// A button output with a [`SendMode`] other than hold.
#[derive(Debug, Default)]
struct SendState {
    /// The output is down (also the toggle's state).
    down: bool,
    /// Repeat runs.
    repeating: bool,
    /// What it pressed last, to release it from a timer.
    press: Option<Press>,
    /// The send settings of the running repeat.
    send: ButtonSend,
    /// The input is on (pressed, axis off center): only a change of it is a trigger, so an
    /// axis or hat that keeps moving does not trigger again.
    input: bool,
    up_timer: Option<u64>,
    next_timer: Option<u64>,
}

fn ms(ms: u32) -> Duration {
    Duration::from_millis(u64::from(ms))
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
    GamepadButton(u32, GamepadControl),
    LeaveTemporary(String),
    TempoRelease(NodeId),
    StopMacro(NodeId),
    TapRelease(NodeId),
    ReleaseKeys(NodeId),
    StopListening(NodeId),
    /// A repeat stops.
    StopSend(NodeId),
    /// A button-axis input went up in another mode.
    AxisRelease(NodeId),
    HatButtons(NodeId),
    /// Releases a logical device button.
    Logical(InputKey),
}

/// Runs when the physical input that registered it is released (Gremlin's ButtonReleaseActions).
#[derive(Debug, Clone, PartialEq)]
struct ReleaseEntry {
    mode: Arc<str>,
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

impl Config {
    /// The entry of a button-axis action's decrease input.
    fn decrease(root: NodeId, virtual_id: u32, layer: Layer, additive: bool) -> Self {
        Self {
            always: false,
            root,
            vbutton: None,
            virtual_id,
            lights: Vec::new(),
            layer,
            additive,
            keep_value: false,
            send_state: false,
        }
    }
}

struct Config {
    root: NodeId,
    /// Runs while paused: the root holds a pause-resume action.
    always: bool,
    vbutton: Option<VButton>,
    virtual_id: u32,
    /// light-led actions of this binding set when its mode becomes active.
    lights: Vec<Light>,
    /// Modifiers it needs (see `gob_core::modifier`).
    layer: Layer,
    additive: bool,
    /// Axis outputs keep their value when the layer stops.
    keep_value: bool,
    /// A button held when the profile starts runs its press then.
    send_state: bool,
}

pub struct Engine {
    /// Shared so running a node does not copy it (its child lists, curve points, keys).
    nodes: Vec<Arc<Node>>,
    tempo: HashMap<NodeId, TempoState>,
    cycle: HashMap<NodeId, usize>,
    sets: Vec<Vec<Config>>,
    lookup: HashMap<InputKey, HashMap<String, usize>>,
    modes: ModeStack,
    releases: HashMap<InputKey, Vec<ReleaseEntry>>,
    /// Entries [`Engine::process_release`] runs, kept to reuse their memory.
    release_buffer: Vec<ReleaseEntry>,
    timers: Vec<Timer>,
    next_timer: u64,
    axes: HashMap<InputKey, f32>,
    relative: HashMap<(u32, u8), RelativeAxis>,
    /// Cursor motion per (action, input), tagged with the mode it started in.
    motion: Motion<(NodeId, InputKey), Arc<str>>,
    toggles: HashMap<NodeId, ToggleState>,
    macros: HashMap<NodeId, MacroRun>,
    taps: HashMap<NodeId, TapState>,
    chains: HashMap<NodeId, ChainState>,
    /// Logical axes moved by speed (relative map-to-logical-device).
    logical_relative: HashMap<InputKey, RelativeAxis>,
    /// Nesting of logical device dispatches, against loops in broken profiles.
    logical_depth: usize,
    /// A logical device input changed since [`Engine::take_logical_changed`].
    logical_changed: bool,
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
    /// Mouse buttons the profile holds down.
    mouse_down: Vec<MouseButton>,
    /// Virtual stick and gamepad buttons the profile holds down (released by
    /// [`Engine::release_all`]).
    vjoy_down: Vec<(u32, u8)>,
    pad_down: Vec<(u32, GamepadControl)>,
    /// Last value written to each virtual gamepad stick and trigger.
    pad_axes: HashMap<(u32, GamepadControl), f32>,
    /// Button outputs with a send mode other than hold.
    sends: HashMap<NodeId, SendState>,
    /// voice-input actions held down, with their microphone.
    listening: HashMap<NodeId, String>,
    /// How long each voice command (by id) holds its button.
    voice_holds: HashMap<u32, Duration>,
    /// Release timer of each voice command whose button is down.
    voice_releases: HashMap<u32, u64>,
    /// Release timer of each profile event (by id) whose button is down, with the mode it runs
    /// in.
    event_releases: HashMap<u32, (u64, Arc<str>)>,
    /// Nesting of mode changes made by mode enter and leave actions.
    mode_event_depth: u8,
    /// Every phrase of the profile's voice commands with the command's id.
    voice_phrases: Vec<(u32, String)>,
    /// Profile's `macro-default-delay` in seconds.
    macro_delay: f64,
    /// Last value written to each vJoy axis, for relative macro steps.
    vjoy_axes: HashMap<(u32, u8), f32>,
    /// Paused by a pause-resume action: only bindings containing one still run.
    paused: bool,
    /// Profile device GUID -> attached device GUID, lowercase.
    aliases: HashMap<String, String>,
    /// On-activate light-led actions met while compiling the current binding.
    compiled_lights: Vec<Light>,
    /// Modifier inputs and their names; they only choose layers, their bindings do not run.
    modifiers: Vec<(InputKey, String)>,
    /// Layer a button or hat chose when pressed; its release goes there too.
    pressed_layers: HashMap<InputKey, Layer>,
    /// Axes moved by button-axis actions.
    button_axes: HashMap<AxisOut, AxisState>,
    /// Writes of moving axes, kept to reuse their memory.
    axis_writes: Vec<(AxisOut, f32)>,
    /// Decrease inputs of the button-axis actions met while compiling the current binding,
    /// with the action's id.
    compiled_decrease: Vec<(gob_core::DecreaseInput, String)>,
    /// While compiling a decrease input's tree: the button-axis action it runs. Other actions
    /// compile to nothing, the containers on the way keep their logic.
    prune: Option<String>,
    /// Start values are written once, when the profile starts.
    started: bool,
    /// The node of each action as its own input runs it (the first binding that has it),
    /// which run-action macro steps run too, so both share its state.
    action_nodes: HashMap<String, NodeId>,
    /// Per macro action id, the targets its run-action steps may run: actions that take a
    /// button press and do not use the macro themselves (no loops).
    run_targets: HashMap<String, Vec<String>>,
    /// Release timer of each action a run-action step holds pressed.
    run_releases: HashMap<NodeId, u64>,
    /// The profile's Lua scripts; `None` without any.
    scripts: Option<ScriptHost>,
    /// Inputs a script handler listens to, with their index in the host: other events never
    /// reach the scripts.
    script_keys: HashMap<InputKey, usize>,
    /// The profile's modes, for scripts' `mode.switch`.
    script_modes: HashSet<String>,
    /// Keys and virtual stick hats scripts hold, released by [`Engine::release_all`].
    script_keys_down: Vec<Key>,
    script_hats: Vec<(u32, u8)>,
    /// Action names (card titles) to their ids, for scripts' `run_action`.
    action_labels: HashMap<String, String>,
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
    /// Action types Joystick Gobloon does not know are kept and do nothing.
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
            release_buffer: Vec::new(),
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
            logical_relative: HashMap::new(),
            logical_depth: 0,
            logical_changed: false,
            cond_cache: HashMap::new(),
            keys_down: HashSet::new(),
            mouse_down: Vec::new(),
            vjoy_down: Vec::new(),
            pad_down: Vec::new(),
            pad_axes: HashMap::new(),
            sends: HashMap::new(),
            listening: HashMap::new(),
            voice_holds: profile
                .voice_commands
                .iter()
                .map(|c| (c.id, Duration::from_secs_f64(c.hold.clamp(0.01, 10.0))))
                .collect(),
            voice_releases: HashMap::new(),
            event_releases: HashMap::new(),
            mode_event_depth: 0,
            voice_phrases: profile
                .voice_phrases()
                .into_iter()
                .map(|(id, p)| (id, p.to_string()))
                .collect(),
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
            compiled_lights: Vec::new(),
            modifiers: Vec::new(),
            pressed_layers: HashMap::new(),
            button_axes: HashMap::new(),
            axis_writes: Vec::new(),
            compiled_decrease: Vec::new(),
            prune: None,
            started: false,
            action_nodes: HashMap::new(),
            run_targets: HashMap::new(),
            run_releases: HashMap::new(),
            scripts: None,
            script_keys: HashMap::new(),
            script_modes: HashSet::new(),
            script_keys_down: Vec::new(),
            script_hats: Vec::new(),
            action_labels: HashMap::new(),
        };
        for m in profile.library.iter().filter(|a| a.kind == "macro") {
            let targets = m.run_targets();
            if targets.is_empty() {
                continue;
            }
            let users = profile.reaching(&m.id);
            let mut allowed: Vec<String> = Vec::new();
            for t in targets {
                let ok = !users.contains(t) && index.get(t).is_some_and(|a| a.runs_on_button());
                if ok && !allowed.iter().any(|a| a == t) {
                    allowed.push(t.to_string());
                }
            }
            engine.run_targets.insert(m.id.clone(), allowed);
        }
        for m in &profile.modifiers {
            let kind = match m.input_type.trim() {
                "button" | "key" => InputKind::Button,
                _ => continue,
            };
            if let Ok(id) = m.input_id.trim().parse() {
                let key = InputKey {
                    device: engine.device(&m.device_id),
                    kind,
                    id,
                };
                engine.modifiers.push((key, m.name.clone()));
            }
        }
        let mut virtual_id = 0;
        // Each decrease input's entry: (its input, the action's input, mode, entry), for the
        // child modes that bind the decrease input themselves (below).
        let mut decreases: Vec<(InputKey, InputKey, String, Config)> = Vec::new();
        for b in &profile.inputs {
            let kind = match b.input_type() {
                InputType::Axis => InputKind::Axis,
                InputType::Button => InputKind::Button,
                InputType::Hat => InputKind::Hat,
                // Keyboard keys (Gremlin's "key" inputs), voice commands and profile events
                // behave like buttons.
                InputType::Other
                    if matches!(
                        b.input_type.trim(),
                        "key" | VOICE_INPUT_TYPE | gob_core::EVENT_INPUT_TYPE
                    ) =>
                {
                    InputKind::Button
                }
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
                let always = matches!(&*engine.nodes[root], Node::Seq(c) if c.iter().any(|&n| matches!(*engine.nodes[n], Node::Pause(_) | Node::LoadProfile { .. })));
                engine.sets[set].push(Config {
                    always,
                    root,
                    vbutton,
                    virtual_id,
                    lights: std::mem::take(&mut engine.compiled_lights),
                    layer: cfg.layer.clone(),
                    additive: cfg.additive,
                    keep_value: cfg.keep_value,
                    send_state: cfg.send_state,
                });
                // A button-axis action's decrease input runs it too, in the same mode and layer,
                // through copies of the containers it sits in (their own state).
                for (input, action) in std::mem::take(&mut engine.compiled_decrease) {
                    engine.prune = Some(action);
                    let node = engine.compile(&index, &cfg.root_action, 0);
                    engine.prune = None;
                    let down = InputKey {
                        device: engine.device(&input.device),
                        kind: InputKind::Button,
                        id: input.id,
                    };
                    virtual_id += 1;
                    let entry = Config::decrease(node, virtual_id, cfg.layer.clone(), cfg.additive);
                    let modes = engine.lookup.entry(down.clone()).or_default();
                    let set = *modes.entry(b.mode.clone()).or_insert_with(|| {
                        engine.sets.push(Vec::new());
                        engine.sets.len() - 1
                    });
                    engine.sets[set].push(entry);
                    let entry = Config::decrease(node, virtual_id, cfg.layer.clone(), cfg.additive);
                    decreases.push((down, key.clone(), b.mode.clone(), entry));
                }
            }
        }
        // Run-action targets no input runs (kept only by the macros) get a node of their own;
        // their decrease inputs and start lights have no binding to go to.
        let mut unbound: Vec<String> = engine
            .run_targets
            .values()
            .flatten()
            .filter(|t| !engine.action_nodes.contains_key(*t))
            .cloned()
            .collect();
        unbound.sort_unstable();
        unbound.dedup();
        for t in unbound {
            if !engine.action_nodes.contains_key(&t) {
                engine.compile(&index, &t, 0);
            }
        }
        engine.compiled_decrease.clear();
        engine.compiled_lights.clear();
        engine.inherit_modes(profile);
        engine.load_scripts(profile);
        // A child mode that binds a decrease input itself has a set of its own for it, so it
        // does not inherit the decrease entry: it gets one in every mode that inherits the
        // action's own input, as the action runs there too.
        for (down, up, mode, entry) in decreases {
            let Some(up_modes) = engine.lookup.get(&up) else {
                continue;
            };
            let Some(&up_set) = up_modes.get(&mode) else {
                continue;
            };
            let inheriting: Vec<String> = up_modes
                .iter()
                .filter(|&(m, &set)| set == up_set && *m != mode)
                .map(|(m, _)| m.clone())
                .collect();
            let down_modes = engine.lookup.entry(down).or_default();
            let own = down_modes.get(&mode).copied();
            for m in inheriting {
                let set = *down_modes.entry(m).or_insert_with(|| {
                    engine.sets.push(Vec::new());
                    engine.sets.len() - 1
                });
                if Some(set) != own {
                    let copy = Config::decrease(
                        entry.root,
                        entry.virtual_id,
                        entry.layer.clone(),
                        entry.additive,
                    );
                    engine.sets[set].push(copy);
                }
            }
        }
        engine
    }

    /// Attached device for a profile device GUID.
    fn device(&self, guid: &str) -> Arc<str> {
        let guid = guid.trim().to_ascii_lowercase();
        self.aliases.get(&guid).cloned().unwrap_or(guid).into()
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
        let pruned = self.prune.as_deref().is_some_and(|target| {
            index
                .get(id)
                .is_some_and(|a| a.id != target && !is_container(a))
        });
        let node = match index.get(id).filter(|_| depth < MAX_DEPTH && !pruned) {
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
                        send: action.button_send(),
                    })
                }
                ActionKind::MapToGamepad {
                    gamepad,
                    control,
                    inverted,
                } => GamepadControl::parse(control).map_or(Node::Noop, |control| Node::Gamepad {
                    gamepad,
                    control,
                    inverted,
                    act: action.activation_mode(),
                    send: action.button_send(),
                }),
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
                    curve_type,
                    deadzone,
                    points,
                } => Node::Curve {
                    deadzone: deadzone.map(|d| d as f32),
                    curve: gob_core::curve::Curve::new(
                        gob_core::curve::CurveType::parse(curve_type),
                        &points,
                    ),
                },
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
                ActionKind::MapToLogicalDevice {
                    input_type,
                    id,
                    inverted,
                    relative,
                } => {
                    let kind = match input_type {
                        InputType::Axis => Some(InputKind::Axis),
                        InputType::Button => Some(InputKind::Button),
                        InputType::Hat => Some(InputKind::Hat),
                        InputType::Other => None,
                    };
                    kind.map_or(Node::Noop, |kind| Node::Logical {
                        key: InputKey {
                            device: gob_core::LOGICAL_DEVICE_GUID.into(),
                            kind,
                            id,
                        },
                        inverted,
                        relative: relative.map(|r| r as f32),
                        act: action.activation_mode(),
                    })
                }
                ActionKind::LoadProfile { file } => Node::LoadProfile {
                    file: file.to_string(),
                    act: action.activation_mode(),
                },
                ActionKind::PlaySound { file, volume } => Node::PlaySound {
                    file: file.to_string(),
                    volume,
                    act: action.activation_mode(),
                },
                ActionKind::LightLed {
                    device,
                    led,
                    element,
                    color,
                    color2,
                    blend,
                    mode,
                    on_activate,
                } => {
                    let light = Light {
                        device: self.device(device).to_string(),
                        led,
                        element,
                        color,
                        color2,
                        blend,
                        mode,
                    };
                    if light.device.is_empty() {
                        Node::Noop
                    } else if on_activate {
                        self.compiled_lights.push(light);
                        Node::Noop
                    } else {
                        Node::LightLed {
                            light,
                            act: action.activation_mode(),
                        }
                    }
                }
                ActionKind::TextToSpeech {
                    text,
                    voice,
                    queue_mode,
                    rate,
                    volume,
                    pitch,
                } => Node::Speak {
                    request: SpeechRequest {
                        text: text.to_string(),
                        voice: voice.map(String::from),
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
                                ConditionSource::Logical { input_type, id } => {
                                    let kind = match input_type {
                                        InputType::Axis => InputKind::Axis,
                                        InputType::Hat => InputKind::Hat,
                                        _ => InputKind::Button,
                                    };
                                    CondSource::Inputs(vec![InputKey {
                                        device: gob_core::LOGICAL_DEVICE_GUID.into(),
                                        kind,
                                        id,
                                    }])
                                }
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
                    send: action.button_send(),
                },
                ActionKind::MapToMouse(action_kind) => Node::Mouse {
                    action: action_kind,
                    act: action.activation_mode(),
                    send: action.button_send(),
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
                    targets: self.run_targets.get(id).cloned().unwrap_or_default(),
                },
                ActionKind::VoiceInput { microphone } => Node::Voice {
                    microphone: microphone.to_string(),
                    act: action.activation_mode(),
                },
                ActionKind::ButtonAxis(axis) => self.button_axis_node(axis, &action.id),
                ActionKind::Other | ActionKind::Description { .. } => Node::Noop,
            },
        };
        self.nodes.push(Arc::new(node));
        let node = self.nodes.len() - 1;
        if self.prune.is_none() && index.contains_key(id) && !self.action_nodes.contains_key(id) {
            self.action_nodes.insert(id.to_string(), node);
        }
        node
    }

    /// button-axis: its node, the decrease one while compiling the decrease input's tree;
    /// otherwise the decrease input goes to `compiled_decrease`.
    fn button_axis_node(&mut self, axis: ButtonAxis, id: &str) -> Node {
        let target = match &axis.output {
            AxisOutput::Vjoy { device, axis } => AxisOut::Vjoy(*device, (*axis).min(255) as u8),
            AxisOutput::Gamepad { gamepad, control } => match GamepadControl::parse(control) {
                Some(c) if matches!(c.kind(), ControlKind::Stick | ControlKind::Trigger) => {
                    AxisOut::Pad(*gamepad, c)
                }
                _ => return Node::Noop,
            },
            AxisOutput::Logical { id } => AxisOut::Logical(InputKey {
                device: gob_core::LOGICAL_DEVICE_GUID.into(),
                kind: InputKind::Axis,
                id: *id,
            }),
        };
        // Compiling the decrease input's tree: this is the node it reaches.
        if self.prune.as_deref() == Some(id) {
            return Node::ButtonAxis {
                axis,
                target,
                up: false,
            };
        }
        if self.prune.is_none()
            && let Some(input) = axis.decrease.clone()
            && matches!(input.input_type.as_str(), "button" | "key")
        {
            self.compiled_decrease.push((input, id.to_string()));
        }
        Node::ButtonAxis {
            axis,
            target,
            up: true,
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn current_mode(&self) -> &str {
        self.modes.current()
    }

    /// Whether a logical device input changed since the last call.
    pub fn take_logical_changed(&mut self) -> bool {
        std::mem::take(&mut self.logical_changed)
    }

    /// Current values of the logical device's inputs (input viewer).
    pub fn logical_values(&self) -> LogicalValues {
        fn of<T: Copy>(map: &HashMap<InputKey, T>) -> BTreeMap<u32, T> {
            map.iter()
                .filter(|(k, _)| &*k.device == gob_core::LOGICAL_DEVICE_GUID)
                .map(|(k, &v)| (k.id, v))
                .collect()
        }
        LogicalValues {
            axes: of(&self.axes),
            buttons: of(&self.buttons),
            hats: of(&self.hats),
        }
    }

    /// vJoy device ids the profile writes to.
    pub fn vjoy_devices(&self) -> BTreeSet<u32> {
        self.nodes
            .iter()
            .flat_map(|n| match &**n {
                Node::Vjoy { device, .. }
                | Node::ButtonAxis {
                    target: AxisOut::Vjoy(device, _),
                    ..
                } => vec![*device],
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

    /// Virtual gamepad ids the profile writes to.
    pub fn gamepads(&self) -> BTreeSet<u32> {
        self.nodes
            .iter()
            .flat_map(|n| match &**n {
                Node::Gamepad { gamepad, .. }
                | Node::ButtonAxis {
                    target: AxisOut::Pad(gamepad, _),
                    ..
                } => vec![*gamepad],
                Node::Macro { steps, .. } => steps
                    .iter()
                    .filter_map(|s| match *s {
                        MacroStep::GamepadButton { gamepad, .. }
                        | MacroStep::GamepadAxis { gamepad, .. }
                        | MacroStep::GamepadDpad { gamepad, .. } => Some(gamepad),
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
            .chain(self.button_axes.values().filter_map(|a| {
                let step = a.moving.then(|| a.last_step + AXIS_STEP);
                step.into_iter().chain(a.spring_at).min()
            }))
            .chain(
                self.scripts
                    .as_ref()
                    .filter(|_| !self.paused)
                    .and_then(ScriptHost::next_deadline),
            )
            .min()
    }

    /// Whether outputs move on their own (button-axis velocity and smooth, relative axes):
    /// the runner then wakes up on time more precisely than the system's default.
    pub fn animating(&self) -> bool {
        self.button_axes.values().any(|a| a.moving)
            || self.relative.values().any(|r| r.active)
            || self.logical_relative.values().any(|r| r.active)
    }

    /// Whether the profile has voice-input actions (a microphone may be needed).
    pub fn uses_voice(&self) -> bool {
        self.nodes.iter().any(|n| matches!(**n, Node::Voice { .. }))
    }

    /// Voice command `id` was heard: its button is pressed and released after the command's
    /// hold time ([`Engine::tick`]); heard again while down, it stays down that long from now.
    /// Unknown ids are ignored.
    pub fn voice_command(&mut self, id: u32, now: Instant, out: &mut dyn Output) {
        let Some(&hold) = self.voice_holds.get(&id) else {
            return;
        };
        match self.voice_releases.remove(&id) {
            Some(timer) => self.timers.retain(|t| t.id != timer),
            None => self.process(&voice_key(id), Value::Button(true), now, out),
        }
        let timer = self.add_timer(now + hold, TimerKind::VoiceRelease(id));
        self.voice_releases.insert(id, timer);
    }

    /// The microphones voice-input actions listen on (empty = the default one), each once.
    pub fn voice_microphones(&self) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for n in &self.nodes {
            if let Node::Voice { microphone, .. } = &**n
                && !found.contains(microphone)
            {
                found.push(microphone.clone());
            }
        }
        found
    }

    /// The phrases to listen for, each with its command's id.
    pub fn voice_phrases(&self) -> &[(u32, String)] {
        &self.voice_phrases
    }

    /// Before this engine is replaced: stops listening on every microphone, as if each
    /// voice-input action were released, and releases the voice commands still held down (their
    /// release timers go with this engine).
    pub fn stop_voice(&mut self, now: Instant, out: &mut dyn Output) {
        let nodes: Vec<NodeId> = self.listening.keys().copied().collect();
        for node in nodes {
            self.stop_listening(node, out);
        }
        let mut held: Vec<(u32, u64)> = self.voice_releases.drain().collect();
        held.sort_unstable();
        for (id, timer) in held {
            self.timers.retain(|t| t.id != timer);
            self.process(&voice_key(id), Value::Button(false), now, out);
        }
    }

    /// Whether the profile has light-led actions.
    pub fn uses_lights(&self) -> bool {
        self.nodes
            .iter()
            .any(|n| matches!(**n, Node::LightLed { .. }))
            || self.sets.iter().flatten().any(|c| !c.lights.is_empty())
    }

    /// Writes the start values of button-axis actions and sets the LEDs of the light-led
    /// actions marked "on activate" that are bound in the current mode (in profile order). Call
    /// once when the profile starts; mode changes call it themselves.
    pub fn activate(&mut self, out: &mut dyn Output) {
        if !std::mem::replace(&mut self.started, true) {
            self.write_start_values(out);
        }
        // Schedules the scripts' periodic functions.
        self.tick_scripts(Instant::now(), out);
        let mode = self.modes.current();
        let mut configs: Vec<&Config> = self
            .lookup
            .values()
            .filter_map(|m| m.get(mode))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .flat_map(|&set| &self.sets[set])
            .filter(|c| !c.lights.is_empty())
            .collect();
        configs.sort_by_key(|c| c.virtual_id);
        for light in configs.iter().flat_map(|c| &c.lights) {
            out.light_led(light);
        }
    }

    /// Loads the profile's Lua scripts stored in it (external files are read in by the app).
    fn load_scripts(&mut self, profile: &Profile) {
        let loaded: Vec<LoadedScript> = profile
            .script_instances()
            .into_iter()
            .filter(|s| s.language == gob_core::ScriptLanguage::Lua)
            .filter_map(|s| match s.source {
                gob_core::ScriptSource::Code(code) => Some(LoadedScript {
                    name: s.name,
                    code,
                    values: s.variables,
                }),
                gob_core::ScriptSource::File(path) => {
                    log::warn!("script {}: file {path} was not read in", s.name);
                    None
                }
            })
            .collect();
        if loaded.is_empty() {
            return;
        }
        let mut host = ScriptHost::load(&loaded);
        let modes: Vec<(String, Option<String>)> = profile
            .modes
            .iter()
            .map(|m| (m.name.clone(), m.parent.clone()))
            .collect();
        host.set_modes(&modes);
        self.script_modes = modes.into_iter().map(|(m, _)| m).collect();
        // A profile's stick may be another attached one: handlers listen to that one.
        host.map_devices(|d| self.device(d).to_string());
        self.script_keys = host
            .inputs()
            .map(|(index, i)| {
                let kind = match i.kind {
                    ScriptKind::Axis => InputKind::Axis,
                    ScriptKind::Button => InputKind::Button,
                    ScriptKind::Hat => InputKind::Hat,
                };
                let key = InputKey {
                    device: i.device.as_str().into(),
                    kind,
                    id: i.id,
                };
                (key, index)
            })
            .collect();
        self.action_labels = profile
            .library
            .iter()
            .filter_map(|a| {
                let label = a.label()?.trim();
                (!label.is_empty() && label != "None").then(|| (label.to_string(), a.id.clone()))
            })
            .collect();
        self.scripts = Some(host);
    }

    /// Script errors since the last call, one per script: load errors and a script's first error
    /// while it runs.
    pub fn take_script_errors(&mut self) -> Vec<ScriptError> {
        self.scripts
            .as_mut()
            .map(ScriptHost::take_errors)
            .unwrap_or_default()
    }

    /// Runs the script handlers of an input event in `mode` (the mode when the event came).
    fn run_scripts(
        &mut self,
        key: &InputKey,
        value: Value,
        mode: &str,
        now: Instant,
        out: &mut dyn Output,
    ) {
        if self.paused {
            return;
        }
        let Some(&index) = self.script_keys.get(key) else {
            return;
        };
        let Some(mut host) = self.scripts.take() else {
            return;
        };
        let event = match value {
            Value::Axis(v) => EventValue::Axis(v),
            Value::Button(b) => EventValue::Button(b),
            Value::Hat(d) => EventValue::Hat(d),
        };
        let mut api = ScriptContext {
            engine: self,
            out,
            now,
        };
        host.event_at(index, event, mode, &mut api);
        self.scripts = Some(host);
    }

    fn tick_scripts(&mut self, now: Instant, out: &mut dyn Output) {
        if self.paused {
            return;
        }
        let Some(mut host) = self.scripts.take() else {
            return;
        };
        let mut api = ScriptContext {
            engine: self,
            out,
            now,
        };
        host.tick(now, &mut api);
        self.scripts = Some(host);
    }

    /// Start values of the button-axis actions, the first one met for each axis.
    fn write_start_values(&mut self, out: &mut dyn Output) {
        let now = Instant::now();
        for node in 0..self.nodes.len() {
            let Node::ButtonAxis {
                ref axis,
                ref target,
                up: true,
            } = *self.nodes[node]
            else {
                continue;
            };
            let Some(start) = axis.start else { continue };
            if self.button_axes.contains_key(target) {
                continue;
            }
            let target = target.clone();
            self.axis_state(&target, node, now).value = start;
            self.write_axis(&target, start, now, out);
        }
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
        let layer = self.held_layer();
        self.seed(key, value);
        let before = self.modes.shared();
        // Script handlers first, like Gremlin's user scripts; the event's actions still run in
        // the mode it came in, even when a handler switched modes.
        self.run_scripts(key, value, &before, now, out);
        if self.is_modifier(key) {
            self.after_layer_change(&layer, now, out);
            self.after_mode_change(&before, now, out);
            return;
        }
        self.dispatch(key, value, before.clone(), now, out);
        self.after_mode_change(&before, now, out);
    }

    fn is_modifier(&self, key: &InputKey) -> bool {
        self.modifiers.iter().any(|(k, _)| k == key)
    }

    /// The modifiers held right now.
    fn held_layer(&self) -> Layer {
        Layer::new(
            self.modifiers
                .iter()
                .filter(|(k, _)| self.buttons.get(k).copied().unwrap_or(false))
                .map(|(_, name)| name.as_str()),
        )
    }

    /// Layer an event runs in: buttons and hats keep the one they were pressed in until
    /// released, axes follow the held modifiers.
    fn event_layer(&mut self, key: &InputKey, value: Value) -> Layer {
        if self.modifiers.is_empty() {
            return Layer::base();
        }
        match value {
            Value::Axis(_) => self.held_layer(),
            Value::Button(true) | Value::Hat(Some(_)) => {
                if let Some(layer) = self.pressed_layers.get(key) {
                    return layer.clone();
                }
                let layer = self.held_layer();
                self.pressed_layers.insert(key.clone(), layer.clone());
                layer
            }
            Value::Button(false) | Value::Hat(None) => self
                .pressed_layers
                .remove(key)
                .unwrap_or_else(|| self.held_layer()),
        }
    }

    /// The held modifiers changed: axis configurations that stop running get the input at rest
    /// (their outputs go back to neutral, unless they keep their value) and the ones that start
    /// get the current position.
    /// Neutral values that a starting configuration writes anyway are not sent, so an output both
    /// layers drive does not jump.
    fn after_layer_change(&mut self, before: &Layer, now: Instant, out: &mut dyn Output) {
        let held = self.held_layer();
        if held == *before {
            return;
        }
        let mode = self.modes.current().to_string();
        let mut axes: Vec<(InputKey, f32, usize)> = self
            .axes
            .iter()
            .filter_map(|(k, &v)| {
                let set = *self.lookup.get(k)?.get(&mode)?;
                Some((k.clone(), v, set))
            })
            .collect();
        axes.sort_by(|a, b| (&a.0.device, a.0.id).cmp(&(&b.0.device, b.0.id)));
        let changed = |engine: &Self, set: usize, starting: bool| -> Vec<usize> {
            let (was, is) = (engine.selection(set, before), engine.selection(set, &held));
            (0..engine.sets[set].len())
                .filter(|&ci| {
                    let layer = &engine.sets[set][ci].layer;
                    let (was, runs) = (was.runs(layer), is.runs(layer));
                    if starting { !was && runs } else { was && !runs }
                })
                .collect()
        };
        let mut neutral = Deferred::new(out);
        for (key, _, set) in &axes {
            let src = self.source(key);
            for ci in changed(self, *set, false) {
                if self.sets[*set][ci].keep_value {
                    continue;
                }
                self.run_config((*set, ci), &src, Value::Axis(0.0), now, &mut neutral);
            }
        }
        let (pending, out) = neutral.into_parts();
        let mut written = Recording::new(out);
        for (key, v, set) in &axes {
            let src = self.source(key);
            for ci in changed(self, *set, true) {
                self.run_config((*set, ci), &src, Value::Axis(*v), now, &mut written);
            }
        }
        let (targets, out) = written.into_parts();
        for w in pending
            .into_iter()
            .filter(|w| !targets.contains(&w.target()))
        {
            w.send(out);
        }
    }

    /// Which configurations of a binding run while `held` is held.
    fn selection(&self, set: usize, held: &Layer) -> Selection {
        Selection::new(self.sets[set].iter().map(|c| (&c.layer, c.additive)), held)
    }

    fn source(&self, key: &InputKey) -> Source {
        Source {
            key: key.clone(),
            mode: self.modes.shared(),
            no_auto_release: false,
        }
    }

    /// Fires due timers (tempo thresholds, pulse releases).
    pub fn tick(&mut self, now: Instant, out: &mut dyn Output) {
        let before = self.modes.shared();
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
                TimerKind::SendUp(node) => self.send_up(node, out),
                TimerKind::SendNext(node) => self.send_next(node, timer.at, out),
                TimerKind::VoiceRelease(id) => {
                    self.voice_releases.remove(&id);
                    self.process(&voice_key(id), Value::Button(false), now, out);
                }
                TimerKind::EventRelease(id) => {
                    if let (Some((_, mode)), Some(event)) =
                        (self.event_releases.remove(&id), ProfileEvent::from_id(id))
                    {
                        self.event_up(event, mode, now, out);
                    }
                }
                TimerKind::RunRelease { node, src } => {
                    self.run_releases.remove(&node);
                    self.run_release(node, &src, now, out);
                }
            }
        }
        self.step_relative(now, out);
        self.step_logical(now, out);
        self.step_button_axes(now, out);
        self.tick_scripts(now, out);
        if let Some((dx, dy)) = self.motion.step(now)
            && (dx != 0 || dy != 0)
        {
            out.mouse_move(dx, dy);
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
        self.refresh_mode(now, out);
        // The mode left, then the one entered; their actions may switch modes again.
        if self.mode_event_depth < MAX_MODE_EVENTS {
            self.mode_event_depth += 1;
            let entered = self.modes.shared();
            self.fire_event(ProfileEvent::Leave, Arc::from(before), now, out);
            self.fire_event(ProfileEvent::Enter, entered.clone(), now, out);
            self.after_mode_change(&entered, now, out);
            self.mode_event_depth -= 1;
        } else {
            log::warn!("mode enter and leave actions keep switching modes: their events stop");
        }
    }

    /// The profile has just started (not an edit applied to it): the start and mode enter
    /// events run in the starting mode, and held buttons that send their state at start run
    /// their press.
    pub fn profile_started(&mut self, now: Instant, out: &mut dyn Output) {
        let mode = self.modes.shared();
        self.fire_event(ProfileEvent::Start, mode.clone(), now, out);
        self.fire_event(ProfileEvent::Enter, mode.clone(), now, out);
        self.send_start_states(&mode, now, out);
        self.after_mode_change(&mode, now, out);
    }

    /// The profile is about to stop: the mode leave and stop events run in the current mode.
    /// True when either has actions, which then need a moment ([`Engine::tick`]) to finish
    /// before [`Engine::release_all`].
    pub fn profile_stopping(&mut self, now: Instant, out: &mut dyn Output) -> bool {
        let mode = self.modes.shared();
        let leave = self.fire_event(ProfileEvent::Leave, mode.clone(), now, out);
        let stop = self.fire_event(ProfileEvent::Stop, mode, now, out);
        leave || stop
    }

    /// Something still runs on a timer (event releases, macros, pulses).
    pub fn has_timers(&self) -> bool {
        !self.timers.is_empty()
    }

    /// Presses a profile event's button in `mode` (it goes up after [`EVENT_HOLD`]); false
    /// when the event has no actions there. A press still down goes up first.
    fn fire_event(
        &mut self,
        event: ProfileEvent,
        mode: Arc<str>,
        now: Instant,
        out: &mut dyn Output,
    ) -> bool {
        let key = event_key(event);
        if !self
            .lookup
            .get(&key)
            .is_some_and(|m| m.contains_key(&*mode))
        {
            return false;
        }
        if let Some((timer, was)) = self.event_releases.remove(&event.id()) {
            self.timers.retain(|t| t.id != timer);
            self.event_up(event, was, now, out);
        }
        log::debug!("profile event {event:?} in mode {mode}");
        self.seed(&key, Value::Button(true));
        self.dispatch(&key, Value::Button(true), mode.clone(), now, out);
        let timer = self.add_timer(now + EVENT_HOLD, TimerKind::EventRelease(event.id()));
        self.event_releases.insert(event.id(), (timer, mode));
        true
    }

    fn event_up(
        &mut self,
        event: ProfileEvent,
        mode: Arc<str>,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let key = event_key(event);
        self.seed(&key, Value::Button(false));
        self.dispatch(&key, Value::Button(false), mode, now, out);
    }

    /// Runs the press of the buttons held now whose binding in `mode` sends its state at start
    /// (only those action sequences, in the held layer).
    fn send_start_states(&mut self, mode: &Arc<str>, now: Instant, out: &mut dyn Output) {
        let mut held: Vec<(InputKey, usize)> = self
            .buttons
            .iter()
            .filter(|&(k, &down)| down && !self.is_modifier(k))
            .filter_map(|(k, _)| {
                let set = *self.lookup.get(k)?.get(&**mode)?;
                self.sets[set]
                    .iter()
                    .any(|c| c.send_state)
                    .then(|| (k.clone(), set))
            })
            .collect();
        held.sort_by(|a, b| (&a.0.device, a.0.id).cmp(&(&b.0.device, b.0.id)));
        for (key, set) in held {
            let src = Source {
                key: key.clone(),
                mode: mode.clone(),
                no_auto_release: false,
            };
            let layer = self.event_layer(&key, Value::Button(true));
            let selection = self.selection(set, &layer);
            for ci in 0..self.sets[set].len() {
                let c = &self.sets[set][ci];
                if c.send_state && selection.runs(&c.layer) {
                    self.run_config((set, ci), &src, Value::Button(true), now, out);
                }
            }
        }
    }

    /// Lights, cursor motion and axis positions follow a mode change.
    fn refresh_mode(&mut self, now: Instant, out: &mut dyn Output) {
        self.activate(out);
        // Stop cursor motion whose input is bound differently in the new mode.
        let (lookup, current) = (&self.lookup, self.modes.current().to_string());
        self.motion.retain(|(_, key), mode| {
            lookup
                .get(key)
                .is_some_and(|m| m.get(&**mode).is_some() && m.get(&**mode) == m.get(&current))
        });
        let axes: Vec<(InputKey, f32)> = self.axes.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for (key, v) in axes {
            let mode = self.modes.shared();
            self.dispatch(&key, Value::Axis(v), mode, now, out);
        }
    }

    fn dispatch(
        &mut self,
        key: &InputKey,
        value: Value,
        mode: Arc<str>,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let src = Source {
            key: key.clone(),
            mode,
            no_auto_release: false,
        };
        let layer = self.event_layer(key, value);
        if let Some(&set) = self.lookup.get(key).and_then(|m| m.get(&*src.mode)) {
            let selection = self.selection(set, &layer);
            for ci in 0..self.sets[set].len() {
                if selection.runs(&self.sets[set][ci].layer) {
                    self.run_config((set, ci), &src, value, now, out);
                }
            }
        }
        match value {
            Value::Button(pressed) => self.process_release(key, pressed, now, out),
            Value::Hat(None) => self.process_release(key, false, now, out),
            _ => {}
        }
    }

    /// Runs one configuration of a binding for an input event.
    fn run_config(
        &mut self,
        (set, ci): (usize, usize),
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        if self.paused && !self.sets[set][ci].always {
            return;
        }
        let (root, virtual_id) = (self.sets[set][ci].root, self.sets[set][ci].virtual_id);
        let states = match self.sets[set][ci].vbutton.as_mut() {
            None => {
                self.run(root, src, &mut value.clone(), now, out, 0);
                return;
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
        let shared = Arc::clone(&self.nodes[node]);
        match *shared {
            Node::Seq(ref children) => {
                for &c in children {
                    self.run(c, src, value, now, out, depth + 1);
                }
            }
            Node::Vjoy {
                device,
                target,
                inverted,
                act,
                relative,
                send,
            } => {
                // Send modes see both edges: a repeat stops on the one that does not trigger.
                if let (VjoyTarget::Button(id), false) = (target, send.mode == SendMode::Hold) {
                    let press = Press::Vjoy(device, id);
                    self.send_event(node, press, send, act, src, *value, now, out);
                    return;
                }
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
                        self.vjoy_button(device, id, pressed, out);
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
            Node::Gamepad {
                gamepad,
                control,
                inverted,
                act,
                send,
            } => {
                if control.kind() == ControlKind::Button && send.mode != SendMode::Hold {
                    let press = Press::Pad(gamepad, control);
                    self.send_event(node, press, send, act, src, *value, now, out);
                    return;
                }
                if !should_execute(act, *value) {
                    return;
                }
                self.gamepad(gamepad, control, inverted, *value, src, out);
            }
            Node::ChangeMode {
                kind,
                ref targets,
                act,
            } => {
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
            Node::Curve {
                deadzone,
                ref curve,
            } => {
                if let Value::Axis(v) = value {
                    *v = curve.eval(f64::from(apply_deadzone(*v, deadzone))) as f32;
                }
            }
            Node::Tempo { .. } => self.tempo_event(node, src, *value, now, out),
            Node::DoubleTap { .. } => self.tap_event(node, src, *value, now, out),
            Node::Chain {
                ref chains,
                timeout,
            } => {
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
            Node::SmartToggle {
                ref children,
                delay,
            } => {
                if let Value::Button(pressed) = *value {
                    self.smart_toggle(node, children, delay, src, pressed, now, out);
                }
            }
            Node::Split {
                split,
                ref lower,
                ref upper,
            } => {
                if let Value::Axis(v) = *value {
                    let (scaled, nodes) = if v < split {
                        (-axis_interpolation(v, -1.0, split), lower)
                    } else {
                        (axis_interpolation(v, split, 1.0), upper)
                    };
                    *value = Value::Axis(scaled);
                    for &n in nodes {
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
                ref executable,
                ref arguments,
                act,
            } => {
                if should_execute(act, *value) && !executable.trim().is_empty() {
                    out.run_command(executable, arguments);
                }
            }
            Node::Mouse { action, act, send } => {
                if let (MouseAction::Button(button), false) = (action, send.mode == SendMode::Hold)
                {
                    let press = Press::Mouse(button);
                    self.send_event(node, press, send, act, src, *value, now, out);
                    return;
                }
                if !should_execute(act, *value) {
                    return;
                }
                self.mouse(node, action, src, *value, now, out);
            }
            Node::Keys { act, send, .. } => {
                if send.mode != SendMode::Hold {
                    let press = Press::Keys(node);
                    self.send_event(node, press, send, act, src, *value, now, out);
                    return;
                }
                if !should_execute(act, *value) {
                    return;
                }
                if value.truthy() {
                    self.press_keys(node, out);
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
            Node::Logical {
                ref key,
                inverted,
                relative,
                act,
            } => {
                if !should_execute(act, *value) {
                    return;
                }
                match (key.kind, *value) {
                    (InputKind::Axis, Value::Axis(v)) if relative.is_some() => {
                        let speed = relative.unwrap_or(1.0);
                        let r = self
                            .logical_relative
                            .entry(key.clone())
                            .or_insert(RelativeAxis {
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
                    }
                    (InputKind::Axis, v) => {
                        let v = match v {
                            Value::Axis(v) => v,
                            Value::Button(b) => f32::from(u8::from(b)),
                            Value::Hat(_) => return,
                        };
                        self.feed_logical(key, Value::Axis(v), now, out);
                    }
                    (InputKind::Button, v) => {
                        let pressed = v.truthy() != inverted;
                        self.feed_logical(key, Value::Button(pressed), now, out);
                        if pressed && !src.no_auto_release {
                            self.register(
                                &src.key,
                                ReleaseEntry {
                                    mode: src.mode.clone(),
                                    any_mode: false,
                                    on_press: inverted,
                                    action: ReleaseAction::Logical(key.clone()),
                                },
                            );
                        }
                    }
                    (InputKind::Hat, Value::Hat(d)) => {
                        self.feed_logical(key, Value::Hat(d), now, out)
                    }
                    _ => {}
                }
            }
            Node::ButtonAxis { ref target, .. } => {
                let pressed = match *value {
                    Value::Button(b) => b,
                    Value::Hat(d) => d.is_some(),
                    Value::Axis(_) => return,
                };
                let target = target.clone();
                self.button_axis_event(node, &target, pressed, now, out);
                if pressed {
                    self.register(
                        &src.key,
                        ReleaseEntry {
                            mode: src.mode.clone(),
                            any_mode: false,
                            on_press: false,
                            action: ReleaseAction::AxisRelease(node),
                        },
                    );
                }
            }
            Node::LoadProfile { ref file, act } => {
                if should_execute(act, *value) && !file.is_empty() {
                    log::info!("load-profile action: {file}");
                    out.load_profile(file);
                }
            }
            Node::PlaySound {
                ref file,
                volume,
                act,
            } => {
                if should_execute(act, *value) && !file.is_empty() {
                    out.play_sound(file, volume);
                }
            }
            Node::LightLed { ref light, act } => {
                if should_execute(act, *value) {
                    out.light_led(light);
                }
            }
            Node::Speak { ref request, act } => {
                if should_execute(act, *value) && !request.text.trim().is_empty() {
                    let mode = self.modes.current();
                    let mut request = request.clone();
                    request.text = request
                        .text
                        .replace("${current_mode}", mode)
                        .replace("$current_mode", mode);
                    out.speak(request);
                }
            }
            Node::Condition {
                all,
                ref conditions,
                ref branches,
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
                        if let Some(prev) = self.cond_cache.insert(node, b)
                            && prev != b
                        {
                            for &c in &branches[prev] {
                                self.run(c, src, &mut Value::Hat(None), now, out, depth + 1);
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
                ref axes,
                inner,
                outer,
                ref outputs,
            } => {
                let [x, y] = axes
                    .each_ref()
                    .map(|k| self.axes.get(k).copied().unwrap_or(0.0));
                if let Some(results) = dual_deadzone(x, y, inner, outer) {
                    for (v, nodes) in results.into_iter().zip(outputs) {
                        let mut v = Value::Axis(v);
                        for &n in nodes {
                            self.run(n, src, &mut v, now, out, depth + 1);
                        }
                    }
                }
            }
            Node::AxisDelta {
                threshold,
                ref positive,
                ref negative,
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
                self.pulse(nodes, Value::Button(true), psrc, now, out);
            }
            Node::Merge {
                ref axes,
                op,
                ref children,
            } => {
                let [a, b] = axes
                    .each_ref()
                    .map(|k| self.axes.get(k).copied().unwrap_or(0.0));
                *value = Value::Axis(op.apply(a, b));
                for &c in children {
                    self.run(c, src, value, now, out, depth + 1);
                }
            }
            Node::HatButtons(ref buttons) => {
                if let Value::Hat(d) = *value {
                    self.hat_buttons_event(node, buttons, src, d, now, out, depth);
                }
            }
            Node::Macro { repeat, act, .. } => {
                if should_execute(act, *value) {
                    self.macro_trigger(node, repeat, src, now, out);
                }
            }
            Node::Voice {
                ref microphone,
                act,
            } => {
                if !should_execute(act, *value) {
                    return;
                }
                if value.truthy() {
                    self.start_listening(node, microphone.clone(), out);
                    // Also when let go in another mode or while paused: the microphone must
                    // never stay open.
                    self.register(
                        &src.key,
                        ReleaseEntry {
                            mode: src.mode.clone(),
                            any_mode: true,
                            on_press: false,
                            action: ReleaseAction::StopListening(node),
                        },
                    );
                } else {
                    self.stop_listening(node, out);
                }
            }
            Node::Noop => {}
        }
    }

    /// A voice-input action is pressed; the microphone opens unless another holds it open.
    fn start_listening(&mut self, node: NodeId, microphone: String, out: &mut dyn Output) {
        if self.listening.contains_key(&node) {
            return;
        }
        if !self.listening.values().any(|m| *m == microphone) {
            out.voice_listen(&microphone, true);
        }
        self.listening.insert(node, microphone);
    }

    /// A voice-input action is released; the microphone closes when nothing else holds it.
    fn stop_listening(&mut self, node: NodeId, out: &mut dyn Output) {
        let Some(microphone) = self.listening.remove(&node) else {
            return;
        };
        if !self.listening.values().any(|m| *m == microphone) {
            out.voice_listen(&microphone, false);
        }
    }

    /// Presses a map-to-keyboard action's keys in order, unless they are down already.
    fn press_keys(&mut self, node: NodeId, out: &mut dyn Output) {
        if !self.keys_down.insert(node) {
            return;
        }
        if let Node::Keys { keys, .. } = &*self.nodes[node] {
            for k in keys {
                out.key(k.scan_code, k.extended, true);
            }
        }
    }

    /// An input event for a button output with a [`SendMode`] other than hold. The trigger is
    /// the press, or the release when the action runs on release.
    #[allow(clippy::too_many_arguments)]
    fn send_event(
        &mut self,
        node: NodeId,
        press: Press,
        send: ButtonSend,
        act: ActivationMode,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        // A hat is on while it is pushed in any direction.
        let on = match value {
            Value::Hat(direction) => direction.is_some(),
            v => v.truthy(),
        };
        let state = self.sends.entry(node).or_default();
        if state.input == on {
            return;
        }
        state.input = on;
        let triggered = on != (act == ActivationMode::Release);
        // A wheel has nothing to hold: press, release and toggle turn it one notch each time.
        if let Press::Mouse(wheel @ (MouseButton::WheelUp | MouseButton::WheelDown)) = press
            && matches!(
                send.mode,
                SendMode::Press | SendMode::Release | SendMode::Toggle
            )
        {
            if triggered {
                out.mouse_wheel(if wheel == MouseButton::WheelUp { 1 } else { -1 });
            }
            return;
        }
        let state = self.sends.entry(node).or_default();
        match send.mode {
            SendMode::Hold => {}
            SendMode::Press | SendMode::Release if triggered => {
                self.press(node, press, send.mode == SendMode::Press, out)
            }
            SendMode::Toggle if triggered => {
                let down = !state.down;
                self.press(node, press, down, out);
            }
            SendMode::Pulse if triggered => {
                self.press(node, press, true, out);
                let at = now + ms(send.pulse_ms);
                self.restart_send_timer(node, at, TimerKind::SendUp(node), false);
            }
            SendMode::Repeat if triggered => {
                if state.repeating {
                    return;
                }
                state.repeating = true;
                state.send = send;
                self.press(node, press, true, out);
                let up = now + ms(send.pulse_ms.min(send.delay_ms / 2).max(1));
                self.restart_send_timer(node, up, TimerKind::SendUp(node), false);
                let next = now + ms(send.delay_ms);
                self.restart_send_timer(node, next, TimerKind::SendNext(node), true);
                // Let go of the input in another mode: the repeat stops too. Inside smart-toggle
                // the container sends the release itself.
                if src.no_auto_release {
                    return;
                }
                self.register(
                    &src.key,
                    ReleaseEntry {
                        mode: src.mode.clone(),
                        any_mode: false,
                        on_press: act == ActivationMode::Release,
                        action: ReleaseAction::StopSend(node),
                    },
                );
            }
            SendMode::Repeat => self.stop_send(node, out),
            _ => {}
        }
    }

    /// A [`Press`] output goes down or up; the action's state remembers it.
    fn press(&mut self, node: NodeId, press: Press, down: bool, out: &mut dyn Output) {
        match press {
            Press::Vjoy(device, button) => self.vjoy_button(device, button, down, out),
            Press::Pad(gamepad, control) => self.pad_button(gamepad, control, down, out),
            Press::Keys(node) if down => self.press_keys(node, out),
            Press::Keys(node) if self.keys_down.contains(&node) => self.release_keys(node, out),
            // Release only: the keys were pressed by another action (press only).
            Press::Keys(node) => self.let_go_of_keys(node, out),
            // A wheel turns one notch each time it goes down.
            Press::Mouse(MouseButton::WheelUp) if down => out.mouse_wheel(1),
            Press::Mouse(MouseButton::WheelDown) if down => out.mouse_wheel(-1),
            Press::Mouse(MouseButton::WheelUp | MouseButton::WheelDown) => {}
            Press::Mouse(button) => self.mouse_button(button, down, out),
        }
        let state = self.sends.entry(node).or_default();
        state.down = down;
        state.press = Some(press);
    }

    /// Replaces the action's pending release (`next` false) or next pulse (`next` true).
    fn restart_send_timer(&mut self, node: NodeId, at: Instant, kind: TimerKind, next: bool) {
        let old = {
            let state = self.sends.entry(node).or_default();
            if next {
                state.next_timer.take()
            } else {
                state.up_timer.take()
            }
        };
        if let Some(id) = old {
            self.timers.retain(|t| t.id != id);
        }
        let id = self.add_timer(at, kind);
        let state = self.sends.entry(node).or_default();
        if next {
            state.next_timer = Some(id);
        } else {
            state.up_timer = Some(id);
        }
    }

    /// A pulse's time is up.
    fn send_up(&mut self, node: NodeId, out: &mut dyn Output) {
        let Some(state) = self.sends.get_mut(&node) else {
            return;
        };
        state.up_timer = None;
        if let (true, Some(press)) = (state.down, state.press) {
            self.press(node, press, false, out);
        }
    }

    /// Repeat: the next pulse, `at` the time it was due (the period does not drift).
    fn send_next(&mut self, node: NodeId, at: Instant, out: &mut dyn Output) {
        let Some(state) = self.sends.get_mut(&node) else {
            return;
        };
        let send = state.send;
        state.next_timer = None;
        let (true, Some(press)) = (state.repeating, state.press) else {
            return;
        };
        if state.down {
            // A pulse longer than the period: the output goes up for one pass first.
            self.press(node, press, false, out);
        }
        self.press(node, press, true, out);
        let up = at + ms(send.pulse_ms.min(send.period_ms / 2).max(1));
        self.restart_send_timer(node, up, TimerKind::SendUp(node), false);
        let next = at + ms(send.period_ms);
        self.restart_send_timer(node, next, TimerKind::SendNext(node), true);
    }

    /// Repeat ends: no more pulses, the output goes up.
    fn stop_send(&mut self, node: NodeId, out: &mut dyn Output) {
        let Some(state) = self.sends.get_mut(&node) else {
            return;
        };
        state.repeating = false;
        let timers = [state.up_timer.take(), state.next_timer.take()];
        let (down, press) = (state.down, state.press);
        self.timers.retain(|t| !timers.contains(&Some(t.id)));
        if let (true, Some(press)) = (down, press) {
            self.press(node, press, false, out);
        }
    }

    /// A mouse button held by the profile (released by [`Engine::release_all`]).
    fn mouse_button(&mut self, button: MouseButton, down: bool, out: &mut dyn Output) {
        out.mouse_button(button, down);
        let held = self.mouse_down.contains(&button);
        if down && !held {
            self.mouse_down.push(button);
        } else if !down && held {
            self.mouse_down.retain(|&b| b != button);
        }
    }

    /// A virtual stick button the profile presses or lets go.
    fn vjoy_button(&mut self, device: u32, button: u8, down: bool, out: &mut dyn Output) {
        out.button(device, button, down);
        self.vjoy_down.retain(|&b| b != (device, button));
        if down {
            self.vjoy_down.push((device, button));
        }
    }

    /// A virtual gamepad stick or trigger value (trigger 0..1), remembered so an axis from
    /// buttons on it starts from where other actions left it.
    fn pad_axis(&mut self, gamepad: u32, control: GamepadControl, v: f32, out: &mut dyn Output) {
        self.pad_axes.insert((gamepad, control), v);
        out.gamepad_axis(gamepad, control, v);
    }

    /// A virtual gamepad button the profile presses or lets go.
    fn pad_button(
        &mut self,
        gamepad: u32,
        control: GamepadControl,
        down: bool,
        out: &mut dyn Output,
    ) {
        out.gamepad_button(gamepad, control, down);
        self.pad_down.retain(|&b| b != (gamepad, control));
        if down {
            self.pad_down.push((gamepad, control));
        }
    }

    /// Lets go of everything the profile holds down: keys, mouse buttons, virtual stick and
    /// gamepad buttons, latched and repeating outputs. Called when the profile stops or
    /// another one takes over (the virtual devices stay, so nothing else would release them).
    pub fn release_all(&mut self, out: &mut dyn Output) {
        for key in std::mem::take(&mut self.script_keys_down) {
            out.key(key.scan_code, key.extended, false);
        }
        for (device, hat) in std::mem::take(&mut self.script_hats) {
            out.hat(device, hat, None);
        }
        // Actions run-action steps hold pressed go up first, at their release time, so their
        // own state (taps, tempo, toggles) is not left down; what they hold is released below.
        self.run_releases.clear();
        let pending: Vec<Timer> = self
            .timers
            .extract_if(.., |t| matches!(t.kind, TimerKind::RunRelease { .. }))
            .collect();
        for t in pending {
            if let TimerKind::RunRelease { node, src } = t.kind {
                self.run_release(node, &src, t.at, out);
            }
        }
        let held: Vec<(NodeId, Press)> = self
            .sends
            .iter()
            .filter_map(|(&n, s)| s.press.filter(|_| s.down).map(|p| (n, p)))
            .collect();
        for (node, press) in held {
            self.press(node, press, false, out);
        }
        self.sends.clear();
        for entry in std::mem::take(&mut self.releases).into_values().flatten() {
            match entry.action {
                ReleaseAction::VjoyButton(device, button) => {
                    self.vjoy_button(device, button, false, out)
                }
                ReleaseAction::GamepadButton(gamepad, control) => {
                    self.pad_button(gamepad, control, false, out)
                }
                _ => {}
            }
        }
        for node in self.keys_down.clone() {
            self.release_keys(node, out);
        }
        for button in std::mem::take(&mut self.mouse_down) {
            out.mouse_button(button, false);
        }
        // Pressed without a release entry (pulses, macro steps) or still waiting for a timer.
        for (device, button) in std::mem::take(&mut self.vjoy_down) {
            out.button(device, button, false);
        }
        for (gamepad, control) in std::mem::take(&mut self.pad_down) {
            out.gamepad_button(gamepad, control, false);
        }
        self.timers.clear();
        // Axes stay where they are, but stop moving.
        for a in self.button_axes.values_mut() {
            (a.held, a.moving, a.spring_at, a.before) = ([false; 2], false, None, None);
        }
    }

    /// The state of a button-axis axis, made at its current value (a virtual stick axis may
    /// have been written by other actions).
    fn axis_state(&mut self, target: &AxisOut, node: NodeId, now: Instant) -> &mut AxisState {
        let known = match target {
            AxisOut::Vjoy(device, axis) => self.vjoy_axes.get(&(*device, *axis)).copied(),
            AxisOut::Logical(key) => self.axes.get(key).copied(),
            // As the device takes it: a trigger 0..1, here -1..1 like the other axes.
            AxisOut::Pad(gamepad, control) => {
                self.pad_axes
                    .get(&(*gamepad, *control))
                    .map(|&v| match control.kind() {
                        ControlKind::Trigger => v * 2.0 - 1.0,
                        _ => v,
                    })
            }
        };
        let state = self
            .button_axes
            .entry(target.clone())
            .or_insert_with(|| AxisState {
                value: known.unwrap_or_else(|| target.rest()),
                node,
                held: [false; 2],
                speed: 0.0,
                target: 0.0,
                before: None,
                last_press: None,
                spring_at: None,
                moving: false,
                last_step: now,
            });
        // Another action wrote it while it stood still.
        if !state.moving
            && let Some(v) = known
        {
            state.value = v;
        }
        state
    }

    /// A button-axis input (`node`'s) went down or up.
    fn button_axis_event(
        &mut self,
        node: NodeId,
        target: &AxisOut,
        pressed: bool,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let shared = Arc::clone(&self.nodes[node]);
        let Node::ButtonAxis { ref axis, up, .. } = *shared else {
            return;
        };
        let dir = if up { 1.0 } else { -1.0 };
        let state = self.axis_state(target, node, now);
        let write = match axis.motion {
            AxisMotion::Set => {
                state.held[usize::from(!up)] = pressed;
                if pressed {
                    state.before.get_or_insert(state.value);
                    Some(dir * axis.value)
                } else if state.held[usize::from(up)] {
                    // The other button is still held: its value again.
                    axis.return_on_release.then_some(-dir * axis.value)
                } else {
                    state.before.take().filter(|_| axis.return_on_release)
                }
            }
            AxisMotion::Velocity => {
                let was = held_direction(state.held);
                state.held[usize::from(!up)] = pressed;
                state.node = node;
                let dir = held_direction(state.held);
                // A new press, or one of two held buttons let go: it travels (again) from
                // the start speed.
                if dir != 0.0 && (pressed || dir != was || !state.moving) {
                    state.speed = axis.speed;
                    if !state.moving {
                        state.moving = true;
                        state.last_step = now;
                    }
                }
                None
            }
            AxisMotion::Stepped if pressed => {
                let v = state.value;
                let eps = ButtonAxis::MIN_STEP;
                if up {
                    axis.positions.iter().copied().find(|&p| p > v + eps)
                } else {
                    axis.positions.iter().copied().rev().find(|&p| p < v - eps)
                }
            }
            AxisMotion::Encoder if pressed => {
                // Presses per second, from the time since the last one.
                let rate = state.last_press.map_or(0.0, |t| {
                    1.0 / now.duration_since(t).as_secs_f32().max(0.001)
                });
                let fast = (rate / axis.fast_rate).min(1.0);
                let step = axis.step + (axis.max_step - axis.step) * fast;
                state.spring_at = (axis.spring_ms > 0).then(|| now + ms(axis.spring_ms));
                state.node = node;
                Some((state.value + dir * step * 2.0).clamp(-1.0, 1.0))
            }
            AxisMotion::Smooth if pressed => {
                let from = if state.moving {
                    state.target
                } else {
                    state.value
                };
                state.target = (from + dir * axis.step * 2.0).clamp(-1.0, 1.0);
                state.node = node;
                if !state.moving {
                    state.moving = true;
                    state.last_step = now;
                }
                None
            }
            _ => None,
        };
        if pressed {
            state.last_press = Some(now);
        }
        if let Some(v) = write {
            state.value = v;
            self.write_axis(target, v, now, out);
        }
    }

    /// Moves the moving button-axis axes to `now`: velocity and smooth over the real time since
    /// their last step, encoder springs when due.
    fn step_button_axes(&mut self, now: Instant, out: &mut dyn Output) {
        let mut writes = std::mem::take(&mut self.axis_writes);
        for (target, a) in &mut self.button_axes {
            if a.spring_at.is_some_and(|t| t <= now) {
                a.spring_at = None;
                let rest = match &*self.nodes[a.node] {
                    Node::ButtonAxis { axis, .. } => axis.start.unwrap_or_else(|| target.rest()),
                    _ => target.rest(),
                };
                a.value = rest;
                writes.push((target.clone(), rest));
            }
            if !a.moving || now < a.last_step + AXIS_STEP {
                continue;
            }
            let dt = now.duration_since(a.last_step).as_secs_f32();
            a.last_step = now;
            let Node::ButtonAxis { axis, .. } = &*self.nodes[a.node] else {
                a.moving = false;
                continue;
            };
            match axis.motion {
                AxisMotion::Velocity => {
                    let dir = held_direction(a.held);
                    let top = axis.max_speed.max(axis.speed);
                    a.speed = (a.speed + axis.acceleration * dt).min(top);
                    a.value = (a.value + dir * a.speed * 2.0 * dt).clamp(-1.0, 1.0);
                    // Let go, or at the end it travels to.
                    if dir == 0.0 || a.value.abs() >= 1.0 && a.value.signum() == dir {
                        a.moving = false;
                    }
                }
                AxisMotion::Smooth => {
                    // 99 % of the way in the response time.
                    let tau = axis.response_ms as f32 / 1000.0 / 4.6;
                    a.value = a.target + (a.value - a.target) * (-dt / tau).exp();
                    if (a.value - a.target).abs() < ButtonAxis::MIN_STEP {
                        a.value = a.target;
                        a.moving = false;
                    }
                }
                _ => a.moving = false,
            }
            writes.push((target.clone(), a.value));
        }
        for (target, v) in writes.drain(..) {
            self.write_axis(&target, v, now, out);
        }
        self.axis_writes = writes;
    }

    fn write_axis(&mut self, target: &AxisOut, v: f32, now: Instant, out: &mut dyn Output) {
        match target {
            AxisOut::Vjoy(device, axis) => self.set_axis(*device, *axis, v, out),
            AxisOut::Pad(gamepad, control) => {
                let v = match control.kind() {
                    ControlKind::Trigger => (v + 1.0) / 2.0,
                    _ => v,
                };
                self.pad_axis(*gamepad, *control, v, out);
            }
            AxisOut::Logical(key) => self.feed_logical(key, Value::Axis(v), now, out),
        }
    }

    /// Lets go of a map-to-keyboard action's keys that another action pressed: key-up for
    /// each of them, and actions whose keys are all up now no longer count as down.
    fn let_go_of_keys(&mut self, node: NodeId, out: &mut dyn Output) {
        let Node::Keys { keys, .. } = &*self.nodes[node] else {
            return;
        };
        let keys = keys.clone();
        for k in keys.iter().rev() {
            out.key(k.scan_code, k.extended, false);
        }
        let nodes = &self.nodes;
        let mut up = Vec::new();
        self.keys_down.retain(|&other| match &*nodes[other] {
            Node::Keys { keys: held, .. } if held.iter().all(|k| keys.contains(k)) => {
                up.push(other);
                false
            }
            _ => true,
        });
        // Their press-only or toggle state is up as well.
        for other in up {
            if let Some(state) = self.sends.get_mut(&other) {
                state.down = false;
            }
        }
    }

    /// Releases a map-to-keyboard action's keys in reverse order, once per press.
    fn release_keys(&mut self, node: NodeId, out: &mut dyn Output) {
        if !self.keys_down.remove(&node) {
            return;
        }
        if let Node::Keys { keys, .. } = &*self.nodes[node] {
            for k in keys.iter().rev() {
                out.key(k.scan_code, k.extended, false);
            }
        }
    }

    /// map-to-gamepad: buttons like vJoy buttons (released with their input, also in another
    /// mode), sticks take the axis as it is, triggers its -1..1 as 0..1, the d-pad a hat. A
    /// button input pushes a stick or trigger fully, released back to rest.
    fn gamepad(
        &mut self,
        gamepad: u32,
        control: GamepadControl,
        inverted: bool,
        value: Value,
        src: &Source,
        out: &mut dyn Output,
    ) {
        match control.kind() {
            ControlKind::Button => {
                let pressed = match value {
                    Value::Button(b) => b,
                    Value::Axis(v) => v != 0.0,
                    Value::Hat(d) => d.is_some(),
                } != inverted;
                self.pad_button(gamepad, control, pressed, out);
                if pressed && !src.no_auto_release {
                    self.register(
                        &src.key,
                        ReleaseEntry {
                            mode: src.mode.clone(),
                            any_mode: false,
                            on_press: inverted,
                            action: ReleaseAction::GamepadButton(gamepad, control),
                        },
                    );
                }
            }
            kind @ (ControlKind::Stick | ControlKind::Trigger) => {
                let v = match value {
                    Value::Axis(v) => v,
                    Value::Button(b) => f32::from(u8::from(b)),
                    Value::Hat(_) => return,
                };
                let v = if inverted { -v } else { v };
                let v = match kind {
                    ControlKind::Trigger if matches!(value, Value::Axis(_)) => (v + 1.0) / 2.0,
                    ControlKind::Trigger => v.abs(),
                    _ => v,
                };
                self.pad_axis(gamepad, control, v, out);
            }
            ControlKind::Dpad => {
                if let Value::Hat(d) = value {
                    out.gamepad_dpad(gamepad, d);
                }
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
        let shared = Arc::clone(&self.nodes[node]);
        let Node::Macro {
            ref steps,
            repeat,
            ref targets,
            ..
        } = *shared
        else {
            return;
        };
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
            let Some(step) = steps.get(run.pos).cloned() else {
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
            if let MacroStep::RunAction { action } = &step {
                if targets.contains(action)
                    && let Some(&target) = self.action_nodes.get(action)
                {
                    self.run_action(target, now, out);
                }
                continue;
            }
            self.macro_step(step, out);
        }
    }

    /// A run-action step: the action runs as a short press of a button of its own would run
    /// it, pressed now and released [`PULSE`] later.
    /// A press still held from an earlier run is released first, so presses and releases
    /// always alternate.
    fn run_action(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        if let Some(timer) = self.run_releases.remove(&node)
            && let Some(pos) = self.timers.iter().position(|t| t.id == timer)
            && let TimerKind::RunRelease { node, src } = self.timers.remove(pos).kind
        {
            self.run_release(node, &src, now, out);
        }
        let src = Source {
            key: InputKey::new(RUN_ACTION_DEVICE, InputKind::Button, node as u32),
            mode: self.modes.shared(),
            no_auto_release: false,
        };
        self.run(node, &src, &mut Value::Button(true), now, out, 0);
        self.process_release(&src.key, true, now, out);
        let timer = self.add_timer(now + PULSE, TimerKind::RunRelease { node, src });
        self.run_releases.insert(node, timer);
    }

    fn run_release(&mut self, node: NodeId, src: &Source, now: Instant, out: &mut dyn Output) {
        self.run(node, src, &mut Value::Button(false), now, out, 0);
        self.process_release(&src.key, false, now, out);
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
            } => self.vjoy_button(device, id(n), pressed, out),
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
            MacroStep::GamepadButton {
                gamepad,
                control,
                pressed,
            } => {
                if let Some(c) = GamepadControl::parse(&control) {
                    self.pad_button(gamepad, c, pressed, out);
                }
            }
            MacroStep::GamepadAxis {
                gamepad,
                control,
                value,
            } => {
                let Some(c) = GamepadControl::parse(&control) else {
                    return;
                };
                let low = if c.kind() == ControlKind::Trigger {
                    0.0
                } else {
                    -1.0
                };
                self.pad_axis(gamepad, c, (value as f32).clamp(low, 1.0), out);
            }
            MacroStep::GamepadDpad { gamepad, direction } => out.gamepad_dpad(gamepad, direction),
            // Run-action steps run in `macro_advance`, which knows the macro's targets.
            MacroStep::Pause(_) | MacroStep::RunAction { .. } | MacroStep::Other => {}
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
                    b => self.mouse_button(b, value.truthy(), out),
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
        } = (*self.nodes[node]).clone()
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
        let Node::Tempo { long, .. } = (*self.nodes[node]).clone() else {
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

    /// A logical device input changed: record it and run its bindings like a device event.
    fn feed_logical(&mut self, key: &InputKey, value: Value, now: Instant, out: &mut dyn Output) {
        if self.logical_depth >= 8 {
            return;
        }
        self.logical_depth += 1;
        self.logical_changed = true;
        self.seed(key, value);
        let mode = self.modes.shared();
        self.run_scripts(key, value, &mode, now, out);
        self.dispatch(key, value, mode, now, out);
        self.logical_depth -= 1;
    }

    /// Advances relative logical axes like [`Engine::step_relative`] does for vJoy.
    fn step_logical(&mut self, now: Instant, out: &mut dyn Output) {
        let mut moved = Vec::new();
        for (key, r) in self.logical_relative.iter_mut().filter(|(_, r)| r.active) {
            let steps = (now.saturating_duration_since(r.last_step).as_secs_f32()
                / RELATIVE_STEP.as_secs_f32())
            .floor();
            if steps < 1.0 {
                continue;
            }
            r.value = (r.value + r.delta * steps).clamp(-1.0, 1.0);
            r.last_step += RELATIVE_STEP.mul_f32(steps);
            moved.push((key.clone(), r.value));
            if r.stopping && now.saturating_duration_since(r.last_update) > RELATIVE_STOP {
                r.active = false;
            }
        }
        for (key, v) in moved {
            self.feed_logical(&key, Value::Axis(v), now, out);
        }
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
        } = (*self.nodes[node]).clone()
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
        } = (*self.nodes[node]).clone()
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
        let Node::DoubleTap { single, double, .. } = (*self.nodes[node]).clone() else {
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
        let Node::Tempo { short, long, .. } = (*self.nodes[node]).clone() else {
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
        let Some(entries) = self.releases.get_mut(key).filter(|e| !e.is_empty()) else {
            return;
        };
        let current = self.modes.shared();
        // The input's list stays in the map (emptied, its memory kept) and the entries to run
        // move into a buffer kept between events: a press and release allocate nothing.
        let mut fire = std::mem::take(&mut self.release_buffer);
        fire.extend(entries.extract_if(.., |e| {
            (e.any_mode || e.mode != current) && pressed == e.on_press
        }));
        for entry in fire.drain(..) {
            match entry.action {
                ReleaseAction::VjoyButton(device, button) => {
                    self.vjoy_button(device, button, false, out)
                }
                ReleaseAction::GamepadButton(gamepad, control) => {
                    self.pad_button(gamepad, control, false, out)
                }
                ReleaseAction::LeaveTemporary(mode) => self.leave_temporary(&mode),
                ReleaseAction::ReleaseKeys(node) => self.release_keys(node, out),
                ReleaseAction::StopListening(node) => self.stop_listening(node, out),
                ReleaseAction::StopSend(node) => self.stop_send(node, out),
                ReleaseAction::Logical(key) => {
                    self.feed_logical(&key, Value::Button(false), now, out)
                }
                ReleaseAction::AxisRelease(node) => {
                    if let Node::ButtonAxis { ref target, .. } = *Arc::clone(&self.nodes[node]) {
                        self.button_axis_event(node, target, false, now, out);
                    }
                }
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
        if self.release_buffer.capacity() < fire.capacity() {
            self.release_buffer = fire;
        }
    }
}

/// A vJoy output written during a layer change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Axis(u32, u8),
    Button(u32, u8),
    Hat(u32, u8),
}

#[derive(Debug, Clone, Copy)]
enum Write {
    Axis(u32, u8, f32),
    Button(u32, u8, bool),
    Hat(u32, u8, Option<u16>),
}

impl Write {
    fn target(&self) -> Target {
        match *self {
            Write::Axis(d, a, _) => Target::Axis(d, a),
            Write::Button(d, b, _) => Target::Button(d, b),
            Write::Hat(d, h, _) => Target::Hat(d, h),
        }
    }

    fn send(self, out: &mut dyn Output) {
        match self {
            Write::Axis(d, a, v) => out.axis(d, a, v),
            Write::Button(d, b, p) => out.button(d, b, p),
            Write::Hat(d, h, v) => out.hat(d, h, v),
        }
    }
}

/// Forwards to `out` but holds back vJoy writes (the neutral values of a layer change).
struct Deferred<'a> {
    out: &'a mut dyn Output,
    pending: Vec<Write>,
}

impl<'a> Deferred<'a> {
    fn new(out: &'a mut dyn Output) -> Self {
        Self {
            out,
            pending: Vec::new(),
        }
    }

    fn into_parts(self) -> (Vec<Write>, &'a mut dyn Output) {
        (self.pending, self.out)
    }
}

/// Forwards to `out` and notes which vJoy outputs were written.
struct Recording<'a> {
    out: &'a mut dyn Output,
    written: Vec<Target>,
}

impl<'a> Recording<'a> {
    fn new(out: &'a mut dyn Output) -> Self {
        Self {
            out,
            written: Vec::new(),
        }
    }

    fn into_parts(self) -> (Vec<Target>, &'a mut dyn Output) {
        (self.written, self.out)
    }
}

/// The non-vJoy outputs of a wrapper go straight through to `self.out`.
macro_rules! forward_output {
    () => {
        fn mouse_move(&mut self, dx: i32, dy: i32) {
            self.out.mouse_move(dx, dy)
        }
        fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
            self.out.mouse_button(button, pressed)
        }
        fn mouse_wheel(&mut self, notches: i32) {
            self.out.mouse_wheel(notches)
        }
        fn key(&mut self, scan_code: u16, extended: bool, pressed: bool) {
            self.out.key(scan_code, extended, pressed)
        }
        fn run_command(&mut self, executable: &str, arguments: &[String]) {
            self.out.run_command(executable, arguments)
        }
        fn play_sound(&mut self, file: &str, volume: u8) {
            self.out.play_sound(file, volume)
        }
        fn light_led(&mut self, light: &Light) {
            self.out.light_led(light)
        }
        fn speak(&mut self, request: SpeechRequest) {
            self.out.speak(request)
        }
        fn load_profile(&mut self, file: &str) {
            self.out.load_profile(file)
        }
        fn vjoy_state(&self, device: u32, kind: InputKind, id: u32) -> Option<Value> {
            self.out.vjoy_state(device, kind, id)
        }
        fn key_pressed(&self, scan_code: u16, extended: bool) -> bool {
            self.out.key_pressed(scan_code, extended)
        }
    };
}

impl Output for Deferred<'_> {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        self.pending.push(Write::Axis(device, axis, value));
    }
    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        self.pending.push(Write::Button(device, button, pressed));
    }
    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        self.pending.push(Write::Hat(device, hat, direction));
    }
    forward_output!();
}

impl Output for Recording<'_> {
    fn axis(&mut self, device: u32, axis: u8, value: f32) {
        self.written.push(Target::Axis(device, axis));
        self.out.axis(device, axis, value);
    }
    fn button(&mut self, device: u32, button: u8, pressed: bool) {
        self.written.push(Target::Button(device, button));
        self.out.button(device, button, pressed);
    }
    fn hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        self.written.push(Target::Hat(device, hat));
        self.out.hat(device, hat, direction);
    }
    forward_output!();
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
    for (i, step) in steps.iter().enumerate() {
        if i > 0 && !pause(step) && !pause(&steps[i - 1]) {
            out.push(MacroStep::Pause(delay));
        }
        out.push(step.clone());
    }
    out
}

/// Velocity: 1 up, -1 down, 0 for none or both of its buttons held.
fn held_direction(held: [bool; 2]) -> f32 {
    f32::from(u8::from(held[0])) - f32::from(u8::from(held[1]))
}

/// Actions with child actions (gob-core's [`Action::child_ids`]): kept when a decrease input's
/// tree is compiled. One without children yet has nothing to keep.
fn is_container(action: &Action) -> bool {
    !action.child_ids().is_empty()
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

/// What scripts reach while one of their functions runs: the engine and its outputs.
struct ScriptContext<'a> {
    engine: &'a mut Engine,
    out: &'a mut dyn Output,
    now: Instant,
}

impl ScriptApi for ScriptContext<'_> {
    fn vjoy_axis(&mut self, device: u32, axis: u8, value: f32) {
        self.engine.set_axis(device, axis, value, self.out);
    }
    fn vjoy_button(&mut self, device: u32, button: u8, pressed: bool) {
        self.engine.vjoy_button(device, button, pressed, self.out);
    }
    fn vjoy_hat(&mut self, device: u32, hat: u8, direction: Option<u16>) {
        let held = &mut self.engine.script_hats;
        held.retain(|&h| h != (device, hat));
        if direction.is_some() {
            held.push((device, hat));
        }
        self.out.hat(device, hat, direction);
    }
    fn gamepad_button(&mut self, gamepad: u32, control: &str, pressed: bool) {
        if let Some(c) = GamepadControl::parse(control) {
            self.engine.pad_button(gamepad, c, pressed, self.out);
        }
    }
    fn gamepad_axis(&mut self, gamepad: u32, control: &str, value: f32) {
        if let Some(c) = GamepadControl::parse(control) {
            let low = if c.kind() == ControlKind::Trigger {
                0.0
            } else {
                -1.0
            };
            self.engine
                .pad_axis(gamepad, c, value.clamp(low, 1.0), self.out);
        }
    }
    fn key(&mut self, key: Key, pressed: bool) {
        let held = &mut self.engine.script_keys_down;
        held.retain(|&k| k != key);
        if pressed {
            held.push(key);
        }
        self.out.key(key.scan_code, key.extended, pressed);
    }
    fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::WheelUp => self.out.mouse_wheel(1),
            MouseButton::WheelDown => self.out.mouse_wheel(-1),
            b => self.engine.mouse_button(b, pressed, self.out),
        }
    }
    fn mouse_move(&mut self, dx: i32, dy: i32) {
        self.out.mouse_move(dx, dy);
    }
    fn mouse_wheel(&mut self, notches: i32) {
        self.out.mouse_wheel(notches);
    }
    fn current_mode(&self) -> String {
        self.engine.modes.current().to_string()
    }
    fn switch_mode(&mut self, mode: &str) -> bool {
        if !self.engine.script_modes.contains(mode) {
            return false;
        }
        self.engine.modes.switch_to(mode, false);
        true
    }
    fn input(&self, input: &ScriptInput) -> Option<EventValue> {
        let kind = match input.kind {
            ScriptKind::Axis => InputKind::Axis,
            ScriptKind::Button => InputKind::Button,
            ScriptKind::Hat => InputKind::Hat,
        };
        let key = InputKey {
            device: self.engine.device(&input.device),
            kind,
            id: input.id,
        };
        match kind {
            InputKind::Axis => self.engine.axes.get(&key).map(|v| EventValue::Axis(*v)),
            InputKind::Button => self
                .engine
                .buttons
                .get(&key)
                .map(|b| EventValue::Button(*b)),
            InputKind::Hat => self.engine.hats.get(&key).map(|d| EventValue::Hat(*d)),
        }
    }
    fn speak(&mut self, text: &str) {
        self.out.speak(SpeechRequest {
            text: text.to_string(),
            voice: None,
            rate: 0.0,
            volume: 1.0,
            pitch: 0.0,
            queue: SpeechQueue::Back,
        });
    }
    fn play_sound(&mut self, file: &str, volume: u8) {
        self.out.play_sound(file, volume);
    }
    fn run_action(&mut self, name: &str) -> bool {
        let node = self
            .engine
            .action_labels
            .get(name.trim())
            .and_then(|id| self.engine.action_nodes.get(id))
            .copied();
        match node {
            Some(node) => {
                self.engine.run_action(node, self.now, self.out);
                true
            }
            None => false,
        }
    }
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
        let curve = gob_core::curve::Curve::new(gob_core::curve::CurveType::PiecewiseLinear, &pts);
        assert!((curve.eval(0.0) - 0.2).abs() < 1e-6);
        assert!((curve.eval(0.5) - 0.6).abs() < 1e-6);
        assert_eq!(curve.eval(2.0), 1.0);
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
