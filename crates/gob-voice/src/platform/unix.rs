use std::path::Path;

pub(crate) const LIBRARY: &str = "libvosk.so";

pub(crate) fn load(path: &Path) -> Result<libloading::Library, libloading::Error> {
    // SAFETY: libvosk runs no code on load that could break the program.
    unsafe { libloading::Library::new(path) }
}
