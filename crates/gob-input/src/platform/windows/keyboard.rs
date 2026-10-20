//! Keyboard input through a low-level Windows hook (like Gremlin's KeyboardHook).
//!
//! Keys still reach other programs. Auto-repeat is filtered (only the first press and the
//! release are reported) and injected keys are ignored, so keys sent by our own
//! map-to-keyboard or macro actions never come back as input.

use std::sync::Arc;

/// Reports (scan code, extended, pressed).
pub type KeySink = Arc<dyn Fn(u16, bool, bool) + Send + Sync>;

mod imp {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    use std::thread::JoinHandle;

    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_EXTENDED,
        LLKHF_INJECTED, MSG, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
    };

    use super::KeySink;

    struct State {
        sink: KeySink,
        down: HashSet<(u16, bool)>,
    }

    /// The hook procedure has no context argument: the sink lives here.
    static STATE: OnceLock<Mutex<Option<State>>> = OnceLock::new();

    unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let injected = info.flags.0 & LLKHF_INJECTED.0 != 0;
            if !injected {
                let key = (
                    (info.scanCode & 0xFF) as u16,
                    info.flags.0 & LLKHF_EXTENDED.0 != 0,
                );
                let pressed = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                if let Some(Ok(mut guard)) = STATE.get().map(|m| m.lock())
                    && let Some(state) = guard.as_mut()
                {
                    // Auto-repeat: a press for a key that is already down.
                    let changed = if pressed {
                        state.down.insert(key)
                    } else {
                        state.down.remove(&key)
                    };
                    if changed {
                        (state.sink)(key.0, key.1, pressed);
                    }
                }
            }
        }
        // SAFETY: passing the event on is always allowed.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    pub struct Hook {
        thread: Option<JoinHandle<()>>,
        thread_id: u32,
    }

    pub fn start(sink: KeySink) -> Option<Hook> {
        *STATE.get_or_init(|| Mutex::new(None)).lock().ok()? = Some(State {
            sink,
            down: HashSet::new(),
        });
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("gob-keyboard".into())
            .spawn(move || {
                // SAFETY: the hook is installed and removed on this thread, which runs the
                // message loop the hook needs.
                unsafe {
                    let module = GetModuleHandleW(None).ok();
                    let hook =
                        SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), module.map(Into::into), 0);
                    let _ = tx.send(GetCurrentThreadId());
                    let Ok(hook) = hook else {
                        log::error!("keyboard hook could not be installed");
                        return;
                    };
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    let _ = UnhookWindowsHookEx(hook);
                }
            })
            .ok()?;
        let thread_id = rx.recv().ok()?;
        Some(Hook {
            thread: Some(thread),
            thread_id,
        })
    }

    impl Drop for Hook {
        fn drop(&mut self) {
            // SAFETY: posting WM_QUIT ends the hook thread's message loop.
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
            if let Some(Ok(mut s)) = STATE.get().map(|m| m.lock()) {
                *s = None;
            }
        }
    }
}

pub use imp::{Hook, start};
