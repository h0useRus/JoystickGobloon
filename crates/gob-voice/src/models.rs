//! Speech models per engine and language: Vosk's small models from alphacephei.com (one per
//! language) and sherpa-onnx models from its GitHub releases (a small and, for most languages, a
//! large one), downloaded once into a folder the caller chooses (the program's data folder). Every download is checked against the SHA-256 pinned here before it is unpacked, and
//! a model counts as installed only when its files are there, so a broken or half-done download
//! is never used. German, Spanish and French share one multilingual sherpa-onnx model. Models of
//! both engines can be installed side by side; choosing the other engine removes nothing.
//!
//! The memory figures are what the model needed on a test PC while decoding (the whole process,
//! plans/SPEECH_PLAN.md "S0 results"): the program shows them so the user can choose.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::Error;
use crate::download::{self, Archive};

const BASE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/";
const VOSK_URL: &str = "https://alphacephei.com/vosk/models/";

/// The speech recognizer voice commands use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Engine {
    /// Vosk in grammar mode: hears only the profile's phrases (or nothing), so a short command
    /// said with an accent or on a quiet microphone still lands on the right phrase.
    Vosk,
    /// sherpa-onnx: writes down any speech, which the matcher then compares with the phrases.
    Sherpa,
}

impl Engine {
    pub const ALL: [Self; 2] = [Self::Vosk, Self::Sherpa];

    /// As settings store it.
    pub fn key(self) -> &'static str {
        match self {
            Self::Vosk => "vosk",
            Self::Sherpa => "sherpa",
        }
    }

    /// From settings; anything else is Vosk.
    pub fn parse(key: &str) -> Self {
        match key {
            "sherpa" => Self::Sherpa,
            _ => Self::Vosk,
        }
    }

    /// Its name (a product name, not translated).
    pub fn name(self) -> &'static str {
        match self {
            Self::Vosk => "Vosk",
            Self::Sherpa => "sherpa-onnx",
        }
    }
}

/// Small: less memory, a little less accurate. Large: more accurate, more memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Size {
    Small,
    Large,
}

impl Size {
    /// As settings store it.
    pub fn key(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Large => "large",
        }
    }

    /// From settings; anything else is small.
    pub fn parse(key: &str) -> Self {
        match key {
            "large" => Self::Large,
            _ => Self::Small,
        }
    }
}

/// A speech model to download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelInfo {
    pub engine: Engine,
    /// The program's language codes (`lang/*.ftl`) the model recognizes.
    pub languages: &'static [&'static str],
    pub size: Size,
    /// The name of the archive (without `.tar.bz2`) and of the folder it unpacks to.
    pub name: &'static str,
    /// Download size in bytes.
    pub bytes: u64,
    pub sha256: &'static str,
    /// Memory while recognizing, in MB (see the module notes).
    pub memory_mb: u32,
}

impl ModelInfo {
    pub fn archive(&self) -> Archive {
        Archive {
            name: self.name,
            url: match self.engine {
                Engine::Vosk => format!("{VOSK_URL}{}.zip", self.name),
                Engine::Sherpa => format!("{BASE_URL}{}.tar.bz2", self.name),
            },
            bytes: self.bytes,
            sha256: self.sha256,
        }
    }
}

const EUROPEAN: &[&str] = &["de", "es", "fr"];

