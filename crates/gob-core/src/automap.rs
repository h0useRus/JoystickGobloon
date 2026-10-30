//! Automapping (the new profile wizard): every input of a physical device gets a Map to vJoy
//! onto one virtual joystick.

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
    /// vJoy device `vjoy` (`to`): axes as [`axis_targets`] says, buttons and hats by number.
    /// Inputs `to` has no room for stay unbound. The device joins the profile's device list.
    pub fn automap(
        &mut self,
        device: &str,
        name: &str,
        from: &Layout,
        vjoy: u32,
        to: &Layout,
        mode: &str,
    ) {
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
        for (axis, target) in axis_targets(&from.axes, &to.axes) {
            if let Some(target) = target {
                map(self, InputType::Axis, axis.into(), target.into());
            }
        }
        for n in 1..=from.buttons.min(to.buttons) {
            map(self, InputType::Button, n, n);
        }
        for n in 1..=from.hats.min(to.hats) {
            map(self, InputType::Hat, n, n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VirtualOutput;

    #[test]
    fn axes_keep_their_slot_and_fill_the_rest_in_order() {
        // X, Y, Rz, Slider onto a joystick with X, Y, Z, Rx.
        let t = axis_targets(&[1, 2, 6, 7], &[1, 2, 3, 4]);
        assert_eq!(
            t,
            vec![(1, Some(1)), (2, Some(2)), (6, Some(3)), (7, Some(4))]
        );
        // More axes than the joystick has: the last ones stay unmapped.
        let t = axis_targets(&[1, 2, 3], &[1, 2]);
        assert_eq!(t, vec![(1, Some(1)), (2, Some(2)), (3, None)]);
    }

    #[test]
    fn automap_binds_what_fits_and_reports_the_rest() {
        let from = Layout {
            axes: vec![1, 2, 3],
            buttons: 40,
            hats: 2,
        };
        let to = Layout {
            axes: vec![1, 2],
            buttons: 32,
            hats: 4,
        };
        let mut p = Profile::default();
        p.automap("stick", "Stick", &from, 2, &to, "Default");
        assert_eq!(p.devices.len(), 1);
        // 2 axes + 32 buttons + 2 hats.
        assert_eq!(p.inputs.len(), 36);
        let usage = p.output_usage(None);
        let vjoy = |input_type, id| VirtualOutput::Vjoy {
            device: 2,
            input_type,
            id,
        };
        assert!(usage.contains_key(&vjoy(InputType::Button, 32)));
        assert!(!usage.contains_key(&vjoy(InputType::Button, 33)));
        assert!(usage.contains_key(&vjoy(InputType::Hat, 2)));
        assert!(usage.values().all(|u| u.len() == 1), "one input per output");
        assert_eq!(p.main_source(2), Some("Stick"));
        assert_eq!(p.main_source(1), None, "nothing writes to device 1");

        let missing = from.missing_in(&to);
        assert_eq!(
            missing,
            Layout {
                axes: vec![3],
                buttons: 8,
                hats: 0,
            }
        );
        assert!(to.missing_in(&to).is_empty());
    }
}
