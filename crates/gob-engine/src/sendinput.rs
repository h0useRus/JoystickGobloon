//! Mouse and keyboard output through the Windows `SendInput` API.

use gob_core::MouseButton;

#[cfg(windows)]
mod imp {
    use std::mem::size_of;

    use gob_core::MouseButton;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS,
        KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VSC_TO_VK_EX, MOUSE_EVENT_FLAGS,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
        MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MapVirtualKeyW, SendInput, VIRTUAL_KEY,
    };

    const WHEEL_DELTA: i32 = 120;
    const XBUTTON1: i32 = 1;
    const XBUTTON2: i32 = 2;

    fn send(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, data: i32) {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data as _,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    pub fn mouse_move(dx: i32, dy: i32) {
        send(MOUSEEVENTF_MOVE, dx, dy, 0);
    }

    pub fn mouse_button(button: MouseButton, pressed: bool) {
        let (down, up, data) = match button {
            MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
            MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0),
            MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0),
            MouseButton::Back => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, XBUTTON1),
            MouseButton::Forward => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, XBUTTON2),
            MouseButton::WheelUp | MouseButton::WheelDown => return,
        };
        send(if pressed { down } else { up }, 0, 0, data);
    }

    /// Positive notches scroll up.
    pub fn mouse_wheel(notches: i32) {
        send(MOUSEEVENTF_WHEEL, 0, 0, notches * WHEEL_DELTA);
    }

    /// Sends both virtual key and scan code like Gremlin's keybd_event call.
    pub fn key(scan_code: u16, extended: bool, pressed: bool) {
        let prefix = if extended { 0xE000 } else { 0 };
        let vk = unsafe { MapVirtualKeyW(u32::from(scan_code) | prefix, MAPVK_VSC_TO_VK_EX) };
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        if !pressed {
            flags |= KEYEVENTF_KEYUP;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk as u16),
                    wScan: scan_code,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    /// Current key state, like Gremlin's keyboard condition.
    pub fn key_pressed(scan_code: u16, extended: bool) -> bool {
        let prefix = if extended { 0xE000 } else { 0 };
        let vk = unsafe { MapVirtualKeyW(u32::from(scan_code) | prefix, MAPVK_VSC_TO_VK_EX) };
        vk != 0 && unsafe { GetAsyncKeyState(vk as i32) } < 0
    }
}

#[cfg(not(windows))]
mod imp {
    use gob_core::MouseButton;

    pub fn mouse_move(_dx: i32, _dy: i32) {}
    pub fn mouse_button(_button: MouseButton, _pressed: bool) {}
    pub fn mouse_wheel(_notches: i32) {}
    pub fn key(_scan_code: u16, _extended: bool, _pressed: bool) {}
    pub fn key_pressed(_scan_code: u16, _extended: bool) -> bool {
        false
    }
}

pub fn mouse_move(dx: i32, dy: i32) {
    imp::mouse_move(dx, dy);
}

pub fn mouse_button(button: MouseButton, pressed: bool) {
    imp::mouse_button(button, pressed);
}

pub fn mouse_wheel(notches: i32) {
    imp::mouse_wheel(notches);
}

pub fn key(scan_code: u16, extended: bool, pressed: bool) {
    imp::key(scan_code, extended, pressed);
}

pub fn key_pressed(scan_code: u16, extended: bool) -> bool {
    imp::key_pressed(scan_code, extended)
}
