//! Natural voices: Piper voices spoken through sherpa-onnx's text to speech. Piper turns text
//! into sounds with espeak-ng (GNU GPL 3), which sherpa-onnx carries up to [`VERSION`] (2.0
//! dropped it): that build is not shipped with the program but downloaded on request, like the
//! voices, into the program's data folder, and loaded next to the speech recognition build
//! (both use the same ONNX Runtime, 1.28.2). Both are checked against the SHA-256 pinned here.
//!
//! Only the Piper (VITS) part of the config is spelled out; the other models' configs are kept
//! zeroed, as large as in `c-api.h` of [`VERSION`] (a test pins the sizes).

use std::ffi::{CString, c_char, c_float};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libloading::Library;

use crate::Error;
use crate::download::{self, Archive};
use crate::sherpa::{symbol, text};

/// The sherpa-onnx version of the voice library, whose C structs this file mirrors.
pub const VERSION: &str = "1.13.8";

const VOICES_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/";

#[repr(C)]
#[derive(Clone, Copy)]
struct Vits {
    model: *const c_char,
    lexicon: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    noise_scale: c_float,
    noise_scale_w: c_float,
    length_scale: c_float,
    dict_dir: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ModelConfig {
    vits: Vits,
    num_threads: i32,
    debug: i32,
    provider: *const c_char,
    /// matcha, kokoro, kitten, zipvoice, pocket and supertonic: not used, all zero.
    others: [u8; 344],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TtsConfig {
    model: ModelConfig,
    rule_fsts: *const c_char,
    max_num_sentences: i32,
    rule_fars: *const c_char,
    silence_scale: c_float,
}

#[repr(C)]
struct GeneratedAudio {
    samples: *const f32,
    n: i32,
    sample_rate: i32,
}

type Handle = *const std::ffi::c_void;
type VersionStr = unsafe extern "C" fn() -> *const c_char;
type CreateTts = unsafe extern "C" fn(*const TtsConfig) -> Handle;
type DestroyTts = unsafe extern "C" fn(Handle);
type Generate = unsafe extern "C" fn(Handle, *const c_char, i32, c_float) -> *const GeneratedAudio;
type DestroyAudio = unsafe extern "C" fn(*const GeneratedAudio);

struct Api {
    create: CreateTts,
    destroy: DestroyTts,
    generate: Generate,
    destroy_audio: DestroyAudio,
    // Last: the function pointers above point into it.
    _lib: Library,
}

/// One Piper voice, loaded: about its model's size in memory (60-80 MB).
pub struct Voice {
    api: Arc<Api>,
    ptr: Handle,
}

// SAFETY: used by one thread at a time (`&self` calls happen on the speech thread only).
unsafe impl Send for Voice {}

impl Voice {
    /// Loads sherpa-onnx with text to speech from `library_dir` and the voice in `voice_dir`.
    pub fn load(library_dir: &Path, voice_dir: &Path) -> Result<Self, Error> {
        let path = library_dir.join(crate::platform::SHERPA_LIBRARY);
        let lib = crate::platform::load(&path).map_err(|e| Error::Library(e.to_string()))?;
        // SAFETY: the types match c-api.h; the pointers stay valid while `_lib` is loaded.
        let api = unsafe {
            let version: VersionStr = symbol(&lib, b"SherpaOnnxGetVersionStr\0")?;
            let version = text(version());
            if version != VERSION {
                return Err(Error::Library(format!(
                    "sherpa-onnx {version} found, {VERSION} needed for voices"
                )));
            }
            Api {
                create: symbol(&lib, b"SherpaOnnxCreateOfflineTts\0")?,
                destroy: symbol(&lib, b"SherpaOnnxDestroyOfflineTts\0")?,
                generate: symbol(&lib, b"SherpaOnnxOfflineTtsGenerate\0")?,
                destroy_audio: symbol(&lib, b"SherpaOnnxDestroyOfflineTtsGeneratedAudio\0")?,
                _lib: lib,
            }
        };
        let files = VoiceFiles::find(voice_dir)
            .ok_or_else(|| Error::Model(format!("{} is not a voice", voice_dir.display())))?;
        let c = |p: &Path| {
            crate::platform::native_path(p).ok_or_else(|| {
                Error::Model(format!(
                    "{} cannot be opened by the speech library: move the data folder to a path \
                     with only Latin letters",
                    p.display()
                ))
            })
        };
        let (model, tokens, data) = (c(&files.model)?, c(&files.tokens)?, c(&files.data)?);
        let empty = CString::default();
        let cpu = CString::new("cpu").expect("no zero byte");
        let config = TtsConfig {
            model: ModelConfig {
                vits: Vits {
                    model: model.as_ptr(),
                    lexicon: empty.as_ptr(),
                    tokens: tokens.as_ptr(),
                    data_dir: data.as_ptr(),
                    noise_scale: 0.667,
                    noise_scale_w: 0.8,
                    length_scale: 1.0,
                    dict_dir: empty.as_ptr(),
                },
                // One thread: a phrase is synthesized in a fraction of its length, and the
                // game keeps the other cores.
                num_threads: 1,
                debug: 0,
                provider: cpu.as_ptr(),
                others: [0; 344],
            },
            rule_fsts: empty.as_ptr(),
            max_num_sentences: 1,
            rule_fars: empty.as_ptr(),
            silence_scale: 0.2,
        };
        // SAFETY: every pointer is a C string alive for the call (sherpa-onnx copies what it
        // keeps); the zeroed parts are null pointers and zero numbers as C expects.
        let ptr = unsafe { (api.create)(&config) };
        if ptr.is_null() {
            return Err(Error::Model(format!(
                "{} is not a voice sherpa-onnx can load",
                voice_dir.display()
            )));
        }
        Ok(Self {
            api: Arc::new(api),
            ptr,
        })
    }

    /// Speaks `text` into mono samples (-1..1) and their sample rate. `speed` 1.0 = normal,
    /// 2.0 = twice as fast.
    pub fn speak(&self, text: &str, speed: f32) -> Option<(Vec<f32>, u32)> {
        let text = CString::new(text.replace('\0', " ")).ok()?;
        // SAFETY: valid voice and C string; the audio is copied before it is freed.
        unsafe {
            let audio = (self.api.generate)(self.ptr, text.as_ptr(), 0, speed.clamp(0.25, 4.0));
            if audio.is_null() {
                return None;
            }
            let a = &*audio;
            let samples = if a.samples.is_null() || a.n <= 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(a.samples, a.n as usize).to_vec()
            };
            let rate = u32::try_from(a.sample_rate).unwrap_or(0);
            (self.api.destroy_audio)(audio);
            (rate > 0 && !samples.is_empty()).then_some((samples, rate))
        }
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        // SAFETY: made by SherpaOnnxCreateOfflineTts.
        unsafe { (self.api.destroy)(self.ptr) };
    }
}

/// The files of an unpacked Piper voice.
struct VoiceFiles {
    model: PathBuf,
    tokens: PathBuf,
    /// espeak-ng's data, which turns text into sounds.
    data: PathBuf,
}

impl VoiceFiles {
    fn find(dir: &Path) -> Option<Self> {
        let model = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|e| e == "onnx"))?;
        let tokens = dir.join("tokens.txt");
        let data = dir.join("espeak-ng-data");
        (tokens.is_file() && data.is_dir()).then_some(Self {
            model,
            tokens,
            data,
        })
    }
}

