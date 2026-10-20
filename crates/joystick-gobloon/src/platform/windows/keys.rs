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
        let code = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
        let scan_code = (code & 0xFF) as u16;
        if scan_code == 0 {
            continue;
        }
        let key = Key {
            scan_code,
            extended: code >> 8 == 0xE0,
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
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
