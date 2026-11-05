//! User scripts in Lua (Luau): Joystick Gobloon's counterpart of Joystick Gremlin's Python user
//! scripts. A script declares variables the user fills in on the Scripts page (inputs, outputs,
//! modes, numbers...), registers handlers for inputs and periodic functions, and acts through a
//! small API (virtual sticks and gamepads, keys, mouse, modes, speech, named actions).
//!
//! Each script runs in its own sandboxed Luau state: no files, no programs, and every call is
//! stopped when it runs longer than [`BUDGET`]. Nothing here reads files or knows the OS: the app
//! hands over the code, the engine implements [`ScriptApi`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gob_core::{
    InputType, KEYBOARD_GUID, Key, LOGICAL_DEVICE_GUID, MouseButton, ScriptValue, key_input_id,
};
use mlua::{Function, Lua, MultiValue, Table, Value, VmState};

const PRELUDE: &str = include_str!("prelude.luau");
/// Longest a handler or a periodic function may run before it is stopped.
pub const BUDGET: Duration = Duration::from_millis(5);
/// Longest a script's top level may run when it is loaded or described.
pub const LOAD_BUDGET: Duration = Duration::from_millis(250);
/// Errors after which a running script is switched off.
const MAX_ERRORS: u32 = 10;
/// Shortest interval of a periodic function.
const MIN_PERIOD: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKind {
    Axis,
    Button,
    Hat,
}

impl InputKind {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "axis" => Some(Self::Axis),
            "button" | "key" => Some(Self::Button),
            "hat" => Some(Self::Hat),
            _ => None,
        }
    }
}

/// An input a handler listens to: a device input, a key ([`KEYBOARD_GUID`]) or a logical input
/// ([`LOGICAL_DEVICE_GUID`]). `device` is lowercase.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptInput {
    pub device: String,
    pub kind: InputKind,
    pub id: u32,
}

impl ScriptInput {
    pub fn new(device: &str, kind: InputKind, id: u32) -> Self {
        Self {
            device: device.trim().to_ascii_lowercase(),
            kind,
            id,
        }
    }
}

/// An input's value: axis -1..1, button pressed, hat direction in hundredths of a degree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventValue {
    Axis(f32),
    Button(bool),
    Hat(Option<u16>),
}

/// What scripts can do; the engine implements it.
pub trait ScriptApi {
    fn vjoy_axis(&mut self, device: u32, axis: u8, value: f32);
    fn vjoy_button(&mut self, device: u32, button: u8, pressed: bool);
    fn vjoy_hat(&mut self, device: u32, hat: u8, direction: Option<u16>);
    /// `control` is a map-to-gamepad control name (`a`, `left-x`, `lt`...).
    fn gamepad_button(&mut self, gamepad: u32, control: &str, pressed: bool);
    fn gamepad_axis(&mut self, gamepad: u32, control: &str, value: f32);
    fn key(&mut self, key: Key, pressed: bool);
    fn mouse_button(&mut self, button: MouseButton, pressed: bool);
    fn mouse_move(&mut self, dx: i32, dy: i32);
    fn mouse_wheel(&mut self, notches: i32);
    fn current_mode(&self) -> String;
    fn switch_mode(&mut self, mode: &str);
    /// Current value of an input; `None` when unknown.
    fn input(&self, input: &ScriptInput) -> Option<EventValue>;
    fn speak(&mut self, text: &str);
    fn play_sound(&mut self, file: &str, volume: u8);
    /// Runs the action with this name; false when there is none.
    fn run_action(&mut self, name: &str) -> bool;
}

/// What a variable holds, with its limits.
#[derive(Debug, Clone, PartialEq)]
pub enum VarKind {
    Bool,
    Int { min: i64, max: i64 },
    Float { min: f64, max: f64 },
    String,
    Selection(Vec<String>),
    Mode,
    Keyboard,
    PhysicalInput(Vec<InputType>),
    VirtualOutput(Vec<InputType>),
    LogicalInput(Vec<InputType>),
}

