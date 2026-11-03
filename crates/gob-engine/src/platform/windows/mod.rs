//! Windows: keyboard and mouse through `SendInput`, speech through SAPI.

mod sapi;
mod sendinput;

pub(crate) use sapi::Speech;
pub(crate) use sendinput::Injector;

/// `timeBeginPeriod(1)` / `timeEndPeriod(1)`: the default timer resolution is 15.6 ms.
pub(crate) fn fine_timer(on: bool) {
    use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
    // SAFETY: no pointers; the runner pairs every begin with an end.
    unsafe {
        if on {
            timeBeginPeriod(1);
        } else {
            timeEndPeriod(1);
        }
    }
}
