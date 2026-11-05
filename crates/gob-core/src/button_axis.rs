//! Joystick Gobloon's own action "axis from buttons" (`button-axis`, not in Gremlin): buttons move
//! an axis of a virtual stick, a virtual gamepad or the logical device. The input it is bound to
//! moves the axis up; a second input stored inside the action (`decrease-*`) moves it down.
//!
//! Values and positions are axis values (-1..1); speeds and steps are parts of the full travel
//! (1.0 = from one end to the other).

use crate::Action;

pub const BUTTON_AXIS: &str = "button-axis";

/// How the buttons move the axis (`motion` property).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum AxisMotion {
    /// A press sets [`ButtonAxis::value`] (the decrease input its opposite).
    #[default]
    Set,
    /// The axis travels while a button is held, faster with time.
    Velocity,
    /// A press moves to the next of [`ButtonAxis::positions`], without wrapping.
    Stepped,
    /// A press moves one step, a bigger one the faster presses come (rotary encoders).
    Encoder,
    /// A press moves the target one step; the axis follows smoothly.
    Smooth,
}

impl AxisMotion {
    pub const ALL: [AxisMotion; 5] = [
        Self::Set,
        Self::Velocity,
        Self::Stepped,
        Self::Encoder,
        Self::Smooth,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Set => "set",
            Self::Velocity => "velocity",
            Self::Stepped => "stepped",
            Self::Encoder => "encoder",
            Self::Smooth => "smooth",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }
}

/// The axis a button-axis action moves (`output-type` property and its target).
#[derive(Debug, Clone, PartialEq)]
pub enum AxisOutput {
    /// Axis `axis` (1..8) of virtual stick `device`.
    Vjoy { device: u32, axis: u32 },
    /// A stick axis or trigger of virtual gamepad `gamepad` (`left-x`, `lt`, ...).
    Gamepad { gamepad: u32, control: String },
    /// Axis `id` of the logical device.
    Logical { id: u32 },
}

/// The second input of a button-axis action, which moves the axis down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecreaseInput {
    /// DirectInput instance GUID (or [`crate::KEYBOARD_GUID`]).
    pub device: String,
    /// `button` or `key`.
    pub input_type: String,
    pub id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ButtonAxis {
    pub output: AxisOutput,
    pub motion: AxisMotion,
    /// Set: the value a press sets.
    pub value: f32,
    /// Set: back to where it was when the button is let go.
    pub return_on_release: bool,
    /// Velocity: travels per second at first, their gain per second, the top speed.
    pub speed: f32,
    pub acceleration: f32,
    pub max_speed: f32,
    /// Stepped: ascending, [`ButtonAxis::MAX_POSITIONS`] at most.
    pub positions: Vec<f32>,
    /// Encoder, smooth: change per press; the encoder's grows up to `max_step` as presses
    /// reach `fast_rate` per second.
    pub step: f32,
    pub max_step: f32,
    pub fast_rate: f32,
    /// Encoder: back to the rest value this long after the last press; 0 = stays.
    pub spring_ms: u32,
    /// Smooth: time to reach the target.
    pub response_ms: u32,
    /// Written when the profile starts; `None` leaves the axis where it is.
    pub start: Option<f32>,
    pub decrease: Option<DecreaseInput>,
}

impl ButtonAxis {
    pub const MAX_POSITIONS: usize = 32;
    /// Steps below this do not move a 16-bit axis.
    pub const MIN_STEP: f32 = 1.0 / 32767.0;
    pub const MAX_SPEED: f32 = 100.0;
    pub const MAX_MS: u32 = 60_000;
    pub const DEFAULT_POSITIONS: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];

    /// Decodes a button-axis action; `None` for other kinds.
    pub fn of(action: &Action) -> Option<Self> {
        if action.kind != BUTTON_AXIS {
            return None;
        }
        let num = |name: &str| -> Option<f32> {
            action
                .str(name)?
                .trim()
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
        };
        let int = |name: &str| -> Option<u32> { action.str(name)?.trim().parse().ok() };
        let output = match action.str("output-type").map(str::trim) {
            Some("gamepad") => AxisOutput::Gamepad {
                gamepad: int("gamepad-id").unwrap_or(1),
                control: action.str("control").unwrap_or("left-x").trim().into(),
            },
            Some("logical") => AxisOutput::Logical {
                id: int("logical-id").unwrap_or(1),
            },
            _ => AxisOutput::Vjoy {
                device: int("vjoy-device-id").unwrap_or(1),
                axis: int("vjoy-input-id").unwrap_or(1),
            },
        };
        let step = num("step").unwrap_or(0.05).clamp(Self::MIN_STEP, 1.0);
        let speed = |name, default: f32| num(name).unwrap_or(default).clamp(0.0, Self::MAX_SPEED);
        let decrease = action.str("decrease-device-id").and_then(|device| {
            Some(DecreaseInput {
                device: device.trim().into(),
                input_type: action
                    .str("decrease-input-type")
                    .unwrap_or("button")
                    .trim()
                    .into(),
                id: int("decrease-input-id")?,
            })
        });
        Some(Self {
            output,
            motion: action
                .str("motion")
                .and_then(AxisMotion::parse)
                .unwrap_or_default(),
            value: num("set-value").unwrap_or(1.0).clamp(-1.0, 1.0),
            return_on_release: action
                .str("return-on-release")
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
            speed: speed("speed", 0.5),
            acceleration: speed("acceleration", 0.0),
            max_speed: speed("max-speed", 2.0),
            positions: action
                .str("positions")
                .map(parse_positions)
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| Self::DEFAULT_POSITIONS.to_vec()),
            step,
            max_step: num("max-step").unwrap_or(0.2).clamp(step, 1.0),
            fast_rate: num("fast-rate").unwrap_or(10.0).clamp(1.0, 1000.0),
            spring_ms: int("spring-ms").unwrap_or(0).min(Self::MAX_MS),
            response_ms: int("response-ms").unwrap_or(200).clamp(10, Self::MAX_MS),
            start: num("start-value").map(|v| v.clamp(-1.0, 1.0)),
            decrease,
        })
    }
}