/// A variable a script declares.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableDecl {
    pub name: String,
    pub description: String,
    pub kind: VarKind,
    /// The script works without a value.
    pub optional: bool,
    pub default: Option<ScriptValue>,
}

/// A script's error, for the log and the Scripts page.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptError {
    /// The script's name.
    pub script: String,
    pub line: Option<u32>,
    pub message: String,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "{} (line {line}): {}", self.script, self.message),
            None => write!(f, "{}: {}", self.script, self.message),
        }
    }
}

/// A script ready to run: its code and the values the user gave its variables.
#[derive(Debug, Clone)]
pub struct LoadedScript {
    pub name: String,
    pub code: String,
    pub values: Vec<(String, ScriptValue)>,
}

#[derive(Default)]
struct Collected {
    decls: Vec<VariableDecl>,
    handlers: Vec<(ScriptInput, Option<String>, Function)>,
    periodic: Vec<(Duration, Function)>,
}

/// One script's Lua state.
struct State {
    name: String,
    lua: Lua,
    host: Table,
    /// Nanoseconds after `clock` at which the running call is stopped.
    deadline: Arc<AtomicU64>,
    clock: Instant,
    collected: Arc<Mutex<Collected>>,
}

impl State {
    fn new(name: &str, values: &[(String, ScriptValue)], describing: bool) -> mlua::Result<Self> {
        let lua = Lua::new();
        let clock = Instant::now();
        let deadline = Arc::new(AtomicU64::new(u64::MAX));
        {
            let deadline = deadline.clone();
            lua.set_interrupt(move |_| {
                if clock.elapsed().as_nanos() as u64 > deadline.load(Ordering::Relaxed) {
                    Err(mlua::Error::runtime("ran too long and was stopped"))
                } else {
                    Ok(VmState::Continue)
                }
            });
        }
        let collected = Arc::new(Mutex::new(Collected::default()));
        let host = lua.create_table()?;
        let values: HashMap<String, ScriptValue> = values.iter().cloned().collect();
        {
            let collected = collected.clone();
            host.set(
                "declare",
                lua.create_function(move |lua, (kind, spec): (String, Table)| {
                    let (decl, value) = declare(&kind, &spec, &values)?;
                    let lua_value = to_lua(lua, &decl.kind, value.as_ref())?;
                    lock(&collected).decls.push(decl);
                    Ok(lua_value)
                })?,
            )?;
        }
        {
            let collected = collected.clone();
            host.set(
                "on",
                lua.create_function(
                    move |_,
                          (device, kind, id, mode, f): (
                        String,
                        String,
                        u32,
                        Option<String>,
                        Function,
                    )| {
                        let kind = InputKind::parse(&kind).ok_or_else(|| {
                            mlua::Error::runtime(format!("unknown input type '{kind}'"))
                        })?;
                        if !describing {
                            let input = ScriptInput::new(&device, kind, id);
                            lock(&collected).handlers.push((input, mode, f));
                        }
                        Ok(())
                    },
                )?,
            )?;
        }
        {
            let collected = collected.clone();
            host.set(
                "on_key",
                lua.create_function(
                    move |_, (scan, extended, mode, f): (u16, bool, Option<String>, Function)| {
                        if !describing {
                            let id = key_input_id(Key {
                                scan_code: scan,
                                extended,
                            });
                            let input = ScriptInput::new(KEYBOARD_GUID, InputKind::Button, id);
                            lock(&collected).handlers.push((input, mode, f));
                        }
                        Ok(())
                    },
                )?,
            )?;
        }
        {
            let collected = collected.clone();
            host.set(
                "every",
                lua.create_function(move |_, (seconds, f): (f64, Function)| {
                    if !describing {
                        let every = Duration::from_secs_f64(seconds.max(0.0)).max(MIN_PERIOD);
                        lock(&collected).periodic.push((every, f));
                    }
                    Ok(())
                })?,
            )?;
        }
        {
            let name = name.to_string();
            host.set(
                "log",
                lua.create_function(move |_, text: String| {
                    log::info!("script {name}: {text}");
                    Ok(())
                })?,
            )?;
        }
        lua.load(PRELUDE)
            .set_name("=prelude")
            .call::<()>(host.clone())?;
        lua.sandbox(true)?;
        Ok(Self {
            name: name.to_string(),
            lua,
            host,
            deadline,
            clock,
            collected,
        })
    }

