use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub(crate) const LIBRARY: &str = "libvosk.so";

/// sherpa-onnx's C API in its Linux release; libonnxruntime.so lies next to it.
pub(crate) const SHERPA_LIBRARY: &str = "libsherpa-onnx-c-api.so";

pub(crate) fn load(path: &Path) -> Result<libloading::Library, libloading::Error> {
    // SAFETY: libvosk runs no code on load that could break the program.
    unsafe { libloading::Library::new(path) }
}

/// The path as libvosk opens files: the bytes as they are.
pub(crate) fn native_path(path: &Path) -> Option<CString> {
    CString::new(path.as_os_str().as_bytes()).ok()
}