/// `positions` property: numbers separated by spaces, commas or semicolons, kept in -1..1,
/// sorted and without repeats, [`ButtonAxis::MAX_POSITIONS`] at most.
pub fn parse_positions(s: &str) -> Vec<f32> {
    let mut p: Vec<f32> = s
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter_map(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(-1.0, 1.0))
        .collect();
    p.sort_by(f32::total_cmp);
    p.dedup_by(|a, b| (*a - *b).abs() < ButtonAxis::MIN_STEP);
    p.truncate(ButtonAxis::MAX_POSITIONS);
    p
}

/// The `positions` property for `positions`.
pub fn format_positions(positions: &[f32]) -> String {
    positions
        .iter()
        .map(|p| format!("{}", (p * 10000.0).round() / 10000.0))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `n` positions spread evenly over the whole axis (2 at least).
pub fn even_positions(n: usize) -> Vec<f32> {
    let n = n.clamp(2, ButtonAxis::MAX_POSITIONS);
    (0..n)
        .map(|i| -1.0 + 2.0 * i as f32 / (n - 1) as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Item, Property};

    fn action(props: &[(&str, &str)]) -> Action {
        Action {
            id: "a".into(),
            kind: BUTTON_AXIS.into(),
            items: props
                .iter()
                .map(|(n, v)| {
                    Item::Property(Property {
                        ty: "string".into(),
                        name: (*n).into(),
                        value: (*v).into(),
                    })
                })
                .collect(),
        }
    }

    #[test]
    fn defaults_and_limits() {
        let a = ButtonAxis::of(&action(&[])).unwrap();
        assert_eq!(a.output, AxisOutput::Vjoy { device: 1, axis: 1 });
        assert_eq!(a.motion, AxisMotion::Set);
        assert_eq!(a.positions, ButtonAxis::DEFAULT_POSITIONS);
        assert_eq!((a.start, a.decrease), (None, None));

        let a = ButtonAxis::of(&action(&[
            ("output-type", "gamepad"),
            ("control", "lt"),
            ("motion", "Encoder"),
            ("step", "0"),
            ("max-step", "-3"),
            ("speed", "1e9"),
            ("start-value", "5"),
            ("decrease-device-id", "{ABC}"),
            ("decrease-input-id", "7"),
        ]))
        .unwrap();
        assert_eq!(
            a.output,
            AxisOutput::Gamepad {
                gamepad: 1,
                control: "lt".into()
            }
        );
        assert_eq!(a.motion, AxisMotion::Encoder);
        assert_eq!(
            (a.step, a.max_step),
            (ButtonAxis::MIN_STEP, ButtonAxis::MIN_STEP)
        );
        assert_eq!(a.speed, ButtonAxis::MAX_SPEED);
        assert_eq!(a.start, Some(1.0));
        assert_eq!(
            a.decrease,
            Some(DecreaseInput {
                device: "{ABC}".into(),
                input_type: "button".into(),
                id: 7
            })
        );
    }

    #[test]
    fn positions_are_sorted_unique_and_bounded() {
        assert_eq!(parse_positions("1, -1;0  0 0.5 9"), [-1.0, 0.0, 0.5, 1.0]);
        assert_eq!(parse_positions("x"), Vec::<f32>::new());
        assert_eq!(
            parse_positions(&format_positions(&even_positions(100))).len(),
            32
        );
        assert_eq!(even_positions(3), [-1.0, 0.0, 1.0]);
        assert_eq!(format_positions(&[-1.0, 1.0 / 3.0]), "-1 0.3333");
    }
}
