//! Game bindings (Joystick Gobloon only): which game action each virtual output, key combination
//! or mouse button drives, per game, so an export plugin can write the game's own binding files.
//! Bindings belong to outputs, not to action cards: a game sees the outputs. Game and action ids
//! are opaque here; only the plugins know what they mean.
//!
//! Keyboard and mouse bindings stay the game's own until an action's are set here: from then on
//! the profile holds them all for that action (`<bind>` with a `key:`/`mouse:` output, or
//! `<clear>` when it has none).
//!
//! ```xml
//! <game-bindings>
//!   <game id="sample" target="main" version="1.1">
//!     <device output="vjoy:1" game-device="js1"/>
//!     <bind action="gear" output="vjoy:1:button:5"/>
//!     <bind action="roll" output="vjoy:2:axis:x" invert="true"/>
//!     <bind action="eject" output="key:0x138+key:0x15"/>
//!     <clear action="lights"/>
//!   </game>
//! </game-bindings>
//! ```

use crate::Key;
use crate::action::{ActionKind, InputType, MacroStep, MouseAction, MouseButton};
use crate::profile::{Profile, Section, VirtualOutput};

/// vJoy axis names in output ids, axis 1 to 8 (the DirectInput slots).
const AXES: [&str; 8] = ["x", "y", "z", "rx", "ry", "rz", "slider", "dial"];

/// Something a game can bind: a virtual output, or keys and mouse buttons pressed together.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameOutput {
    Virtual(VirtualOutput),
    /// One direction of a hat (or a gamepad's d-pad): games bind them one by one.
    HatDirection(VirtualOutput, HatDirection),
    Keys(Combo),
}

/// A key or a mouse button of a [`Combo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComboPart {
    Key(Key),
    Mouse(MouseButton),
}

/// Mouse button names in output ids, in the order of [`MouseButton::ALL`].
const MOUSE: [&str; 7] = [
    "left",
    "right",
    "middle",
    "forward",
    "back",
    "wheel-up",
    "wheel-down",
];

/// Ctrl, Shift, Alt and the Windows keys, left and right (set 1 scan codes).
fn is_modifier(key: Key) -> bool {
    matches!(
        (key.scan_code, key.extended),
        (0x1d | 0x38, _) | (0x2a | 0x36, false) | (0x5b | 0x5c, true)
    )
}

impl ComboPart {
    /// Modifier keys first, then the other keys, then mouse buttons.
    fn order(self) -> (u8, u32) {
        match self {
            Self::Key(k) if is_modifier(k) => (0, crate::key_input_id(k)),
            Self::Key(k) => (1, crate::key_input_id(k)),
            Self::Mouse(b) => (
                2,
                MouseButton::ALL.iter().position(|x| *x == b).unwrap_or(0) as u32,
            ),
        }
    }

    fn parse(s: &str) -> Option<Self> {
        if let Some(id) = s.strip_prefix("key:") {
            let id = u32::from_str_radix(id.strip_prefix("0x")?, 16).ok()?;
            return (id <= 0x1FF).then(|| Self::Key(crate::key_from_input_id(id)));
        }
        let name = s.strip_prefix("mouse:")?;
        let i = MOUSE.iter().position(|m| *m == name)?;
        Some(Self::Mouse(MouseButton::ALL[i]))
    }
}

impl std::fmt::Display for ComboPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key(k) => write!(f, "key:0x{:x}", crate::key_input_id(*k)),
            Self::Mouse(b) => {
                let i = MouseButton::ALL.iter().position(|x| x == b).unwrap_or(0);
                write!(f, "mouse:{}", MOUSE[i])
            }
        }
    }
}

/// Keys and mouse buttons a game takes pressed together: one key, `Alt+Y`, a mouse button,
/// `Alt` + a mouse button. The parts are kept in one order (modifier keys, other keys, mouse
/// buttons), so the order they were pressed in does not matter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Combo(Vec<ComboPart>);

impl Combo {
    /// `None` without parts.
    pub fn new(parts: impl IntoIterator<Item = ComboPart>) -> Option<Self> {
        let mut list: Vec<ComboPart> = Vec::new();
        for p in parts {
            if !list.contains(&p) {
                list.push(p);
            }
        }
        list.sort_by_key(|p| p.order());
        (!list.is_empty()).then_some(Self(list))
    }

    pub fn key(key: Key) -> Self {
        Self(vec![ComboPart::Key(key)])
    }

    pub fn mouse(button: MouseButton) -> Self {
        Self(vec![ComboPart::Mouse(button)])
    }

