//! Filters between what a device reads and what the rest of the program sees, for worn or noisy
//! hardware: a jitter threshold per axis and a debounce time per device for buttons and hats.
//! Shared by every backend; raw values pass unchanged (the calibration page needs them).

use std::time::{Duration, Instant};

use crate::{AXIS_SLOTS, Calibrations, DeviceState, InputEvent};

/// Changes smaller than this are sensor noise and never sent.
const NOISE: f32 = 0.0005;

/// One device's filtered view.
pub(crate) struct Filtered {
    /// What was sent last: changes are found against it, the snapshot shows it.
    pub out: DeviceState,
    /// Per axis slot, the smallest change that passes (in -1..1 units); 0 = off.
    jitter: [f32; AXIS_SLOTS],
    debounce: Duration,
    /// After a change, until when each button (hat) keeps it and ignores further ones.
    buttons_held: Vec<Option<Instant>>,
    hats_held: Vec<Option<Instant>>,
}

impl Filtered {
    pub fn new(state: &DeviceState) -> Self {
        Self {
            out: state.clone(),
            jitter: [0.0; AXIS_SLOTS],
            debounce: Duration::ZERO,
            buttons_held: vec![None; state.buttons.len()],
            hats_held: vec![None; state.hats.len()],
        }
    }

    /// Takes this device's jitter thresholds from `calibrations` and its debounce time.
    pub fn configure(&mut self, calibrations: &Calibrations, debounce_ms: u16) {
        for (i, j) in self.jitter.iter_mut().enumerate() {
            *j = calibrations
                .get(&(self.out.info.guid.clone(), i as u8 + 1))
                .map_or(0.0, |c| c.jitter_step());
        }
        self.debounce = Duration::from_millis(debounce_ms.into());
    }

    /// A button or hat change the device buffered: passes unless that input still holds an
    /// earlier change (its final state follows when the hold ends).
    pub fn event(&mut self, event: InputEvent, at: Instant) -> Option<InputEvent> {
        match &event {
            InputEvent::Button { id, pressed, .. } => {
                let i = usize::from(*id).checked_sub(1)?;
                if held(self.buttons_held.get(i)?, at) {
                    return None;
                }
                *self.out.buttons.get_mut(i)? = *pressed;
                self.buttons_held[i] = self.hold_from(at);
            }
            InputEvent::Hat { id, direction, .. } => {
                let i = usize::from(*id).checked_sub(1)?;
                if held(self.hats_held.get(i)?, at) {
                    return None;
                }
                *self.out.hats.get_mut(i)? = *direction;
                self.hats_held[i] = self.hold_from(at);
            }
            _ => {}
        }
        Some(event)
    }

    /// Brings [`Self::out`] up to the device's `state`, calling `send` with every change;
    /// returns whether anything changed.
    pub fn update(
        &mut self,
        state: &DeviceState,
        at: Instant,
        send: &mut dyn FnMut(InputEvent),
    ) -> bool {
        let device = || state.info.guid.clone();
        let mut changed = false;
        for &id in &state.info.axes {
            let i = usize::from(id) - 1;
            let (v, o) = (state.axes[i], self.out.axes[i]);
            let step = self.jitter[i];
            // The ends and the center always pass, so the axis reaches them.
            let passes = step <= 0.0 || (v - o).abs() >= step || v == 0.0 || v.abs() >= 1.0;
            if passes && (v - o).abs() > NOISE {
                self.out.axes[i] = v;
                changed = true;
                send(InputEvent::Axis {
                    device: device(),
                    id,
                    value: v,
                });
            }
        }
        for i in 0..state.buttons.len().min(self.out.buttons.len()) {
            if state.buttons[i] != self.out.buttons[i] && !held(&self.buttons_held[i], at) {
                self.out.buttons[i] = state.buttons[i];
                self.buttons_held[i] = self.hold_from(at);
                changed = true;
                send(InputEvent::Button {
                    device: device(),
                    id: i as u16 + 1,
                    pressed: state.buttons[i],
                });
            }
        }
        for i in 0..state.hats.len().min(self.out.hats.len()) {
            if state.hats[i] != self.out.hats[i] && !held(&self.hats_held[i], at) {
                self.out.hats[i] = state.hats[i];
                self.hats_held[i] = self.hold_from(at);
                changed = true;
                send(InputEvent::Hat {
                    device: device(),
                    id: i as u8 + 1,
                    direction: state.hats[i],
                });
            }
        }
        self.out.raw = state.raw;
        changed
    }

