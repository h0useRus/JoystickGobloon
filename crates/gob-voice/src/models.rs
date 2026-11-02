//! Speech models per language: Vosk's small models, downloaded once from alphacephei.com into a
//! folder the caller chooses (the program's data folder). Every download is checked against the
//! SHA-256 pinned here before it is unpacked, and a model counts as installed only when its files
//! are there, so a broken or half-done download is never used.

use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::Error;

const BASE_URL: &str = "https://alphacephei.com/vosk/models/";

/// A speech model to download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelInfo {
    /// The program's language code (`lang/*.ftl`) the model recognizes.
    pub language: &'static str,
    /// The name of the zip (without `.zip`) and of the folder it unpacks to.
    pub name: &'static str,
    /// Download size in bytes.
    pub size: u64,
    pub sha256: &'static str,
}

impl ModelInfo {
    pub fn url(&self) -> String {
        format!("{BASE_URL}{}.zip", self.name)
    }
}

/// The models, one per language of the program (checked 2026-11-01: each zip holds
/// `<name>/am/final.mdl` and `<name>/conf/model.conf`).
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        language: "en",
        name: "vosk-model-small-en-us-0.15",
        size: 41_205_931,
        sha256: "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498",
    },
    ModelInfo {
        language: "de",
        name: "vosk-model-small-de-0.15",
        size: 46_499_967,
        sha256: "b7e53c90b1f0a38456f4cd62b366ecd58803cd97cd42b06438e2c131713d5e43",
    },
    ModelInfo {
        language: "es",
        name: "vosk-model-small-es-0.42",
        size: 39_817_833,
        sha256: "09b239888f633ef2f0b4e09736e3d9936acfd810bc65d53fad45261762c6511f",
    },
    ModelInfo {
        language: "fr",
        name: "vosk-model-small-fr-0.22",
        size: 42_233_323,
        sha256: "cabf6180e177eb9b3a9a9d43a437bd5e549f3a7d09525e5d69a3fed787be12ad",
    },
    ModelInfo {
        language: "ko",
        name: "vosk-model-small-ko-0.22",
        size: 86_914_329,
        sha256: "eea36124087fed26c59996a4761519458e3bd185e8ea9d9865ad8760c4a1d989",
    },
    ModelInfo {
        language: "ru",
        name: "vosk-model-small-ru-0.22",
        size: 46_236_750,
        sha256: "961d5ff98a17f4aa6de69864d0aa71fa5bac682301d2b5d17a3f24c5c99a46d4",
    },
    ModelInfo {
        language: "zh",
        name: "vosk-model-small-cn-0.22",
        size: 43_898_754,
        sha256: "3af8b0e7e0f835ae9d414ce5df580237a3cfb08d586c9fbbb0f7ff29ad5b14ba",
    },
];

/// The model for a language code (`en`, `de`, ...; a region like `en-GB` is ignored).
pub fn for_language(language: &str) -> Option<&'static ModelInfo> {
    let base = language.split(['-', '_']).next().unwrap_or(language);
    MODELS
        .iter()
        .find(|m| m.language.eq_ignore_ascii_case(base))
}

/// The model's folder in `dir` when it is installed there.
pub fn installed(dir: &Path, model: &ModelInfo) -> Option<PathBuf> {
    let path = dir.join(model.name);
    is_model(&path).then_some(path)
}

