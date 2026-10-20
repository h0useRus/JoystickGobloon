//! Linux: keyboard and mouse through two uinput devices (gob-uinput), speech through
//! speech-dispatcher.

mod injector;
mod keystate;
mod speech;

pub(crate) use injector::Injector;
pub(crate) use speech::Speech;
