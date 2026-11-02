//! Loading libvosk per platform: its file name, how its own dependencies are found and how it
//! wants file paths.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use unix::{LIBRARY, load, native_path};
#[cfg(windows)]
pub(crate) use windows::{LIBRARY, load, native_path};
