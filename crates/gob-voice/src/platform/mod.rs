//! Loading libvosk per platform: its file name and how its own dependencies are found.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use unix::{LIBRARY, load};
#[cfg(windows)]
pub(crate) use windows::{LIBRARY, load};