/// Deletes the model's folder in `dir` (nothing when it is not there).
pub fn remove(dir: &Path, model: &ModelInfo) -> io::Result<()> {
    match fs::remove_dir_all(dir.join(model.name)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
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
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        // The whole body: generous, so slow lines still get there, but a stalled one ends.
        .timeout_recv_body(Some(Duration::from_secs(30 * 60)))
        .build()
        .new_agent();
    let url = model.url();
    let mut response = agent
        .get(&url)
        .call()
        .map_err(|e| Error::Download(format!("{url}: {e}")))?;
    let total = response.body().content_length().unwrap_or(model.size);
    let reader = response
        .body_mut()
        .with_config()
        .limit(model.size + model.size / 2)
        .reader();
    install_from(reader, total, dir, model, progress, cancel)
}

/// [`install`] from any source of the zip's bytes.
fn install_from(
    mut source: impl Read,
    total: u64,
    dir: &Path,
    model: &ModelInfo,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, Error> {
    let io_error = |e: io::Error| Error::Download(e.to_string());
    fs::create_dir_all(dir).map_err(io_error)?;
    let zip = dir.join(format!("{}.zip.part", model.name));
    let unpacked = dir.join(format!("{}.part", model.name));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&zip)
            .map_err(io_error)?;
        let mut hash = Sha256::new();
        let mut buf = vec![0; 64 * 1024];
        let mut done = 0u64;
        progress(0, total);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let n = match source.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_error(e)),
            };
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(io_error)?;
            done += n as u64;
            progress(done, total.max(done));
        }
        let got = hex(&hash.finalize());
        if !got.eq_ignore_ascii_case(model.sha256) {
            return Err(Error::Download(format!(
                "{}: SHA-256 {got}, expected {}",
                model.name, model.sha256
            )));
        }
        file.rewind().map_err(io_error)?;
        let _ = fs::remove_dir_all(&unpacked);
        unpack(file, &unpacked)?;
        // The zip holds the model in a folder of its own name; take that, or the whole zip.
        let inner = unpacked.join(model.name);
        let root = if inner.is_dir() {
            inner
        } else {
            unpacked.clone()
        };
        if !is_model(&root) {
            return Err(Error::Model(format!(
                "{}: no speech model in the zip",
                model.name
            )));
        }
        let path = dir.join(model.name);
        let _ = fs::remove_dir_all(&path);
        fs::rename(&root, &path).map_err(io_error)?;
        Ok(path)
    })();
    let _ = fs::remove_file(&zip);
    let _ = fs::remove_dir_all(&unpacked);
    match &result {
        Ok(path) => log::info!("speech model installed: {}", path.display()),
        Err(e) => log::warn!("{}: {e}", model.name),
    }
    result
}

/// Unpacks a zip into `to`, refusing entries that would land outside it.
fn unpack(file: impl Read + Seek, to: &Path) -> Result<(), Error> {
    let bad = |e: zip::result::ZipError| Error::Download(format!("zip: {e}"));
    let io_error = |e: io::Error| Error::Download(e.to_string());
    let mut archive = zip::ZipArchive::new(file).map_err(bad)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(bad)?;
        let Some(name) = entry.enclosed_name() else {
            return Err(Error::Download(format!(
                "zip: unsafe path {}",
                String::from_utf8_lossy(entry.name_raw())
            )));
        };
        let path = to.join(name);
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(io_error)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        let mut out = fs::File::create(&path).map_err(io_error)?;
        io::copy(&mut entry, &mut out).map_err(io_error)?;
    }
    Ok(())
}

