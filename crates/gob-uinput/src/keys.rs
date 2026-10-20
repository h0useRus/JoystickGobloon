//! evdev key codes <-> set 1 scan codes (with the E0 prefix flag), the codes Gremlin profiles
//! and the Windows keyboard hook use. Codes 1..=88 are the scan codes themselves (the kernel's
//! table started as set 1), except NumLock, which Windows reports with E0 to tell it from Pause.

/// (evdev code, scan code, E0) for every key outside the 1..=88 identity range, and NumLock.
const TABLE: &[(u16, u16, bool)] = &[
    (69, 0x45, true),   // KEY_NUMLOCK
    (89, 0x73, false),  // KEY_RO
    (92, 0x79, false),  // KEY_HENKAN
    (93, 0x70, false),  // KEY_KATAKANAHIRAGANA
    (94, 0x7b, false),  // KEY_MUHENKAN
    (96, 0x1c, true),   // KEY_KPENTER
    (97, 0x1d, true),   // KEY_RIGHTCTRL
    (98, 0x35, true),   // KEY_KPSLASH
    (99, 0x37, true),   // KEY_SYSRQ (Print Screen)
    (100, 0x38, true),  // KEY_RIGHTALT
    (102, 0x47, true),  // KEY_HOME
    (103, 0x48, true),  // KEY_UP
    (104, 0x49, true),  // KEY_PAGEUP
    (105, 0x4b, true),  // KEY_LEFT
    (106, 0x4d, true),  // KEY_RIGHT
    (107, 0x4f, true),  // KEY_END
    (108, 0x50, true),  // KEY_DOWN
    (109, 0x51, true),  // KEY_PAGEDOWN
    (110, 0x52, true),  // KEY_INSERT
    (111, 0x53, true),  // KEY_DELETE
    (113, 0x20, true),  // KEY_MUTE
    (114, 0x2e, true),  // KEY_VOLUMEDOWN
    (115, 0x30, true),  // KEY_VOLUMEUP
    (117, 0x59, false), // KEY_KPEQUAL
    (119, 0x45, false), // KEY_PAUSE
    (121, 0x7e, false), // KEY_KPCOMMA
    (122, 0x72, false), // KEY_HANGEUL
    (123, 0x71, false), // KEY_HANJA
    (124, 0x7d, false), // KEY_YEN
    (125, 0x5b, true),  // KEY_LEFTMETA
    (126, 0x5c, true),  // KEY_RIGHTMETA
    (127, 0x5d, true),  // KEY_COMPOSE (Menu)
    (163, 0x19, true),  // KEY_NEXTSONG
    (164, 0x22, true),  // KEY_PLAYPAUSE
    (165, 0x10, true),  // KEY_PREVIOUSSONG
    (166, 0x24, true),  // KEY_STOPCD
    (183, 0x64, false), // KEY_F13
    (184, 0x65, false),
    (185, 0x66, false),
    (186, 0x67, false),
    (187, 0x68, false),
    (188, 0x69, false),
    (189, 0x6a, false),
    (190, 0x6b, false),
    (191, 0x6c, false),
    (192, 0x6d, false),
    (193, 0x6e, false), // KEY_F23
    (194, 0x76, false), // KEY_F24
];

/// evdev key code -> (scan code, E0).
pub fn scan_code(code: u16) -> Option<(u16, bool)> {
    if let Some(&(_, scan, extended)) = TABLE.iter().find(|(c, _, _)| *c == code) {
        return Some((scan, extended));
    }
    (1..=88).contains(&code).then_some((code, false))
}

/// (scan code, E0) -> evdev key code.
pub fn key_code(scan_code: u16, extended: bool) -> Option<u16> {
    if let Some(&(code, _, _)) = TABLE
        .iter()
        .find(|(_, s, e)| *s == scan_code && *e == extended)
    {
        return Some(code);
    }
    // Right Shift sent with E0 (Gremlin's "rightshift2") is still Right Shift.
    let plain = (1..=88).contains(&scan_code) && scan_code != 69;
    plain
        .then_some(scan_code)
        .filter(|_| !extended || scan_code == 0x36)
}

/// Every evdev key code with a scan code, for a virtual keyboard's key list.
pub fn all_codes() -> impl Iterator<Item = u16> {
    (1..=88).chain(TABLE.iter().map(|&(c, _, _)| c).filter(|&c| c > 88))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_windows_and_gremlin() {
        assert_eq!(scan_code(1), Some((0x01, false)), "Esc");
        assert_eq!(scan_code(30), Some((0x1e, false)), "A");
        assert_eq!(scan_code(88), Some((0x58, false)), "F12");
        assert_eq!(scan_code(69), Some((0x45, true)), "NumLock");
        assert_eq!(scan_code(119), Some((0x45, false)), "Pause");
        assert_eq!(scan_code(97), Some((0x1d, true)), "right Ctrl");
        assert_eq!(scan_code(194), Some((0x76, false)), "F24");
        assert_eq!(scan_code(0x120), None, "joystick button");
        assert_eq!(key_code(0x1d, false), Some(29), "left Ctrl");
        assert_eq!(key_code(0x1d, true), Some(97), "right Ctrl");
        assert_eq!(key_code(0x45, true), Some(69));
        assert_eq!(key_code(0x45, false), Some(119));
        assert_eq!(key_code(0x36, true), Some(0x36), "rightshift2");
        assert_eq!(key_code(0x1e, true), None, "no E0 A");
    }

    #[test]
    fn round_trip() {
        for code in all_codes() {
            let (scan, extended) = scan_code(code).unwrap();
            assert_eq!(key_code(scan, extended), Some(code), "{code}");
        }
        assert_eq!(all_codes().count(), 88 + TABLE.len() - 1);
    }
}
