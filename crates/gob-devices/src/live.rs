//! A connected device's readings as plugin inputs: which input a person just pressed or moved
//! ([`Detector`]) and whether an input is held now ([`is_active`]).

use gob_input::{AXIS_SLOTS, DeviceState, InputEvent};

use crate::input::{Direction, Input};

/// How far an axis has to move from where it was to count as moved.
pub const AXIS_TRAVEL: f32 = 0.5;

/// Turns one device's events into the inputs a person works: a button press, a hat pushed one
/// way, an axis moved far.
#[derive(Debug, Clone)]
pub struct Detector {
    device: String,
    /// Where each axis was when it last counted (or when watching began).
    from: [f32; AXIS_SLOTS],
    /// The last readings [`Detector::poll`] saw.
    buttons: Vec<bool>,
    hats: Vec<Option<u16>>,
}

impl Detector {
    /// Watches `state`'s device from its readings now.
    pub fn new(state: &DeviceState) -> Self {
        Self {
            device: state.info.guid.clone(),
            from: state.axes,
            buttons: state.buttons.clone(),
            hats: state.hats.clone(),
        }
    }

    /// The inputs worked since the last reading, for callers that only see readings (no
    /// events): buttons that went down, hats that point a new way, axes moved far.
    pub fn poll(&mut self, state: &DeviceState) -> Vec<Input> {
        let mut out = Vec::new();
        for (i, on) in state.buttons.iter().enumerate() {
            if *on && !self.buttons.get(i).copied().unwrap_or(false) {
                out.push(Input::Button(i as u16 + 1));
            }
        }
        for (i, hat) in state.hats.iter().enumerate() {
            if let Some(angle) = hat {
                let d = Direction::of_angle(*angle);
                let was = self.hats.get(i).copied().flatten().map(Direction::of_angle);
                if was != Some(d) {
                    out.push(Input::HatDirection(i as u8 + 1, d));
                }
            }
        }
        for (i, (value, from)) in state.axes.iter().zip(self.from.iter_mut()).enumerate() {
            if (value - *from).abs() > AXIS_TRAVEL {
                *from = *value;
                out.push(Input::Axis(i as u8 + 1));
            }
        }
        self.buttons.clone_from(&state.buttons);
        self.hats.clone_from(&state.hats);
        out
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    /// The input `event` works, if it is this device's and worth showing.
    pub fn feed(&mut self, event: &InputEvent) -> Option<Input> {
        match event {
            InputEvent::Button {
                device,
                id,
                pressed: true,
            } if **device == *self.device => Some(Input::Button(*id)),
            InputEvent::Hat {
                device,
                id,
                direction: Some(angle),
            } if **device == *self.device => {
                Some(Input::HatDirection(*id, Direction::of_angle(*angle)))
            }
            InputEvent::Axis { device, id, value } if **device == *self.device => {
                let from = self.from.get_mut(usize::from(*id).checked_sub(1)?)?;
                ((value - *from).abs() > AXIS_TRAVEL).then(|| {
                    *from = *value;
                    Input::Axis(*id)
                })
            }
            _ => None,
        }
    }
}

/// Whether `input` is held on `state`'s device now; a hat direction counts on its diagonals
/// too. Axes are never "held" (see [`axis_value`]).
pub fn is_active(state: &DeviceState, input: Input) -> bool {
    let hat = |n: u8| {
        state
            .hats
            .get(usize::from(n).wrapping_sub(1))
            .copied()
            .flatten()
    };
    match input {
        Input::Button(n) => state
            .buttons
            .get(usize::from(n).wrapping_sub(1))
            .copied()
            .unwrap_or(false),
        Input::Hat(n) => hat(n).is_some(),
        Input::HatDirection(n, d) => hat(n).is_some_and(|angle| points(angle, d)),
        Input::Axis(_) => false,
    }
}

/// Where an axis input is now, -1 to 1.
pub fn axis_value(state: &DeviceState, input: Input) -> Option<f32> {
    match input {
        Input::Axis(slot) => state.axis(slot),
        _ => None,
    }
}

/// Whether a hat reading points `d` way, diagonals included.
fn points(angle: u16, d: Direction) -> bool {
    let target = match d {
        Direction::Up => 0,
        Direction::Right => 9000,
        Direction::Down => 18000,
        Direction::Left => 27000,
    };
    let diff = (i32::from(angle) - target).rem_euclid(36000);
    diff.min(36000 - diff) <= 4500
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gob_input::DeviceInfo;

    use super::*;

    fn state() -> DeviceState {
        DeviceState::new(DeviceInfo {
            guid: "dev".into(),
            axes: vec![1, 2],
            buttons: 4,
            hats: 1,
            ..Default::default()
        })
    }

    #[test]
    fn polls_readings() {
        let mut s = state();
        let mut d = Detector::new(&s);
        assert!(d.poll(&s).is_empty());
        s.buttons[1] = true;
        s.hats[0] = Some(9000);
        s.axes[0] = 0.8;
        assert_eq!(
            d.poll(&s),
            [
                Input::Button(2),
                Input::HatDirection(1, Direction::Right),
                Input::Axis(1)
            ]
        );
        // Still held, nothing new.
        assert!(d.poll(&s).is_empty());
    }

    #[test]
    fn detects() {
        let mut d = Detector::new(&state());
        let dev: Arc<str> = "dev".into();
        let other: Arc<str> = "other".into();
        let button = |device: &Arc<str>, pressed| InputEvent::Button {
            device: device.clone(),
            id: 2,
            pressed,
        };
        assert_eq!(d.feed(&button(&dev, true)), Some(Input::Button(2)));
        assert_eq!(d.feed(&button(&dev, false)), None);
        assert_eq!(d.feed(&button(&other, true)), None);
        let axis = |value| InputEvent::Axis {
            device: dev.clone(),
            id: 1,
            value,
        };
        assert_eq!(d.feed(&axis(0.3)), None);
        assert_eq!(d.feed(&axis(0.9)), Some(Input::Axis(1)));
        assert_eq!(d.feed(&axis(1.0)), None);
        assert_eq!(d.feed(&axis(0.2)), Some(Input::Axis(1)));
        let hat = InputEvent::Hat {
            device: dev.clone(),
            id: 1,
            direction: Some(27000),
        };
        assert_eq!(d.feed(&hat), Some(Input::HatDirection(1, Direction::Left)));
    }

    #[test]
    fn active() {
        let mut s = state();
        s.buttons[0] = true;
        s.hats[0] = Some(31500);
        assert!(is_active(&s, Input::Button(1)));
        assert!(!is_active(&s, Input::Button(2)));
        assert!(is_active(&s, Input::HatDirection(1, Direction::Up)));
        assert!(is_active(&s, Input::HatDirection(1, Direction::Left)));
        assert!(!is_active(&s, Input::HatDirection(1, Direction::Down)));
        assert!(is_active(&s, Input::Hat(1)));
        s.axes[1] = -0.5;
        assert_eq!(axis_value(&s, Input::Axis(2)), Some(-0.5));
        assert_eq!(axis_value(&s, Input::Axis(3)), None);
    }
}