/// A Piper voice to download (only voices whose recordings may be used freely: CC0 or CC BY).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceInfo {
    /// sherpa-onnx's name for it, `vits-piper-de_DE-thorsten-medium`: its archive and folder.
    pub id: &'static str,
    /// The program's language code (`lang/*.ftl`).
    pub language: &'static str,
    /// The speaker's name as shown.
    pub name: &'static str,
    /// The license of its recordings, as shown.
    pub license: &'static str,
    /// Download size in bytes.
    pub bytes: u64,
    pub sha256: &'static str,
}

impl VoiceInfo {
    /// The name voices are listed and stored under: `Thorsten (de, Piper)`.
    pub fn display_name(&self) -> String {
        format!("{} ({}, Piper)", self.name, self.language)
    }

    fn folder(&self) -> &'static str {
        self.id
    }

    pub fn archive(&self) -> Archive {
        Archive {
            name: self.folder(),
            url: format!("{VOICES_URL}{}.tar.bz2", self.folder()),
            bytes: self.bytes,
            sha256: self.sha256,
        }
    }
}

/// The voices (checked 2026-11-18: sizes and SHA-256 of the release assets, licenses from each
/// voice's model card). Korean has no Piper voice.
pub const VOICES: &[VoiceInfo] = &[
    VoiceInfo {
        id: "vits-piper-en_GB-cori-medium",
        language: "en",
        name: "Cori",
        license: "CC BY 4.0",
        bytes: 67_257_412,
        sha256: "49c9a5361bbdd95d7ca9687c4de11e5908481f65e7c7c368960df79949fdac2b",
    },
    VoiceInfo {
        id: "vits-piper-en_US-libritts_r-medium",
        language: "en",
        name: "LibriTTS",
        license: "CC BY 4.0",
        bytes: 82_038_311,
        sha256: "10dc268f3e371696d721486123e2705a9fc1faa113491979fde4d88dba1f1b1c",
    },
    VoiceInfo {
        id: "vits-piper-de_DE-thorsten-medium",
        language: "de",
        name: "Thorsten",
        license: "CC0",
        bytes: 67_214_254,
        sha256: "50487d9c95fdf2191f31d2588569381063ba1591dcd4c7d4bdd30f12b2191714",
    },
    VoiceInfo {
        id: "vits-piper-es_ES-davefx-medium",
        language: "es",
        name: "Davefx",
        license: "CC0",
        bytes: 67_184_952,
        sha256: "a3f6beb54a9cb893279f72978a22f807a4d9fc9c7848157b524d5cc7b7f58b22",
    },
    VoiceInfo {
        id: "vits-piper-es_ES-sharvard-medium",
        language: "es",
        name: "Sharvard",
        license: "CC BY 3.0",
        bytes: 80_318_184,
        sha256: "b30a7a83df0518f0ee1c7039506648cade99f1f9b498fc49ed2ced2e2536bb5a",
    },
    VoiceInfo {
        id: "vits-piper-fr_FR-siwis-medium",
        language: "fr",
        name: "Siwis",
        license: "CC BY 4.0",
        bytes: 67_207_459,
        sha256: "375909aa30842b3a4efa10b1beb1d761af792960ae6873b4d53889f96c66195b",
    },
    VoiceInfo {
        id: "vits-piper-ru_RU-denis-medium",
        language: "ru",
        name: "Denis",
        license: "CC0",
        bytes: 67_190_991,
        sha256: "efa4c18e0b5e32b81d1b6df36b9d312831e5d545200e27848ef926a4cd930300",
    },
    VoiceInfo {
        id: "vits-piper-ru_RU-dmitri-medium",
        language: "ru",
        name: "Dmitri",
        license: "CC0",
        bytes: 67_188_551,
        sha256: "c86d0803737de13d441923ff3b3f309482fab8d7af3ec85949942809eb9a3660",
    },
    VoiceInfo {
        id: "vits-piper-zh_CN-chaowen-medium",
        language: "zh",
        name: "Chaowen",
        license: "CC0",
        bytes: 60_443_846,
        sha256: "1b69bdcf10bb6331c7f8a7591940febeb175c6bb86caa3802d96531f8e4ad211",
    },
];

