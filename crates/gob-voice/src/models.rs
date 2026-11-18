//! Speech models per language: sherpa-onnx models from its GitHub releases, a small and (for most
//! languages) a large one, downloaded once into a folder the caller chooses (the program's data
//! folder). Every download is checked against the SHA-256 pinned here before it is unpacked, and
//! a model counts as installed only when its files are there, so a broken or half-done download
//! is never used. German, Spanish and French share one multilingual model.
//!
//! The memory figures are what the model needed on a test PC while decoding (the whole process,
//! plans/SPEECH_PLAN.md "S0 results"): the program shows them so the user can choose.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::Error;
use crate::download::{self, Archive};

const BASE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/";

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
            url: format!("{BASE_URL}{}.tar.bz2", self.name),
            bytes: self.bytes,
            sha256: self.sha256,
        }
    }
}

const EUROPEAN: &[&str] = &["de", "es", "fr"];

/// The models (checked 2026-11-18: sizes and SHA-256 of the release assets; each archive holds
/// its files in a folder of its own name).
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        languages: &["en"],
        size: Size::Small,
        name: "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27",
        bytes: 29_858_559,
        sha256: "9ec31b342d8fa3240c3b81b8f82e1cf7e3ac467c93ca5a999b741d5887164f8d",
        memory_mb: 140,
    },
    ModelInfo {
        languages: &["en"],
        size: Size::Large,
        name: "sherpa-onnx-nemo-parakeet_tdt_transducer_110m-en-36000-int8",
        bytes: 108_035_095,
        sha256: "f628312e9fdf8686374cb01a69425c41732529d540860311f16f37cbc32cfe9b",
        memory_mb: 340,
    },
    ModelInfo {
        languages: &["ru"],
        size: Size::Small,
        name: "sherpa-onnx-zipformer-ru-int8-2025-04-20",
        bytes: 60_239_942,
        sha256: "d6a651569aacc9a177259fa54705dd76acae23f6a4d62ea6797bd220d4b57163",
        memory_mb: 260,
    },
    ModelInfo {
        languages: &["ru"],
        size: Size::Large,
        name: "sherpa-onnx-nemo-transducer-giga-am-v3-russian-2025-12-16",
        bytes: 167_388_020,
        sha256: "20a439491904b839f35eb8efaa3d99cbfaaad0c6dcba22d06ff218bb0056772d",
        memory_mb: 500,
    },
    ModelInfo {
        languages: EUROPEAN,
        size: Size::Small,
        name: "sherpa-onnx-nemo-fast-conformer-transducer-be-de-en-es-fr-hr-it-pl-ru-uk-20k-int8",
        bytes: 106_546_673,
        sha256: "06072bad277f0f4c29cc866d7c62b0e47936da39afafeae453faa925025ccad6",
        memory_mb: 340,
    },
    ModelInfo {
        languages: EUROPEAN,
        size: Size::Large,
        name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
        bytes: 487_211_443,
        sha256: "1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9",
        memory_mb: 1370,
    },
    ModelInfo {
        languages: &["zh"],
        size: Size::Small,
        name: "sherpa-onnx-moonshine-base-zh-quantized-2026-02-27",
        bytes: 99_905_420,
        sha256: "6495d4240f66bcdd2bbbbfefaa687c463150d467854001270934e88940b733c1",
        memory_mb: 345,
    },
    ModelInfo {
        languages: &["zh"],
        size: Size::Large,
        name: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
        bytes: 165_783_878,
        sha256: "7305f7905bfcf77fa0b39388a313f3da35c68d971661a65475b56fb2162c8e63",
        memory_mb: 410,
    },
    ModelInfo {
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

/// The languages that have a model, each once, in [`MODELS`] order.
pub fn languages() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for m in MODELS {
        for l in m.languages {
            if !out.contains(l) {
                out.push(l);
            }
        }
    }
    out
}

/// The models of a language code (`en`, `de`...; a region like `en-GB` is ignored), small first.
pub fn for_language(language: &str) -> Vec<&'static ModelInfo> {
    let base = base(language);
    let mut found: Vec<&ModelInfo> = MODELS
        .iter()
        .filter(|m| m.languages.iter().any(|l| l.eq_ignore_ascii_case(base)))
        .collect();
    found.sort_by_key(|m| m.size == Size::Large);
    found
}

/// The model of that size for the language; the small one when it has no other.
pub fn pick(language: &str, size: Size) -> Option<&'static ModelInfo> {
    let models = for_language(language);
    models
        .iter()
        .find(|m| m.size == size)
        .or(models.first())
        .copied()
}

