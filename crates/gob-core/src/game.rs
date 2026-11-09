//! Game bindings (Joystick Gobloon only): which game action each virtual output or key of the
//! profile drives, per game, so an export plugin can write the game's own binding files.
//! Bindings belong to outputs, not to action cards: a game sees the outputs. Game and action ids
//! are opaque here; only the plugins know what they mean.
//!
//! ```xml
//! <game-bindings>
//!   <game id="sample" target="main" version="1.1">
//!     <device output="vjoy:1" game-device="js1"/>
//!     <bind action="gear" output="vjoy:1:button:5"/>
//!     <bind action="roll" output="vjoy:2:axis:x" invert="true"/>
//!   </game>
//! </game-bindings>
//! ```

use crate::action::InputType;
use crate::profile::{Profile, Section, VirtualOutput};

/// vJoy axis names in output ids, axis 1 to 8 (the DirectInput slots).
const AXES: [&str; 8] = ["x", "y", "z", "rx", "ry", "rz", "slider", "dial"];

/// Something a game can bind: a virtual output or a key the profile sends.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameOutput {
    Virtual(VirtualOutput),
    /// One direction of a hat (or a gamepad's d-pad): games bind them one by one.
    HatDirection(VirtualOutput, HatDirection),
    Key(crate::Key),
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
    /// `logical:axis:12`, `key:0x2e` (extended keys `key:0x12e`).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
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
            ["key", id] => {
                let id = u32::from_str_radix(id.strip_prefix("0x")?, 16).ok()?;
                if id > 0x1FF {
                    return None;
                }
                Self::Key(crate::key_from_input_id(id))
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
            Self::Key(_) => None,
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
            Self::Key(k) => write!(f, "key:0x{:x}", crate::key_input_id(*k)),
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
}

impl GameBindings {
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

    /// Replaces how the game numbers the virtual devices.
    pub fn set_game_devices(&mut self, game: &str, devices: Vec<GameDevice>) {
        self.game_bindings_mut(game).devices = devices;
        self.drop_empty_games();
    }

    fn drop_empty_games(&mut self) {
        self.game_bindings
            .retain(|g| !g.binds.is_empty() || !g.devices.is_empty());
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
        ] {
            assert!(GameOutput::parse(bad).is_none(), "{bad}");
        }
        let k = GameOutput::parse("key:0x14b").unwrap();
        assert_eq!(
            k,
            GameOutput::Key(crate::Key {
                scan_code: 0x4b,
                extended: true
            })
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
}
