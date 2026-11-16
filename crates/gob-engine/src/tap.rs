//! What the running profile really sent: the last value of every virtual joystick and gamepad
//! output the driver took, and of every key. The engine thread fills it once per pass of its
//! loop, without allocating once an output has been written; the GUI reads it to show what a
//! press leads to.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{GamepadControl, InputKind, Value};

/// A button, hat or key counts as on this long after it went on, so a tap shorter than a frame
/// of the GUI still shows.
const SHOWN: Duration = Duration::from_millis(150);

/// An output of the running profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sent {
    /// Virtual joystick `device`, input `id` (1-based) of that kind.
    Joystick(u32, InputKind, u32),
    /// Virtual gamepad and its control; the d-pad as a whole is `GamepadControl::Dpad`.
    Gamepad(u32, GamepadControl),
    /// Set 1 scan code, extended.
    Key(u16, bool),
}

/// The last value of an output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tapped {
    pub value: Value,
    /// When it last went on: a button or key down, a hat off center.
    pub on_at: Option<Instant>,
}

impl Tapped {
    /// How much it shows at `now`, 0 to 1: buttons, keys and hats while on and shortly after
    /// going on, axes by how far they are from the center.
    pub fn lit(&self, now: Instant) -> f32 {
        let on = match self.value {
            Value::Button(b) => b,
            Value::Hat(d) => d.is_some(),
            Value::Axis(v) => return ((v.abs() - 0.15) / 0.5).clamp(0.0, 1.0),
        };
        let recent = self
            .on_at
            .is_some_and(|t| now.saturating_duration_since(t) < SHOWN);
        if on || recent { 1.0 } else { 0.0 }
    }
}

/// The outputs of a running profile, shared between its engine thread and the GUI.
#[derive(Debug, Clone, Default)]
pub struct OutputTap(Arc<Mutex<HashMap<Sent, Tapped>>>);

impl OutputTap {
    /// The last value of `output`; `None` when the profile has not written it.
    pub fn get(&self, output: Sent) -> Option<Tapped> {
        self.lock().get(&output).copied()
    }

    /// Reads several outputs under one lock.
    pub fn read<R>(&self, f: impl FnOnce(&HashMap<Sent, Tapped>) -> R) -> R {
        f(&self.lock())
    }

    /// Takes what a pass of the engine loop sent.
    pub(crate) fn publish(&self, sent: &[(Sent, Value)], now: Instant) {
        let mut map = self.lock();
        for &(output, value) in sent {
            let on = match value {
                Value::Button(b) => b,
                Value::Hat(d) => d.is_some(),
                Value::Axis(_) => false,
            };
            let entry = map.entry(output).or_insert(Tapped { value, on_at: None });
            entry.value = value;
            if on {
                entry.on_at = Some(now);
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Sent, Tapped>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_press_still_shows() {
        let tap = OutputTap::default();
        let button = Sent::Joystick(1, InputKind::Button, 3);
        let start = Instant::now();
        assert_eq!(tap.get(button), None);
        tap.publish(
            &[
                (button, Value::Button(true)),
                (button, Value::Button(false)),
            ],
            start,
        );
        let t = tap.get(button).unwrap();
        assert_eq!(t.value, Value::Button(false));
        assert_eq!(t.lit(start + Duration::from_millis(50)), 1.0);
        assert_eq!(t.lit(start + Duration::from_millis(500)), 0.0);
    }

    #[test]
    fn axes_show_off_center() {
        let tap = OutputTap::default();
        let axis = Sent::Joystick(1, InputKind::Axis, 1);
        let now = Instant::now();
        tap.publish(&[(axis, Value::Axis(0.05))], now);
        assert_eq!(tap.get(axis).unwrap().lit(now), 0.0);
        tap.publish(&[(axis, Value::Axis(-1.0))], now);
        assert_eq!(tap.get(axis).unwrap().lit(now), 1.0);
    }
}
