use super::*;

const BTN_MISC: usize = 0x100;
const BTN_TRIGGER: usize = 0x120;
const BTN_SOUTH: usize = 0x130;
const BTN_LEFT: usize = 0x110;
const ABS_X: usize = 0;
const ABS_Y: usize = 1;
const ABS_THROTTLE: usize = 0x06;
const ABS_RUDDER: usize = 0x07;
const ABS_WHEEL: usize = 0x08;

fn caps(keys: &[usize], abs: &[usize]) -> Caps {
    let mut events = vec![EV_SYN.into()];
    if !keys.is_empty() {
        events.push(EV_KEY.into());
    }
    if !abs.is_empty() {
        events.push(EV_ABS.into());
    }
    Caps {
        events: Bits::of(&events),
        keys: Bits::of(keys),
        abs: Bits::of(abs),
        props: Bits::default(),
    }
}

#[test]
fn joysticks_and_not() {
    assert!(is_joystick(&caps(&[BTN_TRIGGER], &[ABS_X, ABS_Y])), "stick");
    assert!(is_joystick(&caps(&[BTN_SOUTH], &[ABS_X, ABS_Y])), "gamepad");
    assert!(is_joystick(&caps(&[], &[ABS_X, ABS_Y, 0x05])), "pedals");
    assert!(
        is_joystick(&caps(&[BTN_TRIGGER_HAPPY + 3], &[])),
        "button box"
    );
    assert!(
        !is_joystick(&caps(&[BTN_LEFT], &[ABS_X, ABS_Y])),
        "absolute mouse"
    );
    assert!(
        !is_joystick(&caps(&[BTN_TOUCH], &[ABS_X, ABS_Y])),
        "touch screen"
    );
    assert!(!is_joystick(&caps(&[30, 31, 32], &[])), "keyboard");
    let mut motion = caps(&[], &[ABS_X, ABS_Y, 0x02]);
    motion.props = Bits::of(&[INPUT_PROP_ACCELEROMETER]);
    assert!(!is_joystick(&motion), "motion sensor");
}

#[test]
fn axes_take_directinput_slots() {
    let abs = Bits::of(&[
        ABS_X,
        ABS_Y,
        ABS_RZ,
        ABS_HAT0X,
        ABS_RUDDER,
        ABS_THROTTLE,
        ABS_WHEEL,
    ]);
    assert_eq!(
        axis_slots(&abs),
        vec![
            (ABS_X, 0),
            (ABS_Y, 1),
            (ABS_RZ, 5),
            (ABS_THROTTLE, 6),
            (ABS_RUDDER, 7)
        ]
    );
    assert_eq!(hat_count(&abs), 1);
    assert_eq!(hat_count(&Bits::of(&[ABS_HAT0X + 5])), 3);
}

#[test]
fn buttons_in_sdl_order() {
    let keys = Bits::of(&[BTN_TRIGGER_HAPPY, BTN_MISC, BTN_TRIGGER + 1, BTN_TRIGGER]);
    assert_eq!(
        button_codes(&keys),
        vec![
            BTN_TRIGGER as u16,
            BTN_TRIGGER as u16 + 1,
            BTN_TRIGGER_HAPPY as u16,
            BTN_MISC as u16
        ]
    );
}

#[test]
fn hats_and_axis_scaling() {
    assert_eq!(hat_direction(0, 0), None);
    assert_eq!(hat_direction(0, -1), Some(0));
    assert_eq!(hat_direction(1, 1), Some(13500));
    assert_eq!(hat_direction(-1, -1), Some(31500));
    let hat = Hat {
        center2: [2, 2],
        value: [2, 0],
    };
    assert_eq!(hat.direction(), Some(4500), "0..2 range: 2 = right, 0 = up");
    assert_eq!(scale_axis(0, 0, 65535), -32768);
    assert_eq!(scale_axis(65535, 0, 65535), 32767);
    assert_eq!(scale_axis(-1, -1, 1), -32768);
    assert_eq!(scale_axis(1, -1, 1), 32767);
    assert_eq!(scale_axis(5, 3, 3), 0);
}

#[test]
fn ids_are_stable_and_distinct() {
    let a = device_guid(
        0x231d,
        0x0200,
        3,
        &identity("", "usb-0000:00:14.0-2/input0", "VKB"),
    );
    assert_eq!(
        a,
        device_guid(
            0x231d,
            0x0200,
            3,
            &identity("", "usb-0000:00:14.0-2/input0", "VKB")
        )
    );
    assert!(a.starts_with("0200231d-0003-0000-"), "{a}");
    assert_eq!(a.len(), 36);
    let b = device_guid(
        0x231d,
        0x0200,
        3,
        &identity("", "usb-0000:00:14.0-3/input0", "VKB"),
    );
    assert_ne!(a, b);
    let serial = identity("ABC123", "anywhere", "VKB");
    assert_eq!(serial, identity("ABC123", "elsewhere", "VKB"));
    assert_eq!(
        identity("", "joystick-gobloon/joystick-2", "JoyGob 2"),
        identity("", "joystick-gobloon/joystick-2", "Gobloon Throttle"),
        "our devices keep their id when renamed"
    );
}
