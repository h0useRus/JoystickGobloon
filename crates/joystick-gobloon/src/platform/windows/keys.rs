//! Keyboard state and key names (Win32).

use gob_core::Key;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyNameTextW, MAPVK_VK_TO_VSC_EX, MapVirtualKeyW,
};

/// Keys held right now. Mouse buttons and the side-neutral Shift/Ctrl/Alt codes are
/// skipped (their left/right versions are reported instead).
pub(super) fn held() -> Vec<Key> {
    let mut keys = Vec::new();
    for vk in 0x08..=0xFEi32 {
        if matches!(vk, 0x10..=0x12) {
            continue;
        }
        // SAFETY: plain Win32 state queries.
        let down = unsafe { GetAsyncKeyState(vk) } < 0;
        if !down {
            continue;
        }
        let Some(key) = scan_code(vk as u32) else {
            continue;
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// Set 1 scan code of a virtual key. Windows maps the navigation keys to the numpad's codes
/// without E0 (Up -> 0x48 = Num 8), Print Screen to 0x54 and Num Lock without E0: those are
/// set here as the keyboard sends them.
fn scan_code(vk: u32) -> Option<Key> {
    let key = |scan_code, extended| {
        Some(Key {
            scan_code,
            extended,
        })
    };
    match vk {
        // Page Up, Page Down, End, Home, arrows, Insert, Delete.
        0x21..=0x28 | 0x2D | 0x2E => {
            // SAFETY: plain value conversion.
            let code = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC_EX) };
            key((code & 0xFF) as u16, true)
        }
        0x2C => key(0x37, true),  // VK_SNAPSHOT
        0x90 => key(0x45, true),  // VK_NUMLOCK
        0x13 => key(0x45, false), // VK_PAUSE
        _ => {
            // SAFETY: plain value conversion.
            let code = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC_EX) };
            let scan_code = (code & 0xFF) as u16;
            (scan_code != 0).then_some(Key {
                scan_code,
                extended: code >> 8 == 0xE0,
            })
        }
    }
}

/// Name in the current keyboard layout, e.g. "Right Ctrl".
pub(super) fn name(key: Key) -> String {
    let lparam = (i32::from(key.scan_code) << 16) | (i32::from(key.extended) << 24);
    let mut buf = [0u16; 64];
    // SAFETY: the buffer outlives the call; the length is passed with it.
    let n = unsafe { GetKeyNameTextW(lparam, &mut buf) };
    if n > 0 {
        String::from_utf16_lossy(&buf[..n as usize])
    } else {
        super::super::key_code(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_keys_are_extended() {
        let e0 = |scan_code| {
            Some(Key {
                scan_code,
                extended: true,
            })
        };
        assert_eq!(scan_code(0x26), e0(0x48), "Up, not Num 8");
        assert_eq!(scan_code(0x25), e0(0x4B), "Left");
        assert_eq!(scan_code(0x24), e0(0x47), "Home");
        assert_eq!(scan_code(0x2E), e0(0x53), "Delete");
        assert_eq!(scan_code(0x2C), e0(0x37), "Print Screen");
        assert_eq!(scan_code(0x90), e0(0x45), "Num Lock");
        assert_eq!(
            scan_code(0x68),
            Some(Key {
                scan_code: 0x48,
                extended: false
            }),
            "Num 8"
        );
        assert_eq!(scan_code(0xA3), e0(0x1D), "Right Ctrl");
    }

    #[test]
    fn windows_names_keys_left_and_right() {
        let l_ctrl = name(Key {
            scan_code: 0x1D,
            extended: false,
        });
        let r_ctrl = name(Key {
            scan_code: 0x1D,
            extended: true,
        });
        println!("{l_ctrl} / {r_ctrl}");
        assert!(!l_ctrl.contains("0x") && !r_ctrl.contains("0x"));
        assert_ne!(l_ctrl, r_ctrl, "the E0 flag tells right from left");
    }
}