    pub fn parts(&self) -> &[ComboPart] {
        &self.0
    }

    /// `key:0x2e`, `key:0x138+key:0x15`, `mouse:left`, `key:0x38+mouse:right`.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Option<Vec<ComboPart>> = s.split('+').map(ComboPart::parse).collect();
        Self::new(parts?)
    }
}

impl std::fmt::Display for Combo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (n, p) in self.0.iter().enumerate() {
            if n > 0 {
                f.write_str("+")?;
            }
            write!(f, "{p}")?;
        }
        Ok(())
    }
}

/// A hat direction games bind on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HatDirection {
    Up,
    Right,
    Down,
    Left,
}

impl HatDirection {
    pub const ALL: [Self; 4] = [Self::Up, Self::Right, Self::Down, Self::Left];

    pub fn name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Right => "right",
            Self::Down => "down",
            Self::Left => "left",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == s)
    }
}

impl GameOutput {
    /// Output ids: `vjoy:1:button:5`, `vjoy:1:axis:x`, `vjoy:1:hat:1`, `gamepad:1:a`,
    /// `logical:axis:12`, `key:0x2e` (extended keys `key:0x12e`), `mouse:left`, and keys and
    /// mouse buttons together joined with `+` (`key:0x138+key:0x15`).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.starts_with("key:") || s.starts_with("mouse:") {
            return Combo::parse(s).map(Self::Keys);
        }
        if let Some((base, dir)) = s.rsplit_once(':')
            && let Some(dir) = HatDirection::parse(dir)
        {
            return match Self::parse(base)? {
                Self::Virtual(
                    o @ (VirtualOutput::Vjoy {
                        input_type: InputType::Hat,
                        ..
                    }
                    | VirtualOutput::Logical {
                        input_type: InputType::Hat,
                        ..
                    }
                    | VirtualOutput::Gamepad { .. }),
                ) => Some(Self::HatDirection(o, dir)),
                _ => None,
            };
        }
        let parts: Vec<&str> = s.split(':').collect();
        let num = |p: &str| p.parse::<u32>().ok().filter(|n| *n > 0);
        let typed = |t: &str, id: &str| -> Option<(InputType, u32)> {
            match t {
                "button" => Some((InputType::Button, num(id)?)),
                "hat" => Some((InputType::Hat, num(id)?)),
                "axis" => {
                    let id = match AXES.iter().position(|a| *a == id) {
                        Some(i) => i as u32 + 1,
                        None => num(id).filter(|n| *n <= 8)?,
                    };
                    Some((InputType::Axis, id))
                }
                _ => None,
            }
        };
        Some(match parts.as_slice() {
            ["vjoy", device, t, id] => {
                let (input_type, id) = typed(t, id)?;
                Self::Virtual(VirtualOutput::Vjoy {
                    device: num(device)?,
                    input_type,
                    id,
                })
            }
            ["gamepad", gamepad, control] if !control.is_empty() => {
                Self::Virtual(VirtualOutput::Gamepad {
                    gamepad: num(gamepad)?,
                    control: control.to_string(),
                })
            }
            ["logical", t, id] => {
                let input_type = match *t {
                    "axis" => InputType::Axis,
                    "button" => InputType::Button,
                    "hat" => InputType::Hat,
                    _ => return None,
                };
                let id = num(id)?;
                Self::Virtual(VirtualOutput::Logical { input_type, id })
            }
            _ => return None,
        })
    }

    /// The device part of the output id (`vjoy:1`, `gamepad:1`), for device links.
    pub fn device(&self) -> Option<String> {
        match self.virtual_output()? {
            VirtualOutput::Vjoy { device, .. } => Some(format!("vjoy:{device}")),
            VirtualOutput::Gamepad { gamepad, .. } => Some(format!("gamepad:{gamepad}")),
            VirtualOutput::Logical { .. } => None,
        }
    }

    /// The virtual output behind it (the whole hat for a direction).
    pub fn virtual_output(&self) -> Option<&VirtualOutput> {
        match self {
            Self::Virtual(o) | Self::HatDirection(o, _) => Some(o),
            Self::Keys(_) => None,
        }
    }

    /// The keys and mouse buttons of a keyboard or mouse binding.
    pub fn combo(&self) -> Option<&Combo> {
        match self {
            Self::Keys(c) => Some(c),
            _ => None,
        }
    }
}

fn typed(t: InputType, id: u32) -> String {
    match t {
        InputType::Axis => match AXES.get(id.wrapping_sub(1) as usize) {
            Some(a) => format!("axis:{a}"),
            None => format!("axis:{id}"),
        },
        InputType::Hat => format!("hat:{id}"),
        _ => format!("button:{id}"),
    }
}

