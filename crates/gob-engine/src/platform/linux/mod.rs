//! Linux: keyboard and mouse through two uinput devices (gob-uinput); speech comes with
//! speech-dispatcher (docs/LINUX_PLAN.md phase 5), until then none.

mod injector;
mod keystate;

pub(crate) use super::none::Speech;
pub(crate) use injector::Injector;
