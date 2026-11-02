use std::ffi::CString;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library};
use windows::Win32::Globalization::{GetACP, WC_NO_BEST_FIT_CHARS, WideCharToMultiByte};
use windows::Win32::Storage::FileSystem::GetShortPathNameW;
use windows::core::{BOOL, PCSTR, PCWSTR};

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

/// The path as libvosk opens files: its MinGW build takes narrow paths in the system's ANSI
/// code page. A path that does not fit in it (a Chinese user name on a Western system) goes as
/// its 8.3 short name; `None` when there is none either.
pub(crate) fn native_path(path: &Path) -> Option<CString> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    ansi(&wide).or_else(|| ansi(&short_name(&wide)?))
}

/// `wide` in the ANSI code page; `None` when a character has no exact match there.
fn ansi(wide: &[u16]) -> Option<CString> {
    if wide.is_empty() {
        return None;
    }
    // SAFETY: GetACP has no preconditions.
    let code_page = unsafe { GetACP() };
    let convert = |out: Option<&mut [u8]>, lossy: &mut BOOL| {
        // SAFETY: valid slices; `lossy` outlives the call.
        unsafe {
            WideCharToMultiByte(
                code_page,
                WC_NO_BEST_FIT_CHARS,
                wide,
                out,
                PCSTR::null(),
                Some(lossy),
            )
        }
    };
    let mut lossy = BOOL(0);
    let len = usize::try_from(convert(None, &mut lossy)).ok()?;
    let mut bytes = vec![0u8; len];
    let written = convert(Some(&mut bytes), &mut lossy);
    if written <= 0 || lossy.as_bool() {
        return None;
    }
    CString::new(bytes).ok()
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