/// The models. Vosk (checked 2026-11-01: each zip holds `<name>/am/final.mdl` and
/// `<name>/conf/model.conf`; memory measured for English, the others estimated from their size);
/// sherpa-onnx (checked 2026-11-18: sizes and SHA-256 of the release assets; each archive holds
/// its files in a folder of its own name).
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        engine: Engine::Vosk,
        languages: &["en"],
        size: Size::Small,
        name: "vosk-model-small-en-us-0.15",
        bytes: 41_205_931,
        sha256: "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498",
        memory_mb: 130,
    },
    ModelInfo {
        engine: Engine::Vosk,
        languages: &["de"],
        size: Size::Small,
        name: "vosk-model-small-de-0.15",
        bytes: 46_499_967,
        sha256: "b7e53c90b1f0a38456f4cd62b366ecd58803cd97cd42b06438e2c131713d5e43",
        memory_mb: 130,
    },
    ModelInfo {
        engine: Engine::Vosk,
        languages: &["es"],
        size: Size::Small,
        name: "vosk-model-small-es-0.42",
        bytes: 39_817_833,
        sha256: "09b239888f633ef2f0b4e09736e3d9936acfd810bc65d53fad45261762c6511f",
        memory_mb: 130,
    },
    ModelInfo {
        engine: Engine::Vosk,
        languages: &["fr"],
        size: Size::Small,
        name: "vosk-model-small-fr-0.22",
        bytes: 42_233_323,
        sha256: "cabf6180e177eb9b3a9a9d43a437bd5e549f3a7d09525e5d69a3fed787be12ad",
        memory_mb: 130,
    },
    ModelInfo {
        engine: Engine::Vosk,
        languages: &["ko"],
        size: Size::Small,
        name: "vosk-model-small-ko-0.22",
        bytes: 86_914_329,
        sha256: "eea36124087fed26c59996a4761519458e3bd185e8ea9d9865ad8760c4a1d989",
        memory_mb: 220,
    },
    ModelInfo {
        engine: Engine::Vosk,
        languages: &["ru"],
        size: Size::Small,
        name: "vosk-model-small-ru-0.22",
        bytes: 46_236_750,
        sha256: "961d5ff98a17f4aa6de69864d0aa71fa5bac682301d2b5d17a3f24c5c99a46d4",
        memory_mb: 130,
    },
    ModelInfo {
        engine: Engine::Vosk,
        languages: &["zh"],
        size: Size::Small,
        name: "vosk-model-small-cn-0.22",
        bytes: 43_898_754,
        sha256: "3af8b0e7e0f835ae9d414ce5df580237a3cfb08d586c9fbbb0f7ff29ad5b14ba",
        memory_mb: 130,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: &["en"],
        size: Size::Small,
        name: "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27",
        bytes: 29_858_559,
        sha256: "9ec31b342d8fa3240c3b81b8f82e1cf7e3ac467c93ca5a999b741d5887164f8d",
        memory_mb: 140,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: &["en"],
        size: Size::Large,
        name: "sherpa-onnx-nemo-parakeet_tdt_transducer_110m-en-36000-int8",
        bytes: 108_035_095,
        sha256: "f628312e9fdf8686374cb01a69425c41732529d540860311f16f37cbc32cfe9b",
        memory_mb: 340,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: &["ru"],
        size: Size::Small,
        name: "sherpa-onnx-zipformer-ru-int8-2025-04-20",
        bytes: 60_239_942,
        sha256: "d6a651569aacc9a177259fa54705dd76acae23f6a4d62ea6797bd220d4b57163",
        memory_mb: 260,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: &["ru"],
        size: Size::Large,
        name: "sherpa-onnx-nemo-transducer-giga-am-v3-russian-2025-12-16",
        bytes: 167_388_020,
        sha256: "20a439491904b839f35eb8efaa3d99cbfaaad0c6dcba22d06ff218bb0056772d",
        memory_mb: 500,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: EUROPEAN,
        size: Size::Small,
        name: "sherpa-onnx-nemo-fast-conformer-transducer-be-de-en-es-fr-hr-it-pl-ru-uk-20k-int8",
        bytes: 106_546_673,
        sha256: "06072bad277f0f4c29cc866d7c62b0e47936da39afafeae453faa925025ccad6",
        memory_mb: 340,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: EUROPEAN,
        size: Size::Large,
        name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
        bytes: 487_211_443,
        sha256: "1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9",
        memory_mb: 1370,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: &["zh"],
        size: Size::Small,
        name: "sherpa-onnx-moonshine-base-zh-quantized-2026-02-27",
        bytes: 99_905_420,
        sha256: "6495d4240f66bcdd2bbbbfefaa687c463150d467854001270934e88940b733c1",
        memory_mb: 345,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: &["zh"],
        size: Size::Large,
        name: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
        bytes: 165_783_878,
        sha256: "7305f7905bfcf77fa0b39388a313f3da35c68d971661a65475b56fb2162c8e63",
        memory_mb: 410,
    },
    ModelInfo {
        engine: Engine::Sherpa,
        languages: &["ko"],
        size: Size::Small,
        name: "sherpa-onnx-moonshine-tiny-ko-quantized-2026-02-27",
        bytes: 49_153_415,
        sha256: "d3b6c5390a7859c9ef20ff4f20b0766fcbad1dc06c0f509fe4840a3a302112dc",
        memory_mb: 200,
    },
];

