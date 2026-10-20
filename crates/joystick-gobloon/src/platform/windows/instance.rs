//! One running copy per user: a second start wakes the first one and exits.
//!
//! The first instance owns a named mutex and waits on a named event; a later start finds
//! the mutex, lets the first instance take the foreground and sets the event.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent,
    WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
use windows::core::HSTRING;

/// Debug builds get their own names, so a development build runs next to an installed one.
const BASE: &str = if cfg!(debug_assertions) {
    "Local\\JoystickGobloon.debug"
} else {
    "Local\\JoystickGobloon"
};

/// Wrapper so the event handle can move to the listener thread.
pub(crate) struct Event(HANDLE);
// SAFETY: a kernel event handle may be used from any thread.
unsafe impl Send for Event {}

impl Event {
    /// Through a method, so closures capture the `Send` wrapper, not the raw handle.
    fn handle(&self) -> HANDLE {
        self.0
    }
}

/// `Some(event)` for the first instance; `None` when another one runs (it was woken).
pub(super) fn claim() -> Option<Event> {
    let mutex_name = HSTRING::from(format!("{BASE}.instance"));
    let event_name = HSTRING::from(format!("{BASE}.show"));
    // SAFETY: plain kernel object calls; the handles live as long as the process.
    unsafe {
        let mutex = CreateMutexW(None, false, &mutex_name);
        let running = GetLastError() == ERROR_ALREADY_EXISTS;
        if mutex.is_err() || !running {
            // First instance (or the mutex failed: start anyway). Keep the mutex open.
            let event = CreateEventW(None, false, false, &event_name).ok()?;
            return Some(Event(event));
        }
        if let Ok(event) = OpenEventW(EVENT_MODIFY_STATE, false, &event_name) {
            // We are in the foreground (the user just started us): pass that on.
            let _ = AllowSetForegroundWindow(ASFW_ANY);
            let _ = SetEvent(event);
        }
        None
    }
}

/// Calls `show` whenever another start signals the event.
pub(super) fn listen(event: Event, show: Box<dyn Fn() + Send>) {
    let spawned = std::thread::Builder::new()
        .name("gob-instance".into())
        .spawn(move || {
            loop {
                // SAFETY: the handle stays open for the life of the process.
                let r = unsafe { WaitForSingleObject(event.handle(), INFINITE) };
                if r.0 != 0 {
                    return;
                }
                show();
            }
        });
    if let Err(e) = spawned {
        log::error!("single instance listener: {e}");
    }
}