    fn arm(&self, budget: Duration) {
        let at = (self.clock.elapsed() + budget).as_nanos() as u64;
        self.deadline.store(at, Ordering::Relaxed);
    }

    fn run_top_level(&self, code: &str) -> Result<(), ScriptError> {
        self.arm(LOAD_BUDGET);
        let result = self
            .lua
            .load(code)
            .set_name(format!("={}", self.name))
            .exec();
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        result.map_err(|e| script_error(&self.name, &e))
    }

    /// Calls `f` with `arg`, `api` reachable through the host's `call` for the call only.
    fn call(&self, f: &Function, arg: Value, api: &mut dyn ScriptApi) -> Result<(), ScriptError> {
        self.arm(BUDGET);
        let host = self.host.clone();
        let result = self.lua.scope(|scope| {
            let api = &mut *api;
            let call = scope
                .create_function_mut(move |lua, args: MultiValue| host_call(lua, args, api))?;
            host.raw_set("call", call)?;
            let r = f.call::<()>(arg);
            host.raw_set("call", Value::Nil)?;
            r
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        result.map_err(|e| script_error(&self.name, &e))
    }
}

fn lock(c: &Mutex<Collected>) -> std::sync::MutexGuard<'_, Collected> {
    c.lock().unwrap_or_else(|e| e.into_inner())
}

fn input_types(spec: &Table) -> mlua::Result<Vec<InputType>> {
    let all = vec![InputType::Axis, InputType::Button, InputType::Hat];
    let Some(types) = spec.get::<Option<Table>>("types")? else {
        return Ok(all);
    };
    let mut out = Vec::new();
    for t in types.sequence_values::<String>() {
        match InputType::parse(&t?) {
            InputType::Other => {}
            t => out.push(t),
        }
    }
    Ok(if out.is_empty() { all } else { out })
}

/// A variable's declaration and the value it gets: the user's, else the default.
fn declare(
    kind: &str,
    spec: &Table,
    values: &HashMap<String, ScriptValue>,
) -> mlua::Result<(VariableDecl, Option<ScriptValue>)> {
    let name: String = spec
        .get::<Option<String>>("name")?
        .ok_or_else(|| mlua::Error::runtime("a variable needs a name"))?;
    let description = spec
        .get::<Option<String>>("description")?
        .unwrap_or_default();
    let optional = spec.get::<Option<bool>>("optional")?.unwrap_or(true);
    let kind = match kind {
        "bool" => VarKind::Bool,
        "int" => VarKind::Int {
            min: spec.get::<Option<i64>>("min")?.unwrap_or(i64::MIN),
            max: spec.get::<Option<i64>>("max")?.unwrap_or(i64::MAX),
        },
        "float" => VarKind::Float {
            min: spec.get::<Option<f64>>("min")?.unwrap_or(f64::MIN),
            max: spec.get::<Option<f64>>("max")?.unwrap_or(f64::MAX),
        },
        "string" => VarKind::String,
        "selection" => {
            let options: Option<Table> = spec.get("options")?;
            let options = match options {
                Some(t) => t.sequence_values::<String>().collect::<mlua::Result<_>>()?,
                None => Vec::new(),
            };
            VarKind::Selection(options)
        }
        "mode" => VarKind::Mode,
        "keyboard" => VarKind::Keyboard,
        "physical-input" => VarKind::PhysicalInput(input_types(spec)?),
        "vjoy" => VarKind::VirtualOutput(input_types(spec)?),
        "logical-device" => VarKind::LogicalInput(input_types(spec)?),
        other => {
            return Err(mlua::Error::runtime(format!(
                "unknown variable type '{other}'"
            )));
        }
    };
    let default = match (&kind, spec.get::<Value>("default")?) {
        (_, Value::Nil) => None,
        (VarKind::Bool, Value::Boolean(b)) => Some(ScriptValue::Bool(b)),
        (VarKind::Int { .. }, Value::Integer(i)) => Some(ScriptValue::Int(i)),
        (VarKind::Int { .. }, Value::Number(n)) => Some(ScriptValue::Int(n as i64)),
        (VarKind::Float { .. }, Value::Integer(i)) => Some(ScriptValue::Float(i as f64)),
        (VarKind::Float { .. }, Value::Number(n)) => Some(ScriptValue::Float(n)),
        (VarKind::String, Value::String(s)) => Some(ScriptValue::String(s.to_str()?.to_string())),
        (VarKind::Mode, Value::String(s)) => Some(ScriptValue::Mode(s.to_str()?.to_string())),
        (VarKind::Selection(options), Value::String(s)) => {
            let s = s.to_str()?;
            options
                .iter()
                .position(|o| *o == *s)
                .map(ScriptValue::Selection)
        }
        (VarKind::Selection(_), Value::Integer(i)) => {
            Some(ScriptValue::Selection((i.max(1) - 1) as usize))
        }
        _ => None,
    };
    // A value of another type (the script changed) counts as not set.
    let value = values
        .get(&name)
        .filter(|v| fits(&kind, v))
        .cloned()
        .or_else(|| default.clone())
        .map(|v| clamp(&kind, v));
    Ok((
        VariableDecl {
            name,
            description,
            kind,
            optional,
            default,
        },
        value,
    ))
}

/// Whether a stored value suits a variable.
pub fn fits(kind: &VarKind, value: &ScriptValue) -> bool {
    use ScriptValue as V;
    match (kind, value) {
        (VarKind::Bool, V::Bool(_))
        | (VarKind::Int { .. }, V::Int(_))
        | (VarKind::Float { .. }, V::Float(_))
        | (VarKind::String, V::String(_))
        | (VarKind::Mode, V::Mode(_))
        | (VarKind::Keyboard, V::Keyboard(_)) => true,
        (VarKind::Selection(options), V::Selection(i)) => *i < options.len(),
        (VarKind::PhysicalInput(types), V::PhysicalInput { input_type, .. })
        | (VarKind::VirtualOutput(types), V::Vjoy { input_type, .. })
        | (VarKind::LogicalInput(types), V::LogicalDevice { input_type, .. }) => {
            types.contains(input_type)
        }
        _ => false,
    }
}

fn clamp(kind: &VarKind, value: ScriptValue) -> ScriptValue {
    match (kind, value) {
        (VarKind::Int { min, max }, ScriptValue::Int(i)) => ScriptValue::Int(i.clamp(*min, *max)),
        (VarKind::Float { min, max }, ScriptValue::Float(f)) => {
            ScriptValue::Float(f.clamp(*min, *max))
        }
        (_, v) => v,
    }
}

fn type_name(t: InputType) -> &'static str {
    match t {
        InputType::Axis => "axis",
        InputType::Hat => "hat",
        _ => "button",
    }
}