/// Whether `dir` holds a Vosk model: its acoustic model and its configuration.
fn is_model(dir: &Path) -> bool {
    dir.join("am").join("final.mdl").is_file() && dir.join("conf").join("model.conf").is_file()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use zip::write::SimpleFileOptions;

    use super::*;

    /// A zip with the given (path, content) entries.
    fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, content) in entries {
            zip.start_file(*path, SimpleFileOptions::default()).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn fake_model(bytes: &[u8]) -> ModelInfo {
        let hash = hex(&Sha256::digest(bytes));
        ModelInfo {
            language: "xx",
            name: "test-model",
            size: bytes.len() as u64,
            sha256: Box::leak(hash.into_boxed_str()),
        }
    }

    fn temp_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gob-voice-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    const MODEL_FILES: &[(&str, &str)] = &[
        ("test-model/am/final.mdl", "acoustic"),
        ("test-model/conf/model.conf", "--sample-frequency=16000"),
        ("test-model/graph/words.txt", "hello 1"),
    ];

    fn run(
        bytes: &[u8],
        model: &ModelInfo,
        dir: &Path,
        cancel: bool,
    ) -> (Result<PathBuf, Error>, Vec<(u64, u64)>) {
        let mut steps = Vec::new();
        let result = install_from(
            Cursor::new(bytes),
            bytes.len() as u64,
            dir,
            model,
            &mut |done, total| steps.push((done, total)),
            &AtomicBool::new(cancel),
        );
        (result, steps)
    }

    /// Only the model's folder is left: no zip, no half-unpacked folder.
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
    fn installs_a_checked_model() {
        let dir = temp_dir("install");
        let bytes = zip_of(MODEL_FILES);
        let model = fake_model(&bytes);
        assert_eq!(installed(&dir, &model), None);
        let (result, steps) = run(&bytes, &model, &dir, false);
        let path = result.unwrap();
        assert_eq!(path, dir.join("test-model"));
        assert_eq!(installed(&dir, &model), Some(path.clone()));
        assert_eq!(
            fs::read_to_string(path.join("graph/words.txt")).unwrap(),
            "hello 1"
        );
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
    fn a_zip_without_the_model_folder_works_too() {
        let dir = temp_dir("flat");
        let entries: Vec<_> = MODEL_FILES
            .iter()
            .map(|(p, c)| (p.trim_start_matches("test-model/"), *c))
            .collect();
        let bytes = zip_of(&entries);
        let model = fake_model(&bytes);
        assert!(run(&bytes, &model, &dir, false).0.is_ok());
        assert!(installed(&dir, &model).is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_hash_is_refused() {
        let dir = temp_dir("hash");
        let bytes = zip_of(MODEL_FILES);
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
        let bytes = zip_of(MODEL_FILES);
        let model = fake_model(&bytes);
        let (result, _) = run(&bytes, &model, &dir, true);
        assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
        assert!(left_in(&dir).is_empty(), "{:?}", left_in(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_zip_without_a_model_is_refused() {
        let dir = temp_dir("empty");
        let bytes = zip_of(&[("test-model/README", "not a model")]);
        let model = fake_model(&bytes);
        let (result, _) = run(&bytes, &model, &dir, false);
        assert!(matches!(result, Err(Error::Model(_))), "{result:?}");
        assert!(left_in(&dir).is_empty(), "{:?}", left_in(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn paths_out_of_the_folder_are_refused() {
        let dir = temp_dir("slip");
        let mut entries = MODEL_FILES.to_vec();
        entries.push(("../escaped.txt", "outside"));
        let bytes = zip_of(&entries);
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
    fn a_model_for_every_language() {
        assert_eq!(
            for_language("en").unwrap().name,
            "vosk-model-small-en-us-0.15"
        );
        assert_eq!(for_language("en-GB"), for_language("en"));
        assert_eq!(for_language("zh").unwrap().name, "vosk-model-small-cn-0.22");
        assert_eq!(for_language("xx"), None);
        // The program's languages (lang/*.ftl).
        for lang in ["en", "de", "es", "fr", "ko", "ru", "zh"] {
            assert!(for_language(lang).is_some(), "{lang}");
        }
        for m in MODELS {
            assert_eq!(m.sha256.len(), 64, "{}", m.name);
            assert!(
                m.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "{}",
                m.name
            );
            assert_eq!(for_language(m.language), Some(m), "one model per language");
        }
    }

    /// Network: downloads the English model (about 40 MB) and installs it.
    #[test]
    #[ignore]
    fn downloads_the_english_model() {
        let dir = temp_dir("download");
        let model = for_language("en").unwrap();
        let mut last = (0, 0);
        let path = install(
            &dir,
            model,
            &mut |done, total| last = (done, total),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(last, (model.size, model.size));
        assert_eq!(installed(&dir, model), Some(path));
        let _ = fs::remove_dir_all(&dir);
    }
}
