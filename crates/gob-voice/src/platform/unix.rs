use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// sherpa-onnx's C API in its Linux release; libonnxruntime.so lies next to it.
pub(crate) const SHERPA_LIBRARY: &str = "libsherpa-onnx-c-api.so";

/// The name in Vosk's Linux release.
pub(crate) const VOSK_LIBRARY: &str = "libvosk.so";

/// sherpa-onnx with text to speech (its last build with espeak-ng, GPL 3): downloaded on
/// request, not shipped.
pub(crate) fn tts_library() -> crate::download::Archive {
    crate::download::Archive {
        name: "sherpa-onnx-v1.13.8-linux-x64-shared-lib",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-linux-x64-shared-lib.tar.bz2".into(),
        bytes: 9_816_899,
        sha256: "3892d184be41027e18165e67f549cd4e4cdd8dcd73ac5579e97afd55e14e30b6",
    }
}

pub(crate) fn load(path: &Path) -> Result<libloading::Library, libloading::Error> {
    // SAFETY: sherpa-onnx, ONNX Runtime and libvosk run no code on load that could break the
    // program.
    unsafe { libloading::Library::new(path) }
}

/// The path as sherpa-onnx opens files: the bytes as they are.
pub(crate) fn native_path(path: &Path) -> Option<CString> {
    CString::new(path.as_os_str().as_bytes()).ok()
}

/// Decoding a phrase must not take the CPU from the game: the voice thread gets a higher nice
/// value (lower priority).
pub(crate) fn lower_priority() {
    // PRIO_PROCESS with who 0 changes the calling thread on Linux (threads have their own nice
    // value); 5 = a little nicer than the rest of the program.
    // SAFETY: a plain syscall with valid arguments.
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 5) } != 0 {
        log::debug!("voice thread priority: {}", std::io::Error::last_os_error());
    }
}
