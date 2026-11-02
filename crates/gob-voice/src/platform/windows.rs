use std::path::Path;

use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library};

/// The name in Vosk's Windows release; its MinGW runtime DLLs lie next to it.
pub(crate) const LIBRARY: &str = "libvosk.dll";

/// Loads the library; a full path finds the DLLs next to it, not only next to the exe.
pub(crate) fn load(path: &Path) -> Result<libloading::Library, libloading::Error> {
    let flags = if path.is_absolute() {
        LOAD_WITH_ALTERED_SEARCH_PATH
    } else {
        0
    };
    // SAFETY: libvosk runs no code on load that could break the program.
    unsafe { Library::load_with_flags(path, flags) }.map(Into::into)
}
