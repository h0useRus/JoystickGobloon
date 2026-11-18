use std::ffi::CString;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library};
use windows::Win32::Storage::FileSystem::GetShortPathNameW;
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
};
use windows::core::PCWSTR;

/// sherpa-onnx's C API in its Windows release; onnxruntime.dll lies next to it.
pub(crate) const SHERPA_LIBRARY: &str = "sherpa-onnx-c-api.dll";

/// sherpa-onnx with text to speech (its last build with espeak-ng, GPL 3): downloaded on
/// request, not shipped.
pub(crate) fn tts_library() -> crate::download::Archive {
    crate::download::Archive {
        name: "sherpa-onnx-v1.13.8-win-x64-shared-MT-Release-lib",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-win-x64-shared-MT-Release-lib.tar.bz2".into(),
        bytes: 8_032_957,
        sha256: "b8eedf41bd6d3779218887b48367bb7a3ece5aaa7667f01f69ee823a12b0a9e7",
    }
}

/// Loads the library; a full path finds the DLLs next to it, not only next to the exe.
pub(crate) fn load(path: &Path) -> Result<libloading::Library, libloading::Error> {
    let flags = if path.is_absolute() {
        LOAD_WITH_ALTERED_SEARCH_PATH
    } else {
        0
    };
    // SAFETY: sherpa-onnx and ONNX Runtime run no code on load that could break the program.
    unsafe { Library::load_with_flags(path, flags) }.map(Into::into)
}

/// The path as sherpa-onnx opens files: narrow strings, read as UTF-8 on the way to ONNX
/// Runtime's wide paths. A path with characters outside ASCII goes as its 8.3 short name, which
/// is ASCII whatever the library assumes; the path itself when there is none (most volumes have
/// short names).
pub(crate) fn native_path(path: &Path) -> Option<CString> {
    let text = path.to_str()?;
    if text.is_ascii() {
        return CString::new(text).ok();
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    match short_name(&wide) {
        Some(short) if short.iter().all(|&c| c < 0x80) => {
            CString::new(String::from_utf16(&short).ok()?).ok()
        }
        _ => {
            // Model files would open (UTF-8), text files next to them not (the ANSI code page):
            // no narrow form of this path works for both.
            log::error!(
                "voice: {} has characters outside ASCII and no 8.3 short name; sherpa-onnx \
                 cannot open files there. Use a data folder with an ASCII path.",
                path.display()
            );
            CString::new(text).ok()
        }
    }
}

/// Decoding a phrase must not take the CPU from the game: the voice thread runs below normal.
pub(crate) fn lower_priority() {
    // SAFETY: the pseudo handle of the calling thread.
    if let Err(e) = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) } {
        log::debug!("voice thread priority: {e}");
    }
}

/// The 8.3 short name of an existing path (`wide` without a terminating zero).
fn short_name(wide: &[u16]) -> Option<Vec<u16>> {
    let long: Vec<u16> = wide.iter().copied().chain([0]).collect();
    let long = PCWSTR(long.as_ptr());
    // SAFETY: `long` is zero-terminated and alive for both calls.
    let len = unsafe { GetShortPathNameW(long, None) } as usize;
    if len == 0 {
        return None;
    }
    let mut short = vec![0u16; len];
    // SAFETY: as above; `short` holds `len` units including the zero.
    let written = unsafe { GetShortPathNameW(long, Some(&mut short)) } as usize;
    (written > 0 && written < len).then(|| short[..written].to_vec())
}