fn to_lua(lua: &Lua, kind: &VarKind, value: Option<&ScriptValue>) -> mlua::Result<Value> {
    use ScriptValue as V;
    let Some(value) = value else {
        return Ok(Value::Nil);
    };
    let input = |device: Value, t: InputType, id: u32| -> mlua::Result<Value> {
        let table = lua.create_table()?;
        table.set("device", device)?;
        table.set("type", type_name(t))?;
        table.set("id", id)?;
        Ok(Value::Table(table))
    };
    Ok(match value {
        V::Bool(b) => Value::Boolean(*b),
        V::Int(i) => Value::Integer(*i),
        V::Float(f) => Value::Number(*f),
        V::String(s) | V::Mode(s) => Value::String(lua.create_string(s)?),
        V::Selection(i) => match kind {
            VarKind::Selection(options) => match options.get(*i) {
                Some(o) => Value::String(lua.create_string(o)?),
                None => Value::Nil,
            },
            _ => Value::Nil,
        },
        V::Keyboard(k) => {
            let table = lua.create_table()?;
            table.set("scan", k.scan_code)?;
            table.set("extended", k.extended)?;
            Value::Table(table)
        }
        V::PhysicalInput {
            device,
            input_type,
            id,
        } => input(
            Value::String(lua.create_string(device.to_ascii_lowercase())?),
            *input_type,
            *id,
        )?,
        V::Vjoy {
            device,
            input_type,
            id,
        } => input(Value::Integer(i64::from(*device)), *input_type, *id)?,
        V::LogicalDevice { input_type, id } => input(
            Value::String(lua.create_string(LOGICAL_DEVICE_GUID)?),
            *input_type,
            *id,
        )?,
    })
}

