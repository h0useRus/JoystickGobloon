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
        Some(short) => CString::new(String::from_utf16(&short).ok()?).ok(),
        None => CString::new(text).ok(),
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
