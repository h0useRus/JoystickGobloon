//! SystemGobloon: Joystick Gobloon's system part, one API on every system. On Linux a root
//! service (crate `system-gobloon`) hides devices and keeps the virtual ones; on Windows our own
//! driver will do the same. Both speak the protocol in [`protocol`]: fixed little-endian records
//! without text parsing, so a kernel driver can take them as they are (`no_std` without the `std`
//! feature). Only the transport differs, hidden in the client: a Unix socket with descriptors in
//! `SCM_RIGHTS` on Linux (`platform/linux.rs`), `DeviceIoControl` on Windows (later).
#![cfg_attr(not(feature = "std"), no_std)]

pub mod protocol;

#[cfg(all(feature = "std", target_os = "linux"))]
mod platform {
    pub mod install;
    pub mod linux;
}

/// Installing, updating and removing the service (Linux).
#[cfg(all(feature = "std", target_os = "linux"))]
pub use platform::install;

#[cfg(all(feature = "std", target_os = "linux"))]
pub use platform::linux::{Client, Error, Frame, SOCKET_PATH, Watch, recv, send};
