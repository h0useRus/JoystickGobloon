//! What a control sends: a button, an axis, a hat or one direction of a hat, written as text
//! like `button:3`, `axis:x`, `hat:1`, `hat:1:up`.

use std::fmt;
use std::str::FromStr;

/// Axis names, slot 1 to 8 (the DirectInput slots, as in gob-input and game output ids).
pub const AXES: [&str; 8] = ["x", "y", "z", "rx", "ry", "rz", "slider", "dial"];

/// A hat direction (four-way: games bind them one by one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

impl Direction {
    pub const ALL: [Direction; 4] = [Self::Up, Self::Right, Self::Down, Self::Left];

    pub fn name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Right => "right",
            Self::Down => "down",
            Self::Left => "left",
        }
    }

    /// The direction a hat reading (hundredths of a degree, 0 = up, clockwise) is closest to;
    /// diagonals count as their nearest four-way direction.
    pub fn of_angle(angle: u16) -> Self {
        match (u32::from(angle) + 4500) % 36000 / 9000 {
            0 => Self::Up,
            1 => Self::Right,
            2 => Self::Down,
            _ => Self::Left,
        }
    }
}

impl FromStr for Direction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|d| d.name() == s)
            .ok_or_else(|| format!("unknown hat direction `{s}`"))
    }
}

/// One input of a device, 1-based like gob-input's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Input {
    Button(u16),
    /// Axis slot 1..=8.
    Axis(u8),
    /// A whole hat (a hat control's four directions at once).
    Hat(u8),
    HatDirection(u8, Direction),
}

impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Button(n) => write!(f, "button:{n}"),
            Self::Axis(slot) => write!(f, "axis:{}", AXES[usize::from(slot - 1)]),
            Self::Hat(n) => write!(f, "hat:{n}"),
            Self::HatDirection(n, d) => write!(f, "hat:{n}:{}", d.name()),
        }
    }
}

impl FromStr for Input {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let bad = || format!("`{s}` is no input (button:N, axis:x, hat:N or hat:N:up)");
        let number = |text: &str| text.parse::<u16>().ok().filter(|n| *n > 0);
        let mut parts = s.split(':');
        let input = match (parts.next(), parts.next(), parts.next()) {
            (Some("button"), Some(n), None) => Self::Button(number(n).ok_or_else(bad)?),
            (Some("axis"), Some(name), None) => {
                let slot = AXES.iter().position(|a| *a == name).ok_or_else(bad)?;
                Self::Axis(slot as u8 + 1)
            }
            (Some("hat"), Some(n), dir) => {
                let n = number(n)
                    .and_then(|n| u8::try_from(n).ok())
                    .ok_or_else(bad)?;
                match dir {
                    None => Self::Hat(n),
                    Some(d) => Self::HatDirection(n, d.parse()?),
                }
            }
            _ => return Err(bad()),
        };
        if parts.next().is_some() {
            return Err(bad());
        }
        Ok(input)
    }
}

impl serde::Serialize for Input {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Input {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for text in [
            "button:1",
            "button:128",
            "axis:x",
            "axis:dial",
            "hat:1",
            "hat:2:left",
        ] {
            let input: Input = text.parse().unwrap();
            assert_eq!(input.to_string(), text);
        }
        assert_eq!("axis:rz".parse(), Ok(Input::Axis(6)));
    }

    #[test]
    fn rejects() {
        for text in [
            "button:0",
            "button",
            "axis:w",
            "hat:1:north",
            "button:1:2",
            "key:3",
        ] {
            assert!(text.parse::<Input>().is_err(), "{text}");
        }
    }

    #[test]
    fn angles() {
        assert_eq!(Direction::of_angle(0), Direction::Up);
        assert_eq!(Direction::of_angle(9000), Direction::Right);
        assert_eq!(Direction::of_angle(31500), Direction::Up);
        assert_eq!(Direction::of_angle(27000), Direction::Left);
    }
}
