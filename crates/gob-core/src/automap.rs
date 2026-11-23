//! Automapping (the new profile wizard): every input of a physical device gets a Map to vJoy
//! onto one virtual joystick; controllers sharing one fill it one after another.

use crate::action::{Action, InputType};
use crate::edit::NewAction;
use crate::profile::{Device, Profile};

/// The inputs of a device: axis slots (1 = X ... 8 = Slider 1), button and hat counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    pub axes: Vec<u8>,
    pub buttons: u32,
    pub hats: u32,
}

impl Layout {
    /// What of `self` does not fit into `to`: missing axes, buttons and hats.
    pub fn missing_in(&self, to: &Layout) -> Layout {
        Layout {
            axes: self.axes.iter().skip(to.axes.len()).copied().collect(),
            buttons: self.buttons.saturating_sub(to.buttons),
            hats: self.hats.saturating_sub(to.hats),
        }
    }

    /// What of `self` is left after `used` (what earlier controllers took): the axes not in
    /// `used`, the buttons and hats past its counts.
    pub fn without(&self, used: &Layout) -> Layout {
        Layout {
            axes: self
                .axes
                .iter()
                .copied()
                .filter(|a| !used.axes.contains(a))
                .collect(),
            buttons: self.buttons.saturating_sub(used.buttons),
            hats: self.hats.saturating_sub(used.hats),
        }
    }

    /// Takes room for `self` on `to` past `used` (what earlier controllers took), as
    /// [`Profile::automap`] does: adds what it takes to `used` and returns what does not fit.
    pub fn take(&self, to: &Layout, used: &mut Layout) -> Layout {
        let free = to.without(used);
        used.axes.extend(
            axis_targets(&self.axes, &free.axes)
                .into_iter()
                .filter_map(|(_, t)| t),
        );
        used.buttons += self.buttons.min(free.buttons);
        used.hats += self.hats.min(free.hats);
        self.missing_in(&free)
    }

    pub fn is_empty(&self) -> bool {
        self.axes.is_empty() && self.buttons == 0 && self.hats == 0
    }
}

/// Which virtual axis each physical axis goes to: the same slot where the virtual joystick has
/// it, the others to its remaining axes in order; `None` when it has no axis left.
pub fn axis_targets(from: &[u8], to: &[u8]) -> Vec<(u8, Option<u8>)> {
    let mut free: Vec<u8> = to.iter().copied().filter(|a| !from.contains(a)).collect();
    from.iter()
        .map(|&a| {
            let target = if to.contains(&a) {
                Some(a)
            } else if free.is_empty() {
                None
            } else {
                Some(free.remove(0))
            };
            (a, target)
        })
        .collect()
}

impl Profile {
    /// Binds every input of physical device `device` (`from`) in `mode` to a Map to vJoy onto
    /// vJoy device `vjoy` (`to`) past what `used` says earlier controllers took on it: axes as
    /// [`axis_targets`] says over the free ones, buttons and hats numbered on from the last used
    /// one. Inputs left without room stay unbound and are returned; what this device takes is
    /// added to `used` ([`Layout::take`]). The device joins the profile's device list.
    #[allow(clippy::too_many_arguments)]
    pub fn automap(
        &mut self,
        device: &str,
        name: &str,
        from: &Layout,
        vjoy: u32,
        to: &Layout,
        used: &mut Layout,
        mode: &str,
    ) -> Layout {
        if !self
            .devices
            .iter()
            .any(|d| d.id.eq_ignore_ascii_case(device))
        {
            self.devices.push(Device {
                id: device.to_string(),
                name: name.to_string(),
            });
        }
        let map = |p: &mut Profile, input_type: InputType, input: u32, output: u32| {
            let kind = match input_type {
                InputType::Axis => "axis",
                InputType::Hat => "hat",
                _ => "button",
            };
            let root = p.bind(device, kind, &input.to_string(), mode);
            let action = Action::new(&NewAction::MapToVjoy {
                input_type,
                device: vjoy,
                input_id: output,
            });
            p.add_child(&root, "actions", action);
        };
        let free = to.without(used);
        let (first_button, first_hat) = (used.buttons, used.hats);
        let missing = from.take(to, used);
        for (axis, target) in axis_targets(&from.axes, &free.axes) {
            if let Some(target) = target {
                map(self, InputType::Axis, axis.into(), target.into());
            }
        }
        for n in 1..=from.buttons - missing.buttons {
            map(self, InputType::Button, n, first_button + n);
        }
        for n in 1..=from.hats - missing.hats {
            map(self, InputType::Hat, n, first_hat + n);
        }
        missing
    }
}

#[cfg(test)]
mod tests;
