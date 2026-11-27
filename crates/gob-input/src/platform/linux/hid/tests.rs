use super::*;

/// A joystick with 128 buttons in report 1 after two 16-bit axes, and a constant pad.
const STICK: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x04, // Usage (Joystick)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x01, //   Report ID (1)
    0x09, 0x30, //   Usage (X)
    0x09, 0x31, //   Usage (Y)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xFF, 0x7F, // Logical Maximum (32767)
    0x75, 0x10, //   Report Size (16)
    0x95, 0x02, //   Report Count (2)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    0x05, 0x09, //   Usage Page (Button)
    0x19, 0x01, //   Usage Minimum (1)
    0x29, 0x80, //   Usage Maximum (128)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x96, 0x80, 0x00, // Report Count (128)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x03, //   Input (Constant)
    0xC0, // End Collection
];

#[test]
fn buttons_follow_the_axes() {
    let layout = buttons(STICK);
    assert!(layout.has_ids);
    // Report 1: 4 bytes of axes, 16 of buttons, 1 constant.
    assert_eq!(layout.reports, vec![(1, 21)]);
    let buttons = layout.buttons;
    assert_eq!(buttons.len(), 128);
    assert_eq!(
        buttons[0],
        Button {
            report: 1,
            bit: 32,
            usage: 1
        }
    );
    assert_eq!(buttons[127].usage, 128);
    assert_eq!(buttons[127].bit, 32 + 127);
}

#[test]
fn reports_set_buttons_by_usage() {
    let layout = buttons(STICK);
    // Id 1, axes (4 bytes), buttons: 1 and 81 and 128 pressed.
    let mut report = vec![1u8, 0, 0, 0, 0];
    let mut bits = [0u8; 16];
    for b in [1u32, 81, 128] {
        bits[((b - 1) / 8) as usize] |= 1 << ((b - 1) % 8);
    }
    report.extend_from_slice(&bits);
    let mut pressed = Vec::new();
    apply(&layout, &report, &mut |b, on| {
        if on {
            pressed.push(b)
        }
    });
    assert_eq!(pressed, vec![1, 81, 128]);
    // Another report id leaves them alone.
    let mut calls = 0;
    apply(&layout, &[2, 0xFF, 0xFF], &mut |_, _| calls += 1);
    assert_eq!(calls, 0);
}

#[test]
fn without_report_ids_and_with_a_usage_list() {
    let desc = [
        0x05, 0x09, // Usage Page (Button)
        0x09, 0x02, // Usage (2)
        0x09, 0x01, // Usage (1)
        0x75, 0x01, // Report Size (1)
        0x95, 0x02, // Report Count (2)
        0x81, 0x02, // Input (Data, Variable)
        0x95, 0x06, // Report Count (6)
        0x81, 0x01, // Input (Constant)
    ];
    let layout = buttons(&desc);
    assert!(!layout.has_ids);
    assert_eq!(layout.reports, vec![(0, 1)]);
    let buttons = layout.buttons;
    assert_eq!(buttons.len(), 2);
    assert_eq!((buttons[0].usage, buttons[0].bit), (2, 0));
    assert_eq!((buttons[1].usage, buttons[1].bit), (1, 1));
}

#[test]
fn push_pop_and_other_pages_and_arrays_are_skipped() {
    let desc = [
        0x05, 0x09, // Usage Page (Button)
        0xA4, // Push
        0x05, 0x07, // Usage Page (Keyboard)
        0x19, 0x04, 0x29, 0x05, // Usage 4..5
        0x75, 0x01, 0x95, 0x02, 0x81, 0x02, // two keyboard bits
        0xB4, // Pop: Button page again
        0x19, 0x01, 0x29, 0x08, // Usage 1..8
        0x75, 0x08, 0x95, 0x01, 0x81, 0x00, // an array (button index), skipped
        0x19, 0x01, 0x29, 0x02, // Usage 1..2
        0x75, 0x01, 0x95, 0x02, 0x81, 0x02, // two button bits
    ];
    assert_eq!(
        buttons(&desc).buttons,
        vec![
            Button {
                report: 0,
                bit: 10,
                usage: 1
            },
            Button {
                report: 0,
                bit: 11,
                usage: 2
            },
        ]
    );
}

#[test]
fn truncated_descriptors_do_not_panic() {
    for n in 0..STICK.len() {
        let _ = buttons(&STICK[..n]);
    }
    let _ = buttons(&[0xFE, 0x10]);
}
