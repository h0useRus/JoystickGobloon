//! Mouse and keyboard output through the Windows `SendInput` API.

use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};

use gob_core::MouseButton;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VSC_TO_VK_EX, MOUSE_EVENT_FLAGS,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MapVirtualKeyW, SendInput, VIRTUAL_KEY,
};

const WHEEL_DELTA: i32 = 120;
const XBUTTON1: i32 = 1;
const XBUTTON2: i32 = 2;

pub(crate) struct Injector;

/// The last `SendInput` call was refused: logged once until one works again.
static REFUSED: AtomicBool = AtomicBool::new(false);

/// Sends one input event. Windows refuses injected input into a program running with higher
/// rights (a game started as administrator): that is logged once, so silent keys are explained.
fn inject(input: INPUT) {
    // SAFETY: one fully initialized INPUT with its size, as SendInput expects.
    let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    let refused = sent == 0;
    if REFUSED.swap(refused, Ordering::Relaxed) != refused && refused {
        log::warn!(
            "keyboard/mouse output refused by Windows ({}); a program running as administrator only takes keys from programs running as administrator too",
            std::io::Error::last_os_error()
        );
    }
}

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
    inject(input);
}

/// Virtual key of a set 1 scan code; 0 when there is none.
fn virtual_key(scan_code: u16, extended: bool) -> u32 {
    let prefix = if extended { 0xE000 } else { 0 };
    // SAFETY: plain value conversion, no pointers.
    unsafe { MapVirtualKeyW(u32::from(scan_code) | prefix, MAPVK_VSC_TO_VK_EX) }
}

impl crate::platform::Injector for Injector {
    fn mouse_move(&self, dx: i32, dy: i32) {
        send(MOUSEEVENTF_MOVE, dx, dy, 0);
    }

    fn mouse_button(&self, button: MouseButton, pressed: bool) {
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

    fn mouse_wheel(&self, notches: i32) {
        send(MOUSEEVENTF_WHEEL, 0, 0, notches * WHEEL_DELTA);
    }

    /// Sends both virtual key and scan code like Gremlin's keybd_event call.
    fn key(&self, scan_code: u16, extended: bool, pressed: bool) {
        let vk = virtual_key(scan_code, extended);
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
        inject(input);
    }

    /// Current key state, like Gremlin's keyboard condition.
    fn key_pressed(&self, scan_code: u16, extended: bool) -> bool {
        let vk = virtual_key(scan_code, extended);
        // SAFETY: plain query by virtual key, no pointers.
        vk != 0 && unsafe { GetAsyncKeyState(vk as i32) } < 0
    }
}
