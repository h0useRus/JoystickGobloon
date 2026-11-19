//! Loading sherpa-onnx and libvosk per platform: its file name, how its own dependencies are found and how
//! it wants file paths; the voice thread's priority.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use unix::{
    SHERPA_LIBRARY, VOSK_LIBRARY, load, lower_priority, native_path, tts_library,
};
#[cfg(windows)]
pub(crate) use windows::{
    SHERPA_LIBRARY, VOSK_LIBRARY, load, lower_priority, native_path, tts_library,
};
