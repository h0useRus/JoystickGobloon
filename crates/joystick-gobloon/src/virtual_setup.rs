//! Virtual devices a profile stores: which ones it writes to, with their layout, taken when it
//! is saved, and what this system lacks of them when it is opened. Setting them up only adds
//! devices or grows them; nothing is deleted or made smaller.

use std::collections::HashMap;

use gob_core::{Profile, VirtualDevices, VirtualJoystick};
use gob_virtual::{Capabilities, Change, DeviceConfig, DeviceInfo, GamepadInfo};

/// What `profile` stores when saved: the joysticks it writes to with their layout here (as
/// stored before when this system lacks one), and the gamepads it writes to.
pub fn capture(profile: &Profile, devices: &[DeviceInfo]) -> VirtualDevices {
    let engine = gob_engine::Engine::new(profile, &HashMap::new());
    let joysticks = engine
        .vjoy_devices()
        .into_iter()
        .filter_map(|id| match devices.iter().find(|d| d.id == id) {
            Some(d) => Some(VirtualJoystick {
                id,
                axes: d.axes.clone(),
                buttons: d.buttons,
                hats: d.hats(),
                discrete_hats: d.disc_hats > 0,
            }),
            None => profile
                .virtual_devices
                .joysticks
                .iter()
                .find(|j| j.id == id)
                .cloned(),
        })
        .collect();
    VirtualDevices {
        joysticks,
        gamepads: engine.gamepads().into_iter().collect(),
    }
}

/// The changes that give this system what `stored` needs: missing devices are created,
/// smaller ones get the missing axes, buttons and hats. `configs` keep the rest of an
/// existing device's setup (force feedback).
pub fn changes(
    stored: &VirtualDevices,
    devices: &[DeviceInfo],
    configs: &[DeviceConfig],
    gamepads: &[GamepadInfo],
    caps: &Capabilities,
) -> Vec<Change> {
    let mut out = Vec::new();
    for j in stored.joysticks.iter().filter(|j| j.id <= caps.max_devices) {
        let want_hats = j.hats.min(caps.max_hats);
        let want_buttons = j.buttons.min(caps.max_buttons);
        let mut config = match devices.iter().find(|d| d.id == j.id) {
            Some(d) => {
                let enough = j.axes.iter().all(|a| d.axes.contains(a))
                    && d.buttons >= want_buttons
                    && d.hats() >= want_hats
                    && (want_hats == 0 || (d.disc_hats > 0) == j.discrete_hats);
                if enough {
                    continue;
                }
                configs
                    .iter()
                    .find(|c| c.id == j.id)
                    .cloned()
                    .unwrap_or_else(|| from_info(d, caps))
            }
            None => DeviceConfig {
                axes: [false; gob_virtual::AXIS_SLOTS],
                buttons: 0,
                hats: 0,
                ..DeviceConfig::new(j.id, caps)
            },
        };
        for &a in &j.axes {
            if let Some(slot) = config.axes.get_mut(usize::from(a).wrapping_sub(1)) {
                *slot = true;
            }
        }
        config.buttons = config.buttons.max(want_buttons);
        config.hats = config.hats.max(want_hats);
        if want_hats > 0 {
            config.discrete_hats = j.discrete_hats;
        }
        out.push(Change::Set(config));
    }
    if caps.max_gamepads > 0 {
        for &id in &stored.gamepads {
            if id <= caps.max_gamepads && !gamepads.iter().any(|g| g.id == id) {
                out.push(Change::AddGamepad(id));
            }
        }
    }
    out
}

/// A device's setup as the driver reports it, without force feedback.
fn from_info(d: &DeviceInfo, caps: &Capabilities) -> DeviceConfig {
    let mut axes = [false; gob_virtual::AXIS_SLOTS];
    for &a in &d.axes {
        if let Some(slot) = axes.get_mut(usize::from(a).wrapping_sub(1)) {
            *slot = true;
        }
    }
    DeviceConfig {
        axes,
        buttons: d.buttons,
        hats: d.hats(),
        discrete_hats: d.disc_hats > 0,
        ..DeviceConfig::new(d.id, caps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gob_virtual::Status;

    fn caps() -> Capabilities {
        Capabilities {
            max_devices: 16,
            max_gamepads: 4,
            max_buttons: 128,
            max_hats: 4,
            ..gob_virtual::NO_CAPABILITIES
        }
    }

    fn info(id: u32, axes: &[u8], buttons: u32, hats: u32) -> DeviceInfo {
        DeviceInfo {
            id,
            status: Status::Free,
            axes: axes.to_vec(),
            buttons,
            cont_hats: hats,
            disc_hats: 0,
        }
    }

    fn joystick(id: u32, axes: &[u8], buttons: u32, hats: u32) -> VirtualJoystick {
        VirtualJoystick {
            id,
            axes: axes.to_vec(),
            buttons,
            hats,
            discrete_hats: false,
        }
    }

    #[test]
    fn missing_and_smaller_devices_are_set_up_without_shrinking() {
        let stored = VirtualDevices {
            joysticks: vec![
                joystick(1, &[1, 2], 8, 1),
                joystick(2, &[1, 7], 32, 0),
                joystick(3, &[1], 4, 0),
            ],
            gamepads: vec![1, 2],
        };
        let devices = [info(1, &[1, 2, 3], 16, 1), info(2, &[1, 2], 64, 2)];
        let pads = [GamepadInfo {
            id: 1,
            status: Status::Free,
        }];
        let changes = changes(&stored, &devices, &[], &pads, &caps());
        // 1 has enough; 2 gets its slider and keeps what it had; 3 is new; gamepad 2 is added.
        assert_eq!(changes.len(), 3, "{changes:?}");
        let Change::Set(two) = &changes[0] else {
            panic!("{changes:?}")
        };
        assert_eq!(two.id, 2);
        assert_eq!(
            two.axes,
            [true, true, false, false, false, false, true, false]
        );
        assert_eq!((two.buttons, two.hats), (64, 2));
        let Change::Set(three) = &changes[1] else {
            panic!("{changes:?}")
        };
        assert_eq!((three.id, three.buttons, three.hats), (3, 4, 0));
        assert_eq!(three.axes.iter().filter(|a| **a).count(), 1);
        assert_eq!(changes[2], Change::AddGamepad(2));

        // Without gamepads on this system they are not asked for.
        let no_pads = Capabilities {
            max_gamepads: 0,
            ..caps()
        };
        assert_eq!(changes_len(&stored, &devices, &no_pads), 2);
    }

    fn changes_len(stored: &VirtualDevices, devices: &[DeviceInfo], caps: &Capabilities) -> usize {
        changes(stored, devices, &[], &[], caps).len()
    }

    #[test]
    fn capture_takes_the_used_devices() {
        // Nothing writes to a device: nothing to store.
        assert!(capture(&Profile::default(), &[info(1, &[1], 8, 0)]).is_empty());

        // The sample maps to vJoy device 1; device 2 is not used.
        let sample = include_str!("../../../tests/fixtures/sample_r14.xml");
        let mut profile = gob_gremlin::from_str(sample).unwrap();
        let devices = [info(1, &[1, 2], 32, 1), info(2, &[1], 8, 0)];
        let taken = capture(&profile, &devices);
        assert_eq!(taken.joysticks, vec![joystick(1, &[1, 2], 32, 1)]);

        // Saved again where device 1 is missing: the stored layout stays.
        profile.set_virtual_devices(taken.clone());
        assert_eq!(capture(&profile, &devices[1..]), taken);
    }
}