/// `en-GB` → `en`.
fn base(language: &str) -> &str {
    language.split(['-', '_']).next().unwrap_or(language)
}

/// The languages `engine` has a model for, each once, in [`MODELS`] order.
pub fn languages(engine: Engine) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for m in MODELS.iter().filter(|m| m.engine == engine) {
        for l in m.languages {
            if !out.contains(l) {
                out.push(l);
            }
        }
    }
    out
}

/// `engine`'s models of a language code (`en`, `de`...; a region like `en-GB` is ignored), small
/// first.
pub fn for_language(engine: Engine, language: &str) -> Vec<&'static ModelInfo> {
    let base = base(language);
    let mut found: Vec<&ModelInfo> = MODELS
        .iter()
        .filter(|m| m.engine == engine)
        .filter(|m| m.languages.iter().any(|l| l.eq_ignore_ascii_case(base)))
        .collect();
    found.sort_by_key(|m| m.size == Size::Large);
    found
}

/// `engine`'s model of that size for the language; the small one when it has no other.
pub fn pick(engine: Engine, language: &str, size: Size) -> Option<&'static ModelInfo> {
    let models = for_language(engine, language);
    models
        .iter()
        .find(|m| m.size == size)
        .or(models.first())
        .copied()
}

/// The model's folder in `dir` when it is installed there.
pub fn installed(dir: &Path, model: &ModelInfo) -> Option<PathBuf> {
    let path = dir.join(model.name);
    usable(model.engine)(&path).then_some(path)
}

/// Deletes the model's folder in `dir` (nothing when it is not there).
pub fn remove(dir: &Path, model: &ModelInfo) -> io::Result<()> {
    download::remove(dir, model.name)
}

/// Downloads the model into `dir`, checks it and unpacks it; returns its folder. `progress` gets
/// (bytes so far, total) as the download goes; setting `cancel` stops it and leaves nothing behind.
/// Blocks: call it from a thread of its own.
pub fn install(
    dir: &Path,
    model: &ModelInfo,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, Error> {
    download::install(
        dir,
        &model.archive(),
        usable(model.engine),
        progress,
        cancel,
    )
}

/// How to tell that a folder holds a model of `engine`.
fn usable(engine: Engine) -> fn(&Path) -> bool {
    match engine {
        Engine::Vosk => is_vosk_model,
        Engine::Sherpa => is_model,
    }
}

/// Whether `dir` holds a speech model sherpa-onnx can load (its token list and model files).
fn is_model(dir: &Path) -> bool {
    dir.is_dir() && crate::sherpa::ModelSpec::detect(dir, "", 1).is_ok()
}

/// Whether `dir` holds a Vosk model: its acoustic model and its configuration.
fn is_vosk_model(dir: &Path) -> bool {
    dir.join("am").join("final.mdl").is_file() && dir.join("conf").join("model.conf").is_file()
}

#[cfg(test)]
mod tests;
