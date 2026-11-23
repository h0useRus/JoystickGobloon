use super::*;

fn red_green(led: u8) -> LedConfig {
    LedConfig {
        led,
        led_mode: LedMode::Constant,
        color1: Color::from_rgb(255, 0, 0),
        color2: Color::from_rgb(0, 255, 0),
        ..Default::default()
    }
}

#[test]
fn config_bytes_match_pyvkb() {
    // pyvkb docstring: LEDConfig(led=1, led_mode=CONSTANT) with its old default
    // colors (#f00 / #0f0) -> b'\x01\x07p\x04'.
    let c = red_green(1);
    assert_eq!(c.to_bytes(), [0x01, 0x07, 0x70, 0x04]);
    assert_eq!(LedConfig::from_bytes(c.to_bytes()), c);
}

#[test]
fn config_round_trips_every_field() {
    let c = LedConfig {
        led: 10,
        color_mode: ColorMode::Color1Plus2,
        led_mode: LedMode::UltraBlink,
        color1: Color::new(1, 2, 3),
        color2: Color::new(4, 5, 6),
    };
    assert_eq!(LedConfig::from_bytes(c.to_bytes()), c);
}

#[test]
fn colors_scale_to_eight_levels() {
    assert_eq!(Color::from_rgb(255, 51, 0), Color::new(7, 1, 0));
    assert_eq!(Color::from_rgb(0x80, 0x80, 0x80), Color::new(4, 4, 4));
}

#[test]
fn checksum_is_modbus_crc() {
    // CRC-16/MODBUS check value.
    assert_eq!(checksum(2, b"123456789"), 0x4B37);
}

#[test]
fn encode_layout() {
    let r = encode(&[red_green(1)], [0x12, 0x34]).unwrap();
    assert_eq!(r[..3], [0x59, 0xA5, 0x0A]);
    assert_eq!(r[5..8], [0x12, 0x34, 2]);
    assert_eq!(r[8..12], [0x01, 0x07, 0x70, 0x04]);
    assert_eq!(r[12..16], [DUMMY_LED, 0, 0, 0]);
    assert!(r[16..].iter().all(|&b| b == 0));
    let crc = checksum(2, &r[5..]).to_le_bytes();
    assert_eq!(r[3..5], crc);
    assert!(encode(&[red_green(1); MAX_CONFIGS], [0, 0]).is_none());
}

#[test]
fn empty_list_has_no_dummy() {
    let r = encode(&[], [0x12, 0x34]).unwrap();
    assert_eq!(r[5..8], [0x12, 0x34, 0]);
    assert_eq!(r[3..5], checksum(0, &r[5..]).to_le_bytes());
    assert!(r[8..].iter().all(|&b| b == 0));
    assert!(decode(&r).is_empty());
}

#[test]
fn decode_round_trip_and_unset() {
    let list = [red_green(0), red_green(10)];
    let r = encode(&list, random_bytes()).unwrap();
    assert_eq!(decode(&r), list);
    assert!(decode(&[0x59; REPORT_LEN]).is_empty());
    assert!(decode(&[0u8; 4]).is_empty());
}

#[test]
fn led_maps_to_color1() {
    let c = LedConfig::from(Led {
        id: 10,
        color: [0, 255, 0],
        mode: Mode::FastestBlink,
        ..Default::default()
    });
    assert_eq!(c.led, 10);
    assert_eq!(c.led_mode, LedMode::UltraBlink);
    assert_eq!(c.color_mode, ColorMode::Color1);
    assert_eq!(c.color1, Color::new(0, 7, 0));
    assert_eq!(c.color2, Color::default());
    let c = LedConfig::from(Led {
        color: [255, 0, 0],
        color2: [255, 0, 0],
        blend: Blend::Alternate,
        ..Default::default()
    });
    assert_eq!(c.color_mode, ColorMode::Color1Then2);
    assert_eq!(c.color2, Color::new(7, 0, 0));
}

#[test]
fn base_elements_are_set_one_by_one() {
    let blue = Led {
        id: 0,
        element: 1,
        color: [255, 0, 0],
        mode: Mode::On,
        ..Default::default()
    };
    let list = apply(&[], &blue);
    assert_eq!(list[0].color_mode, ColorMode::Color1);
    assert_eq!(list[0].color1, Color::new(7, 0, 0));
    assert_eq!(list[0].led_mode, LedMode::Constant);
    let red = Led {
        element: 2,
        mode: Mode::SlowBlink,
        ..blue
    };
    let list = apply(&list, &red);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].color_mode, ColorMode::Color1Plus2);
    assert_eq!((list[0].color1.r, list[0].color2.r), (7, 7));
    assert_eq!(list[0].led_mode, LedMode::SlowBlink);
    // Blue off: red keeps blinking.
    let list = apply(
        &list,
        &Led {
            mode: Mode::Off,
            ..blue
        },
    );
    assert_eq!(list[0].color_mode, ColorMode::Color2);
    assert_eq!(list[0].led_mode, LedMode::SlowBlink);
    let list = apply(
        &list,
        &Led {
            mode: Mode::Off,
            ..red
        },
    );
    assert_eq!(list[0].led_mode, LedMode::Off);
}

#[test]
fn merge_and_remove() {
    let a = red_green(0);
    let mut b = red_green(10);
    let list = merge(&[a, b], {
        b.led_mode = LedMode::SlowBlink;
        b
    });
    assert_eq!(list, [a, b]);
    assert_eq!(merge(&list, red_green(11)).len(), 3);
    assert_eq!(remove(&list, 0), [b]);
}