    /// When a held button or hat may change next; `None` without holds.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.buttons_held
            .iter()
            .chain(&self.hats_held)
            .flatten()
            .min()
            .copied()
    }

    fn hold_from(&self, at: Instant) -> Option<Instant> {
        (!self.debounce.is_zero()).then(|| at + self.debounce)
    }
}

fn held(until: &Option<Instant>, at: Instant) -> bool {
    until.is_some_and(|u| at < u)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Calibration, DeviceInfo};

    fn device() -> DeviceState {
        DeviceState::new(DeviceInfo {
            guid: "dev".into(),
            axes: vec![1],
            buttons: 1,
            hats: 1,
            ..Default::default()
        })
    }

    fn filtered(jitter: u16, debounce: u16) -> Filtered {
        let mut f = Filtered::new(&device());
        let mut cal = Calibrations::new();
        let c = Calibration {
            jitter,
            ..Default::default()
        };
        cal.insert(("dev".into(), 1), c);
        f.configure(&cal, debounce);
        f
    }

    fn axis(f: &mut Filtered, v: f32, at: Instant) -> Vec<InputEvent> {
        let mut s = device();
        s.axes[0] = v;
        let mut sent = Vec::new();
        f.update(&s, at, &mut |e| sent.push(e));
        sent
    }

    #[test]
    fn jitter_below_the_threshold_is_ignored() {
        // 1 % of the travel = 0.02.
        let mut f = filtered(10, 0);
        let t = Instant::now();
        assert_eq!(axis(&mut f, 0.5, t).len(), 1);
        for v in [0.51, 0.49, 0.515, 0.5] {
            assert!(axis(&mut f, v, t).is_empty(), "{v} is jitter");
        }
        assert_eq!(axis(&mut f, 0.53, t).len(), 1, "a real move passes");
        assert_eq!(f.out.axes[0], 0.53);
        // The ends and the center are reached even from closer than the threshold.
        axis(&mut f, 0.99, t);
        assert_eq!(axis(&mut f, 1.0, t).len(), 1);
        axis(&mut f, 0.01, t);
        assert_eq!(axis(&mut f, 0.0, t).len(), 1);
    }

    #[test]
    fn no_threshold_passes_every_change() {
        let mut f = filtered(0, 0);
        let t = Instant::now();
        assert_eq!(axis(&mut f, 0.001, t).len(), 1);
        assert!(axis(&mut f, 0.0012, t).is_empty(), "below the noise");
    }

    #[test]
    fn debounce_keeps_the_first_change_and_sends_the_final_state() {
        let mut f = filtered(0, 20);
        let t = Instant::now();
        let press = |pressed| InputEvent::Button {
            device: "dev".into(),
            id: 1,
            pressed,
        };
        // A bouncing press: the first edge passes at once, the bounces do not.
        assert!(f.event(press(true), t).is_some());
        assert!(
            f.event(press(false), t + Duration::from_millis(2))
                .is_none()
        );
        assert!(f.event(press(true), t + Duration::from_millis(4)).is_none());
        assert_eq!(f.next_deadline(), Some(t + Duration::from_millis(20)));

        // Let go during the hold: the release follows once it ends.
        let mut s = device();
        let mut sent = Vec::new();
        f.update(&s, t + Duration::from_millis(10), &mut |e| sent.push(e));
        assert!(sent.is_empty());
        f.update(&s, t + Duration::from_millis(20), &mut |e| sent.push(e));
        assert_eq!(sent, vec![press(false)]);

        // Without debounce every edge passes.
        let mut f = filtered(0, 0);
        s.buttons[0] = true;
        assert!(f.event(press(true), t).is_some());
        assert!(f.event(press(false), t).is_some());
        assert_eq!(f.next_deadline(), None);
    }
}
