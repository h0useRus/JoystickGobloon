use std::fs;
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
        engine: Engine::Sherpa,
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
        usable(model.engine),
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

/// A .zip with the given (path, content) entries.
fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, content) in entries {
        zip.start_file(*path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Vosk's models are zips holding `am/` and `conf/` in a folder of their own name.
#[test]
fn installs_a_vosk_zip() {
    let dir = temp_dir("vosk");
    let files = [
        ("test-model/am/final.mdl", "am"),
        ("test-model/conf/model.conf", "conf"),
        ("test-model/graph/words.txt", "gear"),
    ];
    let bytes = zip_of(&files);
    let model = ModelInfo {
        engine: Engine::Vosk,
        ..fake_model(&bytes)
    };
    assert!(model.archive().url.ends_with("test-model.zip"));
    let path = run(&bytes, &model, &dir, false).0.unwrap();
    assert_eq!(installed(&dir, &model), Some(path.clone()));
    assert_eq!(
        fs::read_to_string(path.join("conf/model.conf")).unwrap(),
        "conf"
    );
    assert_eq!(left_in(&dir), ["test-model"]);
    // A sherpa-onnx model is not a Vosk one, and the other way round.
    let sherpa = ModelInfo {
        engine: Engine::Sherpa,
        ..model
    };
    assert_eq!(installed(&dir, &sherpa), None);
    let _ = fs::remove_dir_all(&dir);

    // A zip without the model's files, or with a path out of the folder, is refused.
    let dir = temp_dir("vosk-bad");
    let bytes = zip_of(&[("test-model/README", "no model")]);
    let bad = ModelInfo {
        engine: Engine::Vosk,
        ..fake_model(&bytes)
    };
    assert!(matches!(
        run(&bytes, &bad, &dir, false).0,
        Err(Error::Model(_))
    ));
    let bytes = zip_of(&[("../escaped.txt", "outside")]);
    let bad = ModelInfo {
        engine: Engine::Vosk,
        ..fake_model(&bytes)
    };
    let result = run(&bytes, &bad, &dir, false).0;
    assert!(
        matches!(&result, Err(Error::Download(e)) if e.contains("unsafe")),
        "{result:?}"
    );
    assert!(left_in(&dir).is_empty(), "{:?}", left_in(&dir));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn models_for_every_language() {
    use Engine::{Sherpa, Vosk};
    assert_eq!(
        pick(Sherpa, "en", Size::Small).unwrap().name,
        "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27"
    );
    assert_eq!(
        pick(Vosk, "en", Size::Small).unwrap().name,
        "vosk-model-small-en-us-0.15"
    );
    // Vosk has only small models: large falls back to them.
    assert_eq!(pick(Vosk, "en", Size::Large), pick(Vosk, "en", Size::Small));
    assert_eq!(
        pick(Sherpa, "en-GB", Size::Large),
        pick(Sherpa, "en", Size::Large)
    );
    assert_eq!(
        pick(Sherpa, "de", Size::Small),
        pick(Sherpa, "fr", Size::Small),
        "shared"
    );
    assert_ne!(pick(Vosk, "de", Size::Small), pick(Vosk, "fr", Size::Small));
    // Korean has only a small sherpa-onnx model: large falls back to it.
    assert_eq!(
        pick(Sherpa, "ko", Size::Large),
        pick(Sherpa, "ko", Size::Small)
    );
    assert_eq!(pick(Sherpa, "xx", Size::Small), None);
    // The program's languages (lang/*.ftl), on both engines.
    for engine in Engine::ALL {
        for lang in ["en", "de", "es", "fr", "ko", "ru", "zh"] {
            let m = pick(engine, lang, Size::Small);
            assert!(m.is_some_and(|m| m.engine == engine), "{engine:?} {lang}");
            assert!(languages(engine).contains(&lang), "{engine:?} {lang}");
        }
        assert_eq!(Engine::parse(engine.key()), engine);
    }
    assert_eq!(Engine::parse("anything"), Engine::Vosk);
    for m in MODELS {
        assert_eq!(m.sha256.len(), 64, "{}", m.name);
        assert!(
            m.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "{}",
            m.name
        );
        for l in m.languages {
            assert_eq!(
                for_language(m.engine, l)
                    .iter()
                    .filter(|o| o.size == m.size)
                    .count(),
                1,
                "one {:?} {:?} model for {l}",
                m.engine,
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
    let model = pick(Engine::Sherpa, "en", Size::Small).unwrap();
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