/// The folder voices and the voice library are kept in, under the program's voice folder.
pub fn dir(voice_dir: &Path) -> PathBuf {
    voice_dir.join("tts")
}

/// The voice's folder in `dir` when it is installed there.
pub fn installed(dir: &Path, voice: &VoiceInfo) -> Option<PathBuf> {
    let path = dir.join(voice.folder());
    VoiceFiles::find(&path).map(|_| path)
}

/// Downloads a voice into `dir` (see [`download::install`]).
pub fn install(
    dir: &Path,
    voice: &VoiceInfo,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<PathBuf, Error> {
    download::install(dir, &voice.archive(), is_voice, progress, cancel)
}

fn is_voice(dir: &Path) -> bool {
    VoiceFiles::find(dir).is_some()
}

pub fn remove(dir: &Path, voice: &VoiceInfo) -> std::io::Result<()> {
    download::remove(dir, voice.folder())
}

/// The voice library: sherpa-onnx with text to speech for this platform.
pub fn library() -> Archive {
    crate::platform::tts_library()
}

/// The folder sherpa-onnx with text to speech is loaded from, when it is installed in `dir`.
pub fn library_installed(dir: &Path) -> Option<PathBuf> {
    let lib = dir.join(library().name).join("lib");
    lib.join(crate::platform::SHERPA_LIBRARY)
        .is_file()
        .then_some(lib)
}

/// Downloads the voice library into `dir` (see [`download::install`]).
pub fn install_library(
    dir: &Path,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<PathBuf, Error> {
    download::install(dir, &library(), is_library, progress, cancel)
}

fn is_library(dir: &Path) -> bool {
    dir.join("lib")
        .join(crate::platform::SHERPA_LIBRARY)
        .is_file()
}

pub fn remove_library(dir: &Path) -> std::io::Result<()> {
    download::remove(dir, library().name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_layout_matches_c_api() {
        // sizeof/offsetof from c-api.h of sherpa-onnx VERSION, compiled for x64 with MSVC.
        assert_eq!(size_of::<Vits>(), 56);
        assert_eq!(size_of::<ModelConfig>(), 416);
        assert_eq!(size_of::<TtsConfig>(), 448);
        assert_eq!(std::mem::offset_of!(ModelConfig, others), 72);
        assert_eq!(std::mem::offset_of!(TtsConfig, rule_fsts), 416);
        assert_eq!(std::mem::offset_of!(TtsConfig, silence_scale), 440);
        assert_eq!(size_of::<GeneratedAudio>(), 16);
    }

    #[test]
    fn every_voice_is_complete() {
        for v in VOICES {
            assert_eq!(v.sha256.len(), 64, "{}", v.id);
            assert!(v.archive().url.ends_with(".tar.bz2"), "{}", v.id);
            assert!(v.id.starts_with("vits-piper-"), "{}", v.id);
        }
        let library = library();
        assert_eq!(library.sha256.len(), 64);
        assert!(library.url.contains(VERSION));
    }

    #[test]
    fn a_voice_folder_needs_model_tokens_and_data() {
        let dir = std::env::temp_dir().join(format!("gob-voice-tts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("espeak-ng-data")).unwrap();
        assert!(VoiceFiles::find(&dir).is_none());
        std::fs::write(dir.join("x.onnx"), "m").unwrap();
        std::fs::write(dir.join("tokens.txt"), "t").unwrap();
        assert!(VoiceFiles::find(&dir).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