impl std::fmt::Display for GameOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Virtual(VirtualOutput::Vjoy {
                device,
                input_type,
                id,
            }) => write!(f, "vjoy:{device}:{}", typed(*input_type, *id)),
            Self::Virtual(VirtualOutput::Gamepad { gamepad, control }) => {
                write!(f, "gamepad:{gamepad}:{control}")
            }
            // The logical device's axes are not the 8 DirectInput slots: plain numbers.
            Self::Virtual(VirtualOutput::Logical { input_type, id }) => {
                let t = match input_type {
                    InputType::Axis => "axis",
                    InputType::Hat => "hat",
                    _ => "button",
                };
                write!(f, "logical:{t}:{id}")
            }
            Self::HatDirection(o, dir) => {
                write!(f, "{}:{}", Self::Virtual(o.clone()), dir.name())
            }
            Self::Keys(c) => write!(f, "{c}"),
        }
    }
}

/// A game action driven by an output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameBind {
    pub action: String,
    pub output: GameOutput,
    pub invert: bool,
}

/// How the game numbers one of the profile's virtual devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDevice {
    /// `vjoy:1`, `gamepad:1`.
    pub output: String,
    /// The game's own name for it (`js1`).
    pub game_device: String,
}

/// The bindings for one game (`<game>`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameBindings {
    /// The export plugin's id.
    pub game: String,
    /// The install or channel last exported to.
    pub target: String,
    /// The game version the bindings were last exported for.
    pub version: Option<String>,
    pub devices: Vec<GameDevice>,
    pub binds: Vec<GameBind>,
    /// Actions the profile leaves without any keyboard or mouse binding.
    pub cleared: Vec<String>,
}

impl GameBindings {
    /// The keyboard and mouse bindings the profile holds for `action`; `None` while they are
    /// the game's own.
    pub fn keys_of(&self, action: &str) -> Option<Vec<&Combo>> {
        let held: Vec<&Combo> = self
            .binds
            .iter()
            .filter(|b| b.action == action)
            .filter_map(|b| b.output.combo())
            .collect();
        (!held.is_empty() || self.cleared.iter().any(|a| a == action)).then_some(held)
    }

    /// The actions an output drives.
    pub fn actions_of<'a>(&'a self, output: &'a GameOutput) -> impl Iterator<Item = &'a str> {
        self.binds
            .iter()
            .filter(move |b| &b.output == output)
            .map(|b| b.action.as_str())
    }

    /// The outputs driving an action.
    pub fn outputs_of<'a>(&'a self, action: &'a str) -> impl Iterator<Item = &'a GameOutput> {
        self.binds
            .iter()
            .filter(move |b| b.action == action)
            .map(|b| &b.output)
    }

    pub fn game_device(&self, output: &str) -> Option<&str> {
        self.devices
            .iter()
            .find(|d| d.output == output)
            .map(|d| d.game_device.as_str())
    }
}

impl Profile {
    pub fn game_bindings(&self, game: &str) -> Option<&GameBindings> {
        self.game_bindings.iter().find(|g| g.game == game)
    }

    /// The bindings for `game`, added when missing.
    pub fn game_bindings_mut(&mut self, game: &str) -> &mut GameBindings {
        if let Some(i) = self.game_bindings.iter().position(|g| g.game == game) {
            return &mut self.game_bindings[i];
        }
        self.add_section(Section::GameBindings);
        self.game_bindings.push(GameBindings {
            game: game.to_string(),
            ..GameBindings::default()
        });
        self.game_bindings.last_mut().expect("just pushed")
    }

    /// Lets `output` drive `action` (once); false when it already does.
    pub fn bind_game_action(&mut self, game: &str, action: &str, output: GameOutput) -> bool {
        let g = self.game_bindings_mut(game);
        if output.combo().is_some() {
            g.cleared.retain(|a| a != action);
        }
        if g.binds
            .iter()
            .any(|b| b.action == action && b.output == output)
        {
            return false;
        }
        g.binds.push(GameBind {
            action: action.to_string(),
            output,
            invert: false,
        });
        true
    }

    /// Removes the link between `action` and `output`; `None` removes every output of the
    /// action. Drops the game when nothing is left in it.
    pub fn unbind_game_action(&mut self, game: &str, action: &str, output: Option<&GameOutput>) {
        let Some(g) = self.game_bindings.iter_mut().find(|g| g.game == game) else {
            return;
        };
        g.binds
            .retain(|b| !(b.action == action && output.is_none_or(|o| &b.output == o)));
        self.drop_empty_games();
    }

