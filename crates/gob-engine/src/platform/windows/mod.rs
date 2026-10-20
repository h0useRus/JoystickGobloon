//! Windows: keyboard and mouse through `SendInput`, speech through SAPI.

mod sapi;
mod sendinput;

pub(crate) use sapi::Speech;
pub(crate) use sendinput::Injector;
