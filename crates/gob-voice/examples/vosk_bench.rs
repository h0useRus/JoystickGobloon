//! Scores a Vosk model in grammar mode (how voice commands worked up to 0.15) on the same test
//! files as `speech_bench --phrases`, for comparison with the sherpa-onnx models. Live, side by
//! side with a sherpa-onnx model: `speech_bench --listen ... --vosk-lib <dir> --vosk-model <dir>`.
//!
//! vosk_bench --lib <dir with libvosk> --model <vosk model dir> --phrases <dir> [--gain <percent>]
//!     <dir> as for speech_bench: phrases.txt, other.txt, phrases-NN-*.wav, other-NN-*.wav
//!     (16 kHz mono 16-bit). The grammar is every line of phrases.txt plus [unk], like the app
//!     had; a result counts when its words' lowest confidence is at least 0.5.

use std::path::{Path, PathBuf};

mod common {
    pub mod vosk;
}

const MIN_CONFIDENCE: f32 = 0.5;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut lib, mut model, mut phrases, mut gain) = (None, None, None, 1.0f32);
    while let Some(a) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| fail(format!("{a} needs a value")))
        };
        match a.as_str() {
            "--lib" => lib = Some(PathBuf::from(value())),
            "--model" => model = Some(PathBuf::from(value())),
            "--phrases" => phrases = Some(PathBuf::from(value())),
            "--gain" => gain = value().parse::<f32>().unwrap_or_else(|e| fail(e)) / 100.0,
            _ => fail(format!("unknown argument {a}")),
        }
    }
    let (Some(lib), Some(model), Some(dir)) = (lib, model, phrases) else {
        fail("--lib, --model and --phrases are needed");
    };
    let list = lines(&dir.join("phrases.txt"));
    let other = lines(&dir.join("other.txt"));
    let vosk = common::vosk::Vosk::load(&lib, &model, &list).unwrap_or_else(|e| fail(e));

    let (mut right, mut total, mut rejected, mut others) = (0, 0, 0, 0);
    for file in wav_files(&dir) {
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        let line: Option<usize> = name.split('-').nth(1).and_then(|n| n.parse().ok());
        let Some(samples) = read_wav(&file) else {
            continue;
        };
        let samples: Vec<i16> = samples
            .iter()
            .map(|&s| (f32::from(s) * gain).clamp(-32768.0, 32767.0) as i16)
            .collect();
        let (text, confidence) = vosk.hear(&samples);
        let found = vosk.phrase(&text, confidence, MIN_CONFIDENCE);
        let shown = found.map_or("-".to_string(), |i| {
            format!("{} ({confidence:.2})", list[i])
        });
        if name.starts_with("phrases-") {
            total += 1;
            let ok = found == line;
            right += usize::from(ok);
            let said = line.and_then(|l| list.get(l)).map_or("?", |s| s.as_str());
            println!(
                "{} {name}: heard {text:?} -> {shown} (said {said:?})",
                if ok { "ok  " } else { "MISS" }
            );
        } else {
            others += 1;
            let ok = found.is_none();
            rejected += usize::from(ok);
            let said = line.and_then(|l| other.get(l)).map_or("?", |s| s.as_str());
            println!(
                "{} {name}: heard {text:?} -> {shown} (said {said:?})",
                if ok { "ok  " } else { "FIRE" }
            );
        }
    }
    println!(
        "SUMMARY {} (Vosk grammar) | gain {:.0} % | phrases {right}/{total} | other speech rejected {rejected}/{others}",
        model.file_name().unwrap().to_string_lossy(),
        gain * 100.0
    );
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| fail(format!("{}: {e}", path.display())))
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}').to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn wav_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| fail(format!("{}: {e}", dir.display())))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")))
        .collect();
    files.sort();
    files
}

/// The samples of a 16 kHz mono 16-bit PCM WAV file; `None` (with a note) for others.
fn read_wav(path: &Path) -> Option<Vec<i16>> {
    let data = std::fs::read(path).ok()?;
    let mut pos = 12;
    let (mut format, mut samples) = (None, None);
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body = &data[pos + 8..(pos + 8 + len).min(data.len())];
        match id {
            b"fmt " if body.len() >= 16 => {
                format = Some((
                    u16::from_le_bytes([body[2], body[3]]),
                    u32::from_le_bytes(body[4..8].try_into().ok()?),
                    u16::from_le_bytes([body[14], body[15]]),
                ))
            }
            b"data" => {
                samples = Some(
                    body.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| i16::from_le_bytes(*b))
                        .collect::<Vec<_>>(),
                )
            }
            _ => {}
        }
        pos += 8 + len + (len & 1);
    }
    match (format, samples) {
        (Some((1, 16_000, 16)), Some(s)) => Some(s),
        (f, _) => {
            println!("skip {}: not 16 kHz mono 16-bit PCM {f:?}", path.display());
            None
        }
    }
}

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}