    /// The keyboard and mouse bindings of `action`: `Some` makes them the profile's (an empty
    /// list leaves the action without any), `None` gives them back to the game.
    pub fn set_game_keys(&mut self, game: &str, action: &str, keys: Option<Vec<Combo>>) {
        let g = self.game_bindings_mut(game);
        g.binds
            .retain(|b| !(b.action == action && b.output.combo().is_some()));
        g.cleared.retain(|a| a != action);
        match keys {
            Some(keys) if keys.is_empty() => g.cleared.push(action.to_string()),
            Some(keys) => {
                for combo in keys {
                    let output = GameOutput::Keys(combo);
                    if !g
                        .binds
                        .iter()
                        .any(|b| b.action == action && b.output == output)
                    {
                        g.binds.push(GameBind {
                            action: action.to_string(),
                            output,
                            invert: false,
                        });
                    }
                }
            }
            None => {}
        }
        self.drop_empty_games();
    }

    /// Replaces how the game numbers the virtual devices.
    pub fn set_game_devices(&mut self, game: &str, devices: Vec<GameDevice>) {
        self.game_bindings_mut(game).devices = devices;
        self.drop_empty_games();
    }

    fn drop_empty_games(&mut self) {
        self.game_bindings
            .retain(|g| !g.binds.is_empty() || !g.devices.is_empty() || !g.cleared.is_empty());
    }

    /// The keys and mouse buttons the profile's map-to-keyboard, map-to-mouse and macro actions
    /// send, each with the bindings sending it. `mode` limits them as in
    /// [`Self::output_usage`].
    pub fn key_usage(&self, mode: Option<&str>) -> Vec<(Combo, Vec<crate::OutputUse>)> {
        let mut usage: Vec<(Combo, Vec<crate::OutputUse>)> = Vec::new();
        let index = self.action_index();
        for (binding, b) in self.inputs.iter().enumerate() {
            if mode.is_some_and(|m| !self.modes_overlap(&b.mode, m)) {
                continue;
            }
            let mut seen = std::collections::HashSet::new();
            let mut stack: Vec<&str> = b.configs.iter().map(|c| c.root_action.as_str()).collect();
            while let Some(id) = stack.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(action) = index.get(id) else {
                    continue;
                };
                for combo in sent_combos(&action.kind()) {
                    let n = match usage.iter().position(|(c, _)| *c == combo) {
                        Some(n) => n,
                        None => {
                            usage.push((combo, Vec::new()));
                            usage.len() - 1
                        }
                    };
                    let uses = &mut usage[n].1;
                    if uses.last().is_none_or(|u| u.binding != binding) {
                        uses.push(crate::OutputUse {
                            binding,
                            action: id.to_string(),
                        });
                    }
                }
                stack.extend(action.child_ids());
            }
        }
        usage
    }
}

