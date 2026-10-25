//! An Xbox 360-style gamepad: the codes, ranges and ids the kernel's `xpad` driver gives a wired
//! Xbox 360 controller (045e:028e). SDL finds that controller in its mapping database, so games
//! (and Proton, as XInput) see a gamepad with the usual layout instead of a guessed one.
//!
//! In this code order SDL numbers the buttons b0..b10 (A, B, X, Y, LB, RB, Back, Start, Guide,
//! left stick, right stick) and the axes a0..a5 (left X, left Y, LT, right X, right Y, RT), the
//! d-pad is hat 0. Like `xpad`, the Y axes point down: pushing a stick up gives negative values.

use crate::codes::*;
use crate::{Axis, Spec};

pub const VENDOR: u16 = 0x045e;
pub const PRODUCT: u16 = 0x028e;
/// bcdDevice of the wired controller; SDL's database has a mapping for this version.
pub const VERSION: u16 = 0x0114;

/// Buttons in `xpad`'s code order.
pub const BUTTONS: [u16; 11] = [
    BTN_SOUTH, BTN_EAST, BTN_NORTH, BTN_WEST, BTN_TL, BTN_TR, BTN_SELECT, BTN_START, BTN_MODE,
    BTN_THUMBL, BTN_THUMBR,
];

pub const STICK_MIN: i32 = -32768;
pub const STICK_MAX: i32 = 32767;
pub const TRIGGER_MAX: i32 = 255;

const fn axis(code: u16, min: i32, max: i32) -> Axis {
    Axis { code, min, max }
}

/// Sticks, triggers and the d-pad (as an axis pair, like `xpad`).
pub const AXES: [Axis; 8] = [
    axis(ABS_X, STICK_MIN, STICK_MAX),
    axis(ABS_Y, STICK_MIN, STICK_MAX),
    axis(ABS_Z, 0, TRIGGER_MAX),
    axis(ABS_RX, STICK_MIN, STICK_MAX),
    axis(ABS_RY, STICK_MIN, STICK_MAX),
    axis(ABS_RZ, 0, TRIGGER_MAX),
    axis(ABS_HAT0X, -1, 1),
    axis(ABS_HAT0Y, -1, 1),
];

/// A gamepad named `name` (keep the "Joystick Gobloon" prefix: gob-input skips its own devices
/// by it, since the ids are a real controller's).
pub fn spec(name: &str) -> Spec<'_> {
    Spec {
        name,
        bus: BUS_USB,
        vendor: VENDOR,
        product: PRODUCT,
        version: VERSION,
        keys: &BUTTONS,
        axes: &AXES,
        rel: &[],
    }
}

/// Stick value -1.0..=1.0 -> -32768..=32767.
pub fn stick(value: f32) -> i32 {
    let t = (value.clamp(-1.0, 1.0) + 1.0) / 2.0;
    STICK_MIN + (t * (STICK_MAX - STICK_MIN) as f32).round() as i32
}

/// Trigger value 0.0..=1.0 -> 0..=255.
pub fn trigger(value: f32) -> i32 {
    (value.clamp(0.0, 1.0) * TRIGGER_MAX as f32).round() as i32
}

/// D-pad direction in hundredths of a degree (0 = up, clockwise; `None` centered) -> the
/// `ABS_HAT0X` / `ABS_HAT0Y` values (-1, 0, 1; up is -1 on Y). Diagonals set both.
pub fn dpad(direction: Option<u16>) -> (i32, i32) {
    let Some(d) = direction else {
        return (0, 0);
    };
    // Eight 45 degree sectors centered on up, up-right, right, ...
    let sector = ((u32::from(d % 36000) + 2250) / 4500) % 8;
    const XY: [(i32, i32); 8] = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];
    XY[sector as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_scale_to_xpad_ranges() {
        assert_eq!(stick(-1.0), -32768);
        assert_eq!(stick(1.0), 32767);
        assert!(stick(0.0).abs() <= 1);
        assert_eq!(trigger(0.0), 0);
        assert_eq!(trigger(1.0), 255);
        assert_eq!(trigger(-0.5), 0, "clamped");
    }

    #[test]
    fn dpad_directions() {
        assert_eq!(dpad(None), (0, 0));
        assert_eq!(dpad(Some(0)), (0, -1));
        assert_eq!(dpad(Some(9000)), (1, 0));
        assert_eq!(dpad(Some(13500)), (1, 1));
        assert_eq!(dpad(Some(27000)), (-1, 0));
        assert_eq!(dpad(Some(35000)), (0, -1), "near up");
    }

    #[test]
    fn sdl_numbers_buttons_and_axes_by_code() {
        // SDL's Linux driver numbers buttons and axes in code order: it must match the
        // b0..b10 / a0..a5 of the 045e:028e mapping.
        assert!(BUTTONS.windows(2).all(|w| w[0] < w[1]));
        let sticks_and_triggers: Vec<u16> = AXES[..6].iter().map(|a| a.code).collect();
        assert!(sticks_and_triggers.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(
            sticks_and_triggers,
            [ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ]
        );
    }
}