/// Hat degrees (0 = up, clockwise) to hundredths of a degree; `nil` is centered.
fn hat(degrees: Option<f64>) -> Option<u16> {
    degrees.map(|d| (d.rem_euclid(360.0) * 100.0).round() as u16 % 36000)
}

fn event_value(lua: &Lua, value: Option<EventValue>) -> mlua::Result<MultiValue> {
    Ok(match value {
        None => MultiValue::new(),
        Some(EventValue::Axis(v)) => lua.pack_multi(f64::from(v))?,
        Some(EventValue::Button(b)) => lua.pack_multi(b)?,
        Some(EventValue::Hat(d)) => lua.pack_multi(d.map(|d| f64::from(d) / 100.0))?,
    })
}

/// The host's `call(op, ...)`: what the prelude's API does.
fn host_call(lua: &Lua, args: MultiValue, api: &mut dyn ScriptApi) -> mlua::Result<MultiValue> {
    let mut args = args.into_iter();
    let op: String = lua.unpack(args.next().unwrap_or(Value::Nil))?;
    let rest: MultiValue = args.collect();
    let id = |n: u32| n.min(255) as u8;
    let none = Ok(MultiValue::new());
    match op.as_str() {
        "vjoy_axis" => {
            let (d, a, v): (u32, u32, f64) = lua.unpack_multi(rest)?;
            api.vjoy_axis(d, id(a), v.clamp(-1.0, 1.0) as f32);
            none
        }
        "vjoy_button" => {
            let (d, b, p): (u32, u32, bool) = lua.unpack_multi(rest)?;
            api.vjoy_button(d, id(b), p);
            none
        }
        "vjoy_hat" => {
            let (d, h, deg): (u32, u32, Option<f64>) = lua.unpack_multi(rest)?;
            api.vjoy_hat(d, id(h), hat(deg));
            none
        }
        "pad_button" => {
            let (pad, control, p): (u32, String, bool) = lua.unpack_multi(rest)?;
            api.gamepad_button(pad, &control, p);
            none
        }
        "pad_axis" => {
            let (pad, control, v): (u32, String, f64) = lua.unpack_multi(rest)?;
            api.gamepad_axis(pad, &control, v.clamp(-1.0, 1.0) as f32);
            none
        }
        "key" => {
            let (scan, extended, p): (u16, bool, bool) = lua.unpack_multi(rest)?;
            let key = Key {
                scan_code: scan,
                extended,
            };
            api.key(key, p);
            none
        }
        "mouse_button" => {
            let (name, p): (String, bool) = lua.unpack_multi(rest)?;
            let button = MouseButton::parse(&name)
                .ok_or_else(|| mlua::Error::runtime(format!("unknown mouse button '{name}'")))?;
            api.mouse_button(button, p);
            none
        }
        "mouse_move" => {
            let (dx, dy): (f64, f64) = lua.unpack_multi(rest)?;
            api.mouse_move(dx.round() as i32, dy.round() as i32);
            none
        }
        "mouse_wheel" => {
            let n: f64 = lua.unpack_multi(rest)?;
            api.mouse_wheel(n.round() as i32);
            none
        }
        "mode" => lua.pack_multi(api.current_mode()),
        "switch_mode" => {
            let name: String = lua.unpack_multi(rest)?;
            api.switch_mode(&name);
            none
        }
        "axis" | "button" | "hat" => {
            let (device, n): (String, u32) = lua.unpack_multi(rest)?;
            let kind = InputKind::parse(&op).unwrap_or(InputKind::Button);
            event_value(lua, api.input(&ScriptInput::new(&device, kind, n)))
        }
        "speak" => {
            let text: String = lua.unpack_multi(rest)?;
            api.speak(&text);
            none
        }
        "sound" => {
            let (file, volume): (String, f64) = lua.unpack_multi(rest)?;
            api.play_sound(&file, volume.clamp(0.0, 100.0) as u8);
            none
        }
        "run_action" => {
            let name: String = lua.unpack_multi(rest)?;
            lua.pack_multi(api.run_action(&name))
        }
        _ => Err(mlua::Error::runtime(format!("unknown operation '{op}'"))),
    }
}