/// What an action sends to the keyboard and mouse: the keys of a map-to-keyboard, the button of
/// a map-to-mouse, and of a macro every set of keys and buttons it holds down together.
fn sent_combos(kind: &ActionKind) -> Vec<Combo> {
    match kind {
        ActionKind::MapToKeyboard { keys } => Combo::new(keys.iter().map(|k| ComboPart::Key(*k)))
            .into_iter()
            .collect(),
        ActionKind::MapToMouse(MouseAction::Button(b)) => vec![Combo::mouse(*b)],
        ActionKind::Macro { steps, .. } => {
            let mut out: Vec<Combo> = Vec::new();
            let mut held: Vec<ComboPart> = Vec::new();
            // A press was added since the held set was last taken.
            let mut grown = false;
            let take = |held: &[ComboPart], out: &mut Vec<Combo>| {
                if let Some(c) = Combo::new(held.iter().copied())
                    && !out.contains(&c)
                {
                    out.push(c);
                }
            };
            for step in steps {
                let (part, pressed) = match step {
                    MacroStep::Key {
                        scan_code,
                        extended,
                        pressed,
                    } => (
                        ComboPart::Key(Key {
                            scan_code: *scan_code,
                            extended: *extended,
                        }),
                        *pressed,
                    ),
                    MacroStep::MouseButton { button, pressed } => {
                        (ComboPart::Mouse(*button), *pressed)
                    }
                    _ => continue,
                };
                if pressed {
                    if !held.contains(&part) {
                        held.push(part);
                    }
                    grown = true;
                } else {
                    if grown {
                        take(&held, &mut out);
                        grown = false;
                    }
                    held.retain(|p| *p != part);
                }
            }
            if grown {
                take(&held, &mut out);
            }
            out
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_ids_round_trip() {
        for id in [
            "vjoy:1:button:5",
            "vjoy:2:axis:x",
            "vjoy:2:axis:dial",
            "vjoy:1:hat:1",
            "vjoy:1:hat:2:left",
            "gamepad:1:dpad:up",
            "gamepad:1:left-x",
            "logical:axis:2",
            "key:0x2e",
            "key:0x14b",
            "key:0x138+key:0x15",
            "mouse:left",
            "key:0x38+mouse:right",
            "mouse:wheel-up",
        ] {
            let o = GameOutput::parse(id).unwrap_or_else(|| panic!("{id}"));
            assert_eq!(o.to_string(), id);
        }
        assert_eq!(
            GameOutput::parse("vjoy:1:axis:3").unwrap().to_string(),
            "vjoy:1:axis:z"
        );
        for bad in [
            "",
            "vjoy:0:button:1",
            "vjoy:1:axis:9",
            "key:46",
            "key:0x300",
            "x:1",
            "vjoy:1:button:1:up",
            "key:0x2e:up",
            "key:0x2e+",
            "mouse:up",
            "key:0x2e+vjoy:1:button:1",
        ] {
            assert!(GameOutput::parse(bad).is_none(), "{bad}");
        }
        let k = GameOutput::parse("key:0x14b").unwrap();
        assert_eq!(
            k,
            GameOutput::Keys(Combo::key(crate::Key {
                scan_code: 0x4b,
                extended: true
            }))
        );
        // The order pressed in does not matter: modifier keys come first.
        assert_eq!(
            GameOutput::parse("key:0x15+mouse:left+key:0x138+key:0x15")
                .unwrap()
                .to_string(),
            "key:0x138+key:0x15+mouse:left"
        );
        assert_eq!(
            GameOutput::parse("vjoy:3:button:1")
                .unwrap()
                .device()
                .unwrap(),
            "vjoy:3"
        );
    }

    #[test]
    fn binding_and_unbinding() {
        let mut p = Profile::default();
        let out = GameOutput::parse("vjoy:1:button:5").unwrap();
        assert!(p.bind_game_action("g", "gear", out.clone()));
        assert!(!p.bind_game_action("g", "gear", out.clone()));
        assert!(p.bind_game_action("g", "lights", out.clone()));
        assert_eq!(p.order.last(), Some(&Section::GameBindings));
        let g = p.game_bindings("g").unwrap();
        assert_eq!(g.actions_of(&out).collect::<Vec<_>>(), ["gear", "lights"]);
        assert_eq!(g.outputs_of("gear").count(), 1);
        assert!(p.needs_own_format());
        p.unbind_game_action("g", "gear", None);
        p.unbind_game_action("g", "lights", Some(&out));
        assert!(p.game_bindings.is_empty());
    }

    #[test]
    fn key_usage_lists_what_inputs_send() {
        use crate::edit::NewAction;
        let mut p = Profile::default();
        let key = |scan_code| Key {
            scan_code,
            extended: false,
        };
        let root = p.bind("dev", "button", "3", "Default");
        let id = p
            .add_child(
                &root,
                "actions",
                crate::Action::new(&NewAction::MapToKeyboard),
            )
            .unwrap();
        p.set_keys(&id, &[key(0x15), key(0x38)]);
        let usage = p.key_usage(None);
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].0.to_string(), "key:0x38+key:0x15");
        assert_eq!(usage[0].1[0].binding, 0);
    }

    #[test]
    fn keys_are_the_games_until_set() {
        let mut p = Profile::default();
        let combo = |s| Combo::parse(s).unwrap();
        p.set_game_keys("g", "eject", Some(vec![combo("key:0x138+key:0x15")]));
        p.set_game_keys("g", "lights", Some(Vec::new()));
        let g = p.game_bindings("g").unwrap();
        assert_eq!(g.keys_of("eject").unwrap().len(), 1);
        assert_eq!(g.keys_of("lights"), Some(Vec::new()));
        assert_eq!(g.keys_of("gear"), None);
        // Binding a key ends "none at all".
        p.bind_game_action("g", "lights", GameOutput::Keys(combo("key:0x26")));
        assert!(p.game_bindings("g").unwrap().cleared.is_empty());
        p.set_game_keys("g", "lights", None);
        p.set_game_keys("g", "eject", None);
        assert!(p.game_bindings.is_empty());
    }
}
