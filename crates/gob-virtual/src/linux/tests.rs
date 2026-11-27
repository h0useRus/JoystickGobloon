use super::*;

#[test]
fn gamepad_read_back_as_input_device() {
    use GamepadControl as C;
    assert_eq!(gamepad_inputs("JoyGob 1"), None);
    assert_eq!(gamepad_inputs("Some Pad"), None);
    let pad = gamepad_inputs("GameGob 2").unwrap();
    assert_eq!(
        pad.axes,
        [
            (1, C::LeftX),
            (2, C::LeftY),
            (3, C::Lt),
            (4, C::RightX),
            (5, C::RightY),
            (6, C::Rt)
        ]
    );
    let buttons: Vec<C> = pad.buttons.iter().map(|&(_, c)| c).collect();
    assert_eq!(
        buttons,
        [
            C::A,
            C::B,
            C::X,
            C::Y,
            C::Lb,
            C::Rb,
            C::Back,
            C::Start,
            C::Guide,
            C::LeftStick,
            C::RightStick
        ]
    );
    assert_eq!(pad.buttons[0].0, 1);
    assert_eq!(pad.dpad, Some(1));
}

#[test]
fn buttons_hats_and_axes() {
    assert_eq!(button_code(1), Some(0x120));
    assert_eq!(button_code(16), Some(0x12f));
    assert_eq!(button_code(17), Some(0x2c0));
    assert_eq!(button_code(79), Some(0x2fe));
    assert_eq!(button_code(80), Some(0x100));
    assert_eq!(button_code(95), Some(0x10f));
    assert_eq!(button_code(96), None);
    let codes: Vec<u16> = (1..=95).filter_map(button_code).collect();
    assert!(
        codes
            .iter()
            .all(|c| !(0x130..0x140).contains(c) && *c != 0x2ff),
        "no gamepad codes, no KEY_MAX"
    );
    // SDL's order: BTN_JOYSTICK up to KEY_MAX, then everything below BTN_JOYSTICK.
    let mut sdl = codes.clone();
    sdl.sort_by_key(|&c| (c < 0x120, c));
    assert_eq!(sdl, codes, "button numbers follow SDL's order");
    assert_eq!(hat_xy(None, false), (0, 0));
    assert_eq!(hat_xy(Some(0), false), (0, -1));
    assert_eq!(hat_xy(Some(4500), false), (1, -1));
    assert_eq!(hat_xy(Some(35000), false), (0, -1), "nearest sector");
    assert_eq!(
        hat_xy(Some(4500), true),
        (1, 0),
        "discrete: right wins at 45°"
    );
    assert_eq!(hat_xy(Some(27000), true), (-1, 0));
    assert_eq!(axis_value(-1.0), AXIS_MIN);
    assert_eq!(axis_value(1.0), AXIS_MAX);
    assert_eq!(axis_value(0.0), 0);
}

#[test]
fn config_file_round_trip() {
    let default = parse(None).unwrap();
    assert_eq!(default.devices, vec![DeviceConfig::new(1, &CAPABILITIES)]);
    assert!(default.gamepads.is_empty(), "no gamepad until one is added");
    let joystick = &default.devices[0];
    assert_eq!(
        (joystick.buttons, joystick.hats),
        (95, 4),
        "the whole layout"
    );
    assert_eq!(joystick.axes, [true; AXIS_SLOTS]);
    let mut c = DeviceConfig::new(3, &CAPABILITIES);
    c.axes[2] = false;
    c.discrete_hats = true;
    c.name = Some("Gobloon Throttle".into());
    c.product = Some(0xBE42);
    let layout = changed(default, &Change::Set(c.clone())).unwrap();
    let layout = changed(layout, &Change::AddGamepad(2)).unwrap();
    let text = to_text(&layout).unwrap();
    assert!(text.contains("[[device]]"), "{text}");
    assert!(text.contains("name = \"Gobloon Throttle\""), "{text}");
    assert!(
        !text.contains("name = \"Joystick"),
        "default names are not stored"
    );
    assert!(text.contains("[[gamepad]]\nid = 2"), "{text}");
    assert_eq!(parse(Some(&text)).unwrap(), layout);
    let layout = changed(layout, &Change::Delete(1)).unwrap();
    assert_eq!(layout.devices, vec![c]);
    assert!(changed(layout.clone(), &Change::Enable(false)).is_err());
    assert!(
        changed(layout.clone(), &Change::AddGamepad(5)).is_err(),
        "4 at most"
    );
    let layout = changed(layout, &Change::DeleteGamepad(2)).unwrap();
    assert!(layout.gamepads.is_empty());
    assert!(!to_text(&layout).unwrap().contains("gamepad"));
    let bad = parse(Some(
        "[[device]]
id = 1
axes = []
buttons = 1
hats = 0
name = \"\"
product = 48813
",
    ))
    .unwrap();
    assert_eq!(
        (&bad.devices[0].name, bad.devices[0].product),
        (&None, None),
        "an empty name and vJoy's product id are dropped"
    );
    let empty = parse(Some("")).unwrap();
    assert!(
        empty.devices.is_empty() && empty.gamepads.is_empty(),
        "no devices"
    );
}

#[test]
fn gamepad_controls_and_dpad() {
    let buttons: Vec<u16> = GamepadControl::ALL
        .into_iter()
        .filter(|c| c.kind() == ControlKind::Button)
        .filter_map(pad_code)
        .collect();
    assert_eq!(
        buttons.len(),
        gamepad::BUTTONS.len(),
        "every button has a code"
    );
    assert!(buttons.iter().all(|b| gamepad::BUTTONS.contains(b)));
    for c in GamepadControl::ALL {
        let axis = matches!(c.kind(), ControlKind::Stick | ControlKind::Trigger);
        if axis {
            assert!(pad_code(c).is_some(), "{c:?}");
        }
    }
    assert_eq!(dpad_xy(0), (0, 0));
    assert_eq!(dpad_xy(UP | RIGHT), (1, -1));
    assert_eq!(dpad_xy(UP | DOWN), (0, 0), "opposites cancel");
    for xy in [(0, 0), (1, -1), (-1, 0), (0, 1)] {
        assert_eq!(dpad_xy(dpad_bits(xy)), xy);
    }
}