/// The model's folder in `dir` when it is installed there.
pub fn installed(dir: &Path, model: &ModelInfo) -> Option<PathBuf> {
    let path = dir.join(model.name);
    is_model(&path).then_some(path)
}

/// Deletes the model's folder in `dir` (nothing when it is not there).
pub fn remove(dir: &Path, model: &ModelInfo) -> io::Result<()> {
    download::remove(dir, model.name)
}

/// Deletes the speech models of the program's earlier speech engine (Vosk: `vosk-model-*`
/// folders) from `dir`; they are of no use any more.
pub fn remove_old(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("vosk-model-") && e.path().is_dir() {
            match fs::remove_dir_all(e.path()) {
                Ok(()) => log::info!("removed the old speech model {name}"),
                Err(err) => log::warn!("removing the old speech model {name}: {err}"),
            }
        }
    }
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
    download::install(dir, &model.archive(), is_model, progress, cancel)
}

/// Whether `dir` holds a speech model sherpa-onnx can load (its token list and model files).
fn is_model(dir: &Path) -> bool {
    dir.is_dir() && crate::sherpa::ModelSpec::detect(dir, "", 1).is_ok()
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::download::hex;

    /// A .tar.bz2 with the given (path, content) entries.
    fn archive_of(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for (path, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, content.as_bytes())
                .unwrap();
        }
        let tar = tar.into_inner().unwrap();
        let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        bz.write_all(&tar).unwrap();
        bz.finish().unwrap()
    }

    /// A tar entry whose name leaves the folder (`Builder` refuses to write one).
    fn archive_with_escape() -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for (path, content) in MODEL_FILES {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_cksum();
            tar.append_data(&mut header, path, content.as_bytes())
                .unwrap();
        }
        let mut header = tar::Header::new_old();
        let name = b"../escaped.txt";
        header.as_old_mut().name[..name.len()].copy_from_slice(name);
        header.set_size(7);
        header.set_cksum();
        tar.append(&header, &b"outside"[..]).unwrap();
        let tar = tar.into_inner().unwrap();
        let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        bz.write_all(&tar).unwrap();
        bz.finish().unwrap()
    }

    fn fake_model(bytes: &[u8]) -> ModelInfo {
        let hash = hex(&Sha256::digest(bytes));
        ModelInfo {
            languages: &["xx"],
            size: Size::Small,
            name: "test-model",
            bytes: bytes.len() as u64,
            sha256: Box::leak(hash.into_boxed_str()),
            memory_mb: 1,
        }
    }

    fn temp_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gob-voice-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A transducer as sherpa-onnx's releases lay it out.
    const MODEL_FILES: &[(&str, &str)] = &[
        ("test-model/encoder.int8.onnx", "e"),
        ("test-model/decoder.int8.onnx", "d"),
        ("test-model/joiner.int8.onnx", "j"),
        ("test-model/tokens.txt", "<blk> 0"),
        ("test-model/test_wavs/0.wav", "RIFF"),
    ];

    fn run(
        bytes: &[u8],
        model: &ModelInfo,
        dir: &Path,
        cancel: bool,
    ) -> (Result<PathBuf, Error>, Vec<(u64, u64)>) {
        let mut steps = Vec::new();
        let result = download::install_from(
            Cursor::new(bytes),
            bytes.len() as u64,
            dir,
            &model.archive(),
            is_model,
            &mut |done, total| steps.push((done, total)),
            &AtomicBool::new(cancel),
        );
        (result, steps)
    }

    /// What is left in `dir`: only the model's folder, no archive, no half-unpacked folder.
    fn left_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn installs_a_checked_model_without_its_samples() {
        let dir = temp_dir("install");
        let bytes = archive_of(MODEL_FILES);
        let model = fake_model(&bytes);
        assert_eq!(installed(&dir, &model), None);
        let (result, steps) = run(&bytes, &model, &dir, false);
        let path = result.unwrap();
        assert_eq!(path, dir.join("test-model"));
        assert_eq!(installed(&dir, &model), Some(path.clone()));
        assert_eq!(
            fs::read_to_string(path.join("tokens.txt")).unwrap(),
            "<blk> 0"
        );
        assert!(!path.join("test_wavs").exists(), "samples left out");
        assert_eq!(steps.first(), Some(&(0, bytes.len() as u64)));
        assert_eq!(
            steps.last(),
            Some(&(bytes.len() as u64, bytes.len() as u64))
        );
        assert_eq!(left_in(&dir), ["test-model"]);
        // Installing again replaces it.
        run(&bytes, &model, &dir, false).0.unwrap();
        remove(&dir, &model).unwrap();
        assert_eq!(installed(&dir, &model), None);
        remove(&dir, &model).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_archive_without_the_model_folder_works_too() {
        let dir = temp_dir("flat");
        let entries: Vec<_> = MODEL_FILES
            .iter()
            .map(|(p, c)| (p.trim_start_matches("test-model/"), *c))
            .collect();
        let bytes = archive_of(&entries);
        let model = fake_model(&bytes);
        assert!(run(&bytes, &model, &dir, false).0.is_ok());
        assert!(installed(&dir, &model).is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_hash_is_refused() {
        let dir = temp_dir("hash");
        let bytes = archive_of(MODEL_FILES);
        let model = ModelInfo {
            sha256: "00",
            ..fake_model(&bytes)
        };
        let (result, _) = run(&bytes, &model, &dir, false);
        assert!(
            matches!(&result, Err(Error::Download(e)) if e.contains("SHA-256")),
            "{result:?}"
        );
        assert_eq!(installed(&dir, &model), None);
        assert!(left_in(&dir).is_empty(), "{:?}", left_in(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancelling_leaves_nothing() {
        let dir = temp_dir("cancel");
        let bytes = archive_of(MODEL_FILES);
        let model = fake_model(&bytes);
        let (result, _) = run(&bytes, &model, &dir, true);
        assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
        assert!(left_in(&dir).is_empty(), "{:?}", left_in(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_archive_without_a_model_is_refused() {
        let dir = temp_dir("empty");
        let bytes = archive_of(&[("test-model/README", "not a model")]);
        let model = fake_model(&bytes);
        let (result, _) = run(&bytes, &model, &dir, false);
        assert!(matches!(result, Err(Error::Model(_))), "{result:?}");
        assert!(left_in(&dir).is_empty(), "{:?}", left_in(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn paths_out_of_the_folder_are_refused() {
        let dir = temp_dir("slip");
        let bytes = archive_with_escape();
        let model = fake_model(&bytes);
        let (result, _) = run(&bytes, &model, &dir, false);
        assert!(
            matches!(&result, Err(Error::Download(e)) if e.contains("unsafe")),
            "{result:?}"
        );
        assert!(!dir.parent().unwrap().join("escaped.txt").exists());
        assert!(left_in(&dir).is_empty(), "{:?}", left_in(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_vosk_models_are_removed() {
        let dir = temp_dir("old");
        fs::create_dir_all(dir.join("vosk-model-small-en-us-0.15/am")).unwrap();
        fs::create_dir_all(dir.join("sherpa-onnx-keep")).unwrap();
        remove_old(&dir);
        assert_eq!(left_in(&dir), ["sherpa-onnx-keep"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn models_for_every_language() {
        assert_eq!(
            pick("en", Size::Small).unwrap().name,
            "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27"
        );
        assert_eq!(pick("en-GB", Size::Large), pick("en", Size::Large));
        assert_eq!(pick("de", Size::Small), pick("fr", Size::Small), "shared");
        // Korean has only a small model: large falls back to it.
        assert_eq!(pick("ko", Size::Large), pick("ko", Size::Small));
        assert_eq!(pick("xx", Size::Small), None);
        // The program's languages (lang/*.ftl).
        for lang in ["en", "de", "es", "fr", "ko", "ru", "zh"] {
            assert!(pick(lang, Size::Small).is_some(), "{lang}");
            assert!(languages().contains(&lang), "{lang}");
        }
        for m in MODELS {
            assert_eq!(m.sha256.len(), 64, "{}", m.name);
            assert!(
                m.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "{}",
                m.name
            );
            for l in m.languages {
                assert_eq!(
                    for_language(l).iter().filter(|o| o.size == m.size).count(),
                    1,
                    "one {:?} model for {l}",
                    m.size
                );
            }
        }
        assert_eq!(Size::parse(Size::Large.key()), Size::Large);
        assert_eq!(Size::parse("anything"), Size::Small);
    }

    /// Network: downloads the small English model (about 30 MB) and installs it.
    #[test]
    #[ignore]
    fn downloads_the_small_english_model() {
        let dir = temp_dir("download");
        let model = pick("en", Size::Small).unwrap();
        let mut last = (0, 0);
        let path = install(
            &dir,
            model,
            &mut |done, total| last = (done, total),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(last, (model.bytes, model.bytes));
        assert_eq!(installed(&dir, model), Some(path));
        let _ = fs::remove_dir_all(&dir);
    }
}
