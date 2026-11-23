//! Keyboard state and key names on Linux.
//!
//! Key state comes from the evdev keyboards (`EVIOCGKEY`) when the user may read them (the
//! `input` group); otherwise there is none and key recording uses the window's key events.
//! Names are English, by set 1 scan code (the US layout's key caps).

use gob_core::Key;

/// Keys held on any keyboard; `None` without access to the keyboards.
pub(super) fn held() -> Option<Vec<Key>> {
    let keys = gob_input::held_keys()?;
    Some(
        keys.into_iter()
            .map(|(scan_code, extended)| Key {
                scan_code,
                extended,
            })
            .collect(),
    )
}

/// English name, e.g. "Right Ctrl"; `None` for keys without one here.
pub(super) fn name(key: Key) -> Option<String> {
    const ROW: [&str; 0x59] = [
        "",
        "Esc",
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "9",
        "0",
        "-",
        "=",
        "Backspace",
        "Tab",
        "Q",
        "W",
        "E",
        "R",
        "T",
        "Y",
        "U",
        "I",
        "O",
        "P",
        "[",
        "]",
        "Enter",
        "Left Ctrl",
        "A",
        "S",
        "D",
        "F",
        "G",
        "H",
        "J",
        "K",
        "L",
        ";",
        "'",
        "`",
        "Left Shift",
        "\\",
        "Z",
        "X",
        "C",
        "V",
        "B",
        "N",
        "M",
        ",",
        ".",
        "/",
        "Right Shift",
        "Numpad *",
        "Left Alt",
        "Space",
        "Caps Lock",
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
        "F7",
        "F8",
        "F9",
        "F10",
        "Pause",
        "Scroll Lock",
        "Numpad 7",
        "Numpad 8",
        "Numpad 9",
        "Numpad -",
        "Numpad 4",
        "Numpad 5",
        "Numpad 6",
        "Numpad +",
        "Numpad 1",
        "Numpad 2",
        "Numpad 3",
        "Numpad 0",
        "Numpad .",
        "",
        "",
        "<>",
        "F11",
        "F12",
    ];
    let extended = match (key.scan_code, key.extended) {
        (code, false) if usize::from(code) < ROW.len() && !ROW[usize::from(code)].is_empty() => {
            return Some(ROW[usize::from(code)].to_string());
        }
        (code @ 0x64..=0x6e, false) => return Some(format!("F{}", code - 0x64 + 13)),
        (0x76, false) => "F24",
        (0x1c, true) => "Numpad Enter",
        (0x1d, true) => "Right Ctrl",
        (0x35, true) => "Numpad /",
        (0x36, true) => "Right Shift",
        (0x37, true) => "Print Screen",
        (0x38, true) => "Right Alt",
        (0x45, true) => "Num Lock",
        (0x47, true) => "Home",
        (0x48, true) => "Up",
        (0x49, true) => "Page Up",
        (0x4b, true) => "Left",
        (0x4d, true) => "Right",
        (0x4f, true) => "End",
        (0x50, true) => "Down",
        (0x51, true) => "Page Down",
        (0x52, true) => "Insert",
        (0x53, true) => "Delete",
        (0x5b, true) => "Left Super",
        (0x5c, true) => "Right Super",
        (0x5d, true) => "Menu",
        (0x20, true) => "Mute",
        (0x2e, true) => "Volume Down",
        (0x30, true) => "Volume Up",
        (0x19, true) => "Next Track",
        (0x10, true) => "Previous Track",
        (0x22, true) => "Play/Pause",
        (0x24, true) => "Stop",
        _ => return None,
    };
    Some(extended.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(scan_code: u16, extended: bool) -> Option<String> {
        name(Key {
            scan_code,
            extended,
        })
    }

    #[test]
    fn names_by_scan_code() {
        assert_eq!(key(0x1e, false).as_deref(), Some("A"));
        assert_eq!(key(0x1d, false).as_deref(), Some("Left Ctrl"));
        assert_eq!(key(0x1d, true).as_deref(), Some("Right Ctrl"));
        assert_eq!(key(0x45, false).as_deref(), Some("Pause"));
        assert_eq!(key(0x45, true).as_deref(), Some("Num Lock"));
        assert_eq!(key(0x58, false).as_deref(), Some("F12"));
        assert_eq!(key(0x64, false).as_deref(), Some("F13"));
        assert_eq!(key(0x76, false).as_deref(), Some("F24"));
        assert_eq!(key(0x48, true).as_deref(), Some("Up"));
        assert_eq!(key(0x7f, false), None);
    }
}
