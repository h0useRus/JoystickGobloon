//! The virtual keyboard and mouse of keyboard and mouse output (gob-engine's injector), made by
//! the program itself or kept by SystemGobloon and lent to it.

use std::io;

use crate::{Spec, VirtualDevice, codes, keys};

pub const KEYBOARD_NAME: &str = "Joystick Gobloon Keyboard";
pub const MOUSE_NAME: &str = "Joystick Gobloon Mouse";
const KEYBOARD_PHYS: &str = "joystick-gobloon/keyboard";
const MOUSE_PHYS: &str = "joystick-gobloon/mouse";
pub const MOUSE_BUTTONS: [u16; 5] = [
    codes::BTN_LEFT,
    codes::BTN_RIGHT,
    codes::BTN_MIDDLE,
    codes::BTN_SIDE,
    codes::BTN_EXTRA,
];

/// A keyboard with every key Gremlin's scan codes map to.
pub fn keyboard() -> io::Result<VirtualDevice> {
    let keys: Vec<u16> = keys::all_codes().collect();
    VirtualDevice::create(&Spec {
        name: KEYBOARD_NAME,
        phys: KEYBOARD_PHYS,
        bus: codes::BUS_VIRTUAL,
        keys: &keys,
        ..Spec::default()
    })
}

/// A mouse with five buttons and both wheels.
pub fn mouse() -> io::Result<VirtualDevice> {
    VirtualDevice::create(&Spec {
        name: MOUSE_NAME,
        phys: MOUSE_PHYS,
        bus: codes::BUS_VIRTUAL,
        keys: &MOUSE_BUTTONS,
        rel: &[
            codes::REL_X,
            codes::REL_Y,
            codes::REL_WHEEL,
            codes::REL_HWHEEL,
        ],
        ..Spec::default()
    })
}

/// The events that release every key of the keyboard.
pub fn keyboard_release() -> Vec<(u16, u16, i32)> {
    keys::all_codes().map(|c| (codes::EV_KEY, c, 0)).collect()
}

/// The events that release every mouse button.
pub fn mouse_release() -> Vec<(u16, u16, i32)> {
    MOUSE_BUTTONS
        .iter()
        .map(|&c| (codes::EV_KEY, c, 0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ours_by_phys() {
        assert!(KEYBOARD_PHYS.starts_with(crate::OWN_PHYS));
        assert!(MOUSE_PHYS.starts_with(crate::OWN_PHYS));
        assert_eq!(keyboard_release().len(), keys::all_codes().count());
    }
}
