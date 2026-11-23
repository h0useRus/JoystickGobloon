//! Filters between what a device reads and what the rest of the program sees, for worn or noisy
//! hardware: a jitter threshold per axis and a debounce time per device for buttons and hats.
//! Shared by every backend; raw values pass unchanged (the calibration page needs them).

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::platform::Buffered;
use crate::{AXIS_SLOTS, Calibrations, DeviceState, InputEvent};

/// Changes smaller than this are sensor noise and never sent.
const NOISE: f32 = 0.0005;

/// One device's filtered view.
pub(crate) struct Filtered {
    /// What was sent last: changes are found against it, the snapshot shows it.
    pub out: DeviceState,
    /// The device id its events carry, made once.
    device: Arc<str>,
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
            device: state.info.guid.as_str().into(),
            jitter: [0.0; AXIS_SLOTS],
            debounce: Duration::ZERO,
            buttons_held: vec![None; state.buttons.len()],
            hats_held: vec![None; state.hats.len()],
        }
    }

    /// Follows a device whose layout changed while connected (Linux: all buttons became
    /// readable); returns whether it did. New buttons and hats start released and centered.
    pub fn follow_layout(&mut self, state: &DeviceState) -> bool {
        if self.out.info == state.info {
            return false;
        }
        self.out.info.clone_from(&state.info);
        if *self.device != *state.info.guid {
            self.device = state.info.guid.as_str().into();
        }
        self.out.buttons.resize(state.buttons.len(), false);
        self.out.hats.resize(state.hats.len(), None);
        self.buttons_held.resize(state.buttons.len(), None);
        self.hats_held.resize(state.hats.len(), None);
        true
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
    pub fn event(&mut self, change: Buffered, at: Instant) -> Option<InputEvent> {
        let device = self.device.clone();
        Some(match change {
            Buffered::Button { id, pressed } => {
                let i = usize::from(id).checked_sub(1)?;
                if held(self.buttons_held.get(i)?, at) {
                    return None;
                }
                *self.out.buttons.get_mut(i)? = pressed;
                self.buttons_held[i] = self.hold_from(at);
                InputEvent::Button {
                    device,
                    id,
                    pressed,
                }
            }
            Buffered::Hat { id, direction } => {
                let i = usize::from(id).checked_sub(1)?;
                if held(self.hats_held.get(i)?, at) {
                    return None;
                }
                *self.out.hats.get_mut(i)? = direction;
                self.hats_held[i] = self.hold_from(at);
                InputEvent::Hat {
                    device,
                    id,
                    direction,
                }
            }
        })
    }

    /// Brings [`Self::out`] up to the device's `state`, calling `send` with every change;
    /// returns whether anything changed.
    pub fn update(
        &mut self,
        state: &DeviceState,
        at: Instant,
        send: &mut dyn FnMut(InputEvent),
    ) -> bool {
        let device = || self.device.clone();
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
mod tests;
