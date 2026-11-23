//! Virtual devices a profile stores: which ones it writes to, with their layout, taken when it
//! is saved, and what this system lacks of them when it is opened. Setting them up only adds
//! devices or grows them; nothing is deleted or made smaller.

use std::collections::HashMap;

use gob_core::{Profile, VirtualDevices, VirtualJoystick};
use gob_virtual::{Capabilities, Change, DeviceConfig, DeviceInfo, GamepadInfo};

/// What `profile` stores when saved: the joysticks it writes to with their layout, name and
/// product id here (as stored before when this system lacks one), and the gamepads it writes to;
/// never less than the outputs it writes need ([`wanted`]).
pub fn capture(
    profile: &Profile,
    devices: &[DeviceInfo],
    configs: &[DeviceConfig],
) -> VirtualDevices {
    let engine = gob_engine::Engine::new(profile, &HashMap::new());
    let joysticks = engine
        .vjoy_devices()
        .into_iter()
        .filter_map(|id| match devices.iter().find(|d| d.id == id) {
            Some(d) => {
                let config = configs.iter().find(|c| c.id == id);
                Some(VirtualJoystick {
                    id,
                    axes: d.axes.clone(),
                    buttons: d.buttons,
                    hats: d.hats(),
                    discrete_hats: d.disc_hats > 0,
                    name: config.and_then(|c| c.name.clone()),
                    product: config.and_then(|c| c.product),
                })
            }
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
    .merged(&profile.needed_virtual_devices())
}

/// The virtual devices `profile` needs: those it stores, grown to what the outputs it writes
/// need (a profile from Joystick Gremlin, or saved where a device was missing, stores none).
pub fn wanted(profile: &Profile) -> VirtualDevices {
    profile
        .virtual_devices
        .merged(&profile.needed_virtual_devices())
}

/// The changes that give this system what `stored` needs: missing devices are created (with
/// the stored name and product id where the system has them), smaller ones get the missing
/// axes, buttons and hats. `configs` keep the rest of an existing device's setup (force
/// feedback, name).
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
                name: j
                    .name
                    .clone()
                    .filter(|n| caps.custom_names && gob_virtual::name_allowed(n)),
                product: j
                    .product
                    .filter(|&p| caps.custom_names && gob_virtual::product_allowed(p)),
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
mod tests;