/// A Lua error as the user should read it, with its line when there is one.
fn script_error(script: &str, e: &mlua::Error) -> ScriptError {
    let text = match e {
        mlua::Error::CallbackError { cause, .. } => cause.to_string(),
        e => e.to_string(),
    };
    let first = text.lines().next().unwrap_or_default().trim();
    // Lua writes "<chunk>:<line>: <message>", after "runtime error: " or "syntax error: ".
    let prefix = format!("{script}:");
    let at_line = first
        .find(&prefix)
        .map(|i| &first[i + prefix.len()..])
        .and_then(|r| r.split_once(':'))
        .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    let (line, message) = match at_line {
        Some((n, msg)) => (n.parse().ok(), msg.trim().to_string()),
        None => (
            None,
            first
                .trim_start_matches("runtime error: ")
                .trim_start_matches("syntax error: ")
                .to_string(),
        ),
    };
    ScriptError {
        script: script.to_string(),
        line,
        message,
    }
}

/// The variables a script declares, for the Scripts page. Runs the script's top level without
/// registering anything.
pub fn describe(name: &str, code: &str) -> Result<Vec<VariableDecl>, ScriptError> {
    let state = State::new(name, &[], true).map_err(|e| script_error(name, &e))?;
    state.run_top_level(code)?;
    let decls = std::mem::take(&mut lock(&state.collected).decls);
    Ok(decls)
}

struct Handler {
    script: usize,
    mode: Option<String>,
    f: Function,
}

struct Periodic {
    script: usize,
    every: Duration,
    next: Option<Instant>,
    f: Function,
}

struct Running {
    state: State,
    errors: u32,
    off: bool,
}

/// The profile's scripts while it runs.
#[derive(Default)]
pub struct ScriptHost {
    scripts: Vec<Running>,
    handlers: HashMap<ScriptInput, Vec<Handler>>,
    periodic: Vec<Periodic>,
    /// Errors not yet reported to the user (one per script).
    errors: Vec<ScriptError>,
}

impl ScriptHost {
    /// Loads the scripts: a script whose top level fails is left out and its error kept.
    pub fn load(scripts: &[LoadedScript]) -> Self {
        let mut host = Self::default();
        for s in scripts {
            let state = match State::new(&s.name, &s.values, false) {
                Ok(state) => state,
                Err(e) => {
                    host.report(script_error(&s.name, &e));
                    continue;
                }
            };
            if let Err(e) = state.run_top_level(&s.code) {
                host.report(e);
                continue;
            }
            let index = host.scripts.len();
            let collected = std::mem::take(&mut *lock(&state.collected));
            for (input, mode, f) in collected.handlers {
                host.handlers.entry(input).or_default().push(Handler {
                    script: index,
                    mode,
                    f,
                });
            }
            for (every, f) in collected.periodic {
                host.periodic.push(Periodic {
                    script: index,
                    every,
                    next: None,
                    f,
                });
            }
            host.scripts.push(Running {
                state,
                errors: 0,
                off: false,
            });
        }
        host
    }

    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty() && self.periodic.is_empty()
    }

    /// Every input a handler listens to.
    pub fn inputs(&self) -> impl Iterator<Item = &ScriptInput> {
        self.handlers.keys()
    }

    /// A handler listens to `input`.
    pub fn handles(&self, input: &ScriptInput) -> bool {
        self.handlers.contains_key(input)
    }

    fn report(&mut self, e: ScriptError) {
        log::warn!("script error: {e}");
        if !self.errors.iter().any(|x| x.script == e.script) {
            self.errors.push(e);
        }
    }

    /// Errors since the last call, one per script.
    pub fn take_errors(&mut self) -> Vec<ScriptError> {
        std::mem::take(&mut self.errors)
    }

    fn run(&mut self, script: usize, f: &Function, arg: Value, api: &mut dyn ScriptApi) {
        let Some(s) = self.scripts.get_mut(script).filter(|s| !s.off) else {
            return;
        };
        if let Err(e) = s.state.call(f, arg, api) {
            s.errors += 1;
            if s.errors >= MAX_ERRORS {
                s.off = true;
                log::warn!(
                    "script {}: switched off after {MAX_ERRORS} errors",
                    s.state.name
                );
            }
            self.report(e);
        }
    }

    /// Runs the handlers of `input` for an event. `modes` is the current mode and its parents:
    /// handlers of the nearest mode that has some run, and handlers for every mode.
    pub fn event(
        &mut self,
        input: &ScriptInput,
        value: EventValue,
        modes: &[&str],
        api: &mut dyn ScriptApi,
    ) {
        let Some(list) = self.handlers.get(input) else {
            return;
        };
        let mode = modes
            .iter()
            .find(|m| list.iter().any(|h| h.mode.as_deref() == Some(**m)))
            .copied();
        let due: Vec<(usize, Function)> = list
            .iter()
            .filter(|h| h.mode.is_none() || h.mode.as_deref() == mode)
            .map(|h| (h.script, h.f.clone()))
            .collect();
        for (script, f) in due {
            let Some(lua) = self.scripts.get(script).map(|s| s.state.lua.clone()) else {
                continue;
            };
            let arg = match event_table(&lua, value) {
                Ok(t) => Value::Table(t),
                Err(_) => continue,
            };
            self.run(script, &f, arg, api);
        }
    }

    /// Runs periodic functions that are due; the first call schedules them.
    pub fn tick(&mut self, now: Instant, api: &mut dyn ScriptApi) {
        for i in 0..self.periodic.len() {
            let p = &mut self.periodic[i];
            match p.next {
                None => p.next = Some(now + p.every),
                Some(at) if at <= now => {
                    // Keep the rhythm, but never run twice for one late tick.
                    let mut next = at + p.every;
                    if next <= now {
                        next = now + p.every;
                    }
                    p.next = Some(next);
                    let (script, f) = (p.script, p.f.clone());
                    self.run(script, &f, Value::Nil, api);
                }
                Some(_) => {}
            }
        }
    }

    /// When periodic functions are due next.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.periodic.iter().filter_map(|p| p.next).min()
    }

    /// Periodic functions not scheduled yet: the next tick should come soon.
    pub fn starting(&self) -> bool {
        self.periodic.iter().any(|p| p.next.is_none())
    }
}

fn event_table(lua: &Lua, value: EventValue) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    match value {
        EventValue::Axis(v) => t.set("value", f64::from(v))?,
        EventValue::Button(b) => {
            t.set("value", b)?;
            t.set("pressed", b)?;
        }
        EventValue::Hat(d) => {
            let deg = d.map(|d| f64::from(d) / 100.0);
            t.set("value", deg)?;
            t.set("direction", deg)?;
        }
    }
    Ok(t)
}
