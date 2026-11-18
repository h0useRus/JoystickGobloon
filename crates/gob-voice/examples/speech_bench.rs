//! Measures a sherpa-onnx speech model the way voice commands use it: load time, memory, decode
//! time per utterance (real-time factor) and how often the phrase matcher picks the right phrase.
//!
//! speech_bench --lib <dir> --model <dir> [--lang <code>] [--threads <n>] --phrases <dir>
//!     <dir> holds phrases.txt and other.txt (one per line) and 16 kHz mono 16-bit WAV files named
//!     phrases-NN-*.wav / other-NN-*.wav (NN = line, from 00): each phrase file should match its
//!     line, each other file (speech that is no command) should match nothing.
//! speech_bench --lib <dir> --model <dir> [--lang <code>] [--threads <n>] --wavs <dir>
//!     prints what it hears in every WAV file of <dir> (e.g. the model's test_wavs).
//! speech_bench --lib <dir> --model <dir> [--lang <code>] [--threads <n>] --listen <phrases.txt> [--gain <percent>]
//!     push to talk on the default microphone, kept open like the app does: Enter starts a
//!     phrase, Enter again ends it; prints what was heard and the phrase it matches. q quits.
//!
//! Prints one summary line at the end: load s | memory MB after load / peak | mean RTF | right
//! phrases | rejected other speech. Memory is the whole process (working set).

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use gob_voice::sherpa::{ModelSpec, Sherpa};
use gob_voice::{Capture, SAMPLE_RATE, matcher};

/// Below this confidence a phrase counts as not recognized (the app's default).
const MIN_CONFIDENCE: f32 = 0.5;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut lib, mut model, mut phrases, mut wavs, mut listen) = (None, None, None, None, None);
    let (mut lang, mut threads, mut gain) = (String::new(), 1, 1.0);
    while let Some(a) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| fail(format!("{a} needs a value")))
        };
        match a.as_str() {
            "--lib" => lib = Some(PathBuf::from(value())),
            "--model" => model = Some(PathBuf::from(value())),
            "--lang" => lang = value(),
            "--threads" => threads = value().parse().unwrap_or_else(|e| fail(e)),
            "--phrases" => phrases = Some(PathBuf::from(value())),
            "--wavs" => wavs = Some(PathBuf::from(value())),
            "--listen" => listen = Some(PathBuf::from(value())),
            "--gain" => gain = value().parse::<f32>().unwrap_or_else(|e| fail(e)) / 100.0,
            _ => fail(format!("unknown argument {a}")),
        }
    }
    let (Some(lib), Some(model)) = (lib, model) else {
        fail("--lib and --model are needed");
    };
    let base = memory();
    let sherpa = Sherpa::load(&[lib]).unwrap_or_else(|e| fail(e));
    let spec = ModelSpec::detect(&model, &lang, threads).unwrap_or_else(|e| fail(e));
    let started = Instant::now();
    let loaded = sherpa.model(&spec).unwrap_or_else(|e| fail(e));
    let load = started.elapsed().as_secs_f32();
    let after_load = memory();
    println!(
        "sherpa-onnx {}, {:?}, loaded in {load:.2} s",
        sherpa.version(),
        spec.kind
    );

    if let Some(list) = listen {
        talk(&loaded, &lines(&list), gain);
        return;
    }

    let mut rtf = Vec::new();
    let mut hear = |file: &Path| -> Option<String> {
        let samples = read_wav(file)?;
        let mut r = loaded.recognizer();
        r.feed(&samples);
        let t = Instant::now();
        let text = r.finish();
        let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
        rtf.push(t.elapsed().as_secs_f32() / seconds);
        Some(text)
    };

    let (mut right, mut total, mut rejected, mut others) = (0, 0, 0, 0);
    if let Some(dir) = &phrases {
        let list = lines(&dir.join("phrases.txt"));
        let other = lines(&dir.join("other.txt"));
        for file in wav_files(dir) {
            let name = file.file_name().unwrap().to_string_lossy().to_string();
            let line: Option<usize> = name.split('-').nth(1).and_then(|n| n.parse().ok());
            let Some(text) = hear(&file) else { continue };
            let found =
                matcher::best(&text, &commands(&list)).filter(|&(_, c)| c >= MIN_CONFIDENCE);
            let shown = found.map_or("-".to_string(), |(i, c)| format!("{} ({c:.2})", list[i]));
            if name.starts_with("phrases-") {
                total += 1;
                let ok = found.map(|f| f.0) == line;
                right += usize::from(ok);
                let expected = line.and_then(|l| list.get(l)).map_or("?", |s| s.as_str());
                println!(
                    "{} {name}: heard {text:?} -> {shown} (said {expected:?})",
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
    }
    if let Some(dir) = &wavs {
        for file in wav_files(dir) {
            if let Some(text) = hear(&file) {
                println!("{}: {text:?}", file.file_name().unwrap().to_string_lossy());
            }
        }
    }
    let peak = memory();
    let mean = rtf.iter().sum::<f32>() / rtf.len().max(1) as f32;
    println!(
        "SUMMARY {} | load {load:.2} s | memory {:.0} MB after load (+{:.0} for the model) / peak {:.0} MB | RTF {mean:.3} ({} files, {threads} thread(s)) | phrases {right}/{total} | other speech rejected {rejected}/{others}",
        model.file_name().unwrap().to_string_lossy(),
        after_load.0,
        after_load.0 - base.0,
        peak.1,
        rtf.len()
    );
}

/// Push to talk with Enter on the default microphone, which stays open between phrases.
fn talk(model: &gob_voice::sherpa::Model, list: &[String], gain: f32) {
    let (tx, rx) = mpsc::channel::<Vec<i16>>();
    let capture = Capture::open(None, gain, move |s| {
        let _ = tx.send(s.to_vec());
    })
    .unwrap_or_else(|e| fail(e));
    println!(
        "{}: Enter starts a phrase, Enter again ends it, q quits. Phrases: {}",
        capture.name(),
        list.join(" | ")
    );
    let mut input = std::io::stdin().lock().lines();
    loop {
        let Some(Ok(line)) = input.next() else { return };
        if line.trim() == "q" {
            return;
        }
        // What came in before the press (kept open, nobody listening) is dropped.
        while rx.try_recv().is_ok() {}
        println!("listening... (Enter ends the phrase)");
        let Some(Ok(_)) = input.next() else { return };
        let samples: Vec<i16> = rx.try_iter().flatten().collect();
        let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        let mut r = model.recognizer();
        r.feed(&samples);
        let t = Instant::now();
        let text = r.finish();
        let decode = t.elapsed();
        let found = matcher::best(&text, &commands(list));
        let shown = match found {
            Some((i, c)) if c >= MIN_CONFIDENCE => format!("-> {} ({c:.2})", list[i]),
            Some((i, c)) => format!("-> nothing (closest {} at {c:.2})", list[i]),
            None => "-> nothing".to_string(),
        };
        println!(
            "{seconds:.1} s, loudest {:.2}, decoded in {} ms: heard {text:?} {shown}",
            f32::from(peak) / f32::from(i16::MAX),
            decode.as_millis()
        );
        println!("Enter for the next phrase, q quits");
    }
}

/// Each phrase a command of its own, as the matcher takes them.
fn commands(list: &[String]) -> Vec<(u32, String)> {
    (0u32..).zip(list.iter().cloned()).collect()
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

/// The samples of a mono 16-bit PCM WAV file at 16 kHz (other rates resampled); `None` (with a
/// note) for other files.
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
        // Another rate: linear resampling is good enough for speech.
        (Some((1, rate, 16)), Some(s)) if rate > 0 => {
            let n = s.len() as u64 * 16_000 / u64::from(rate);
            Some(
                (0..n)
                    .map(|i| {
                        let x = i as f64 * f64::from(rate) / 16_000.0;
                        let (j, f) = (x as usize, x.fract());
                        let a = f64::from(s[j.min(s.len() - 1)]);
                        let b = f64::from(s[(j + 1).min(s.len() - 1)]);
                        (a + (b - a) * f) as i16
                    })
                    .collect(),
            )
        }
        (f, _) => {
            println!("skip {}: not 16 kHz mono 16-bit PCM {f:?}", path.display());
            None
        }
    }
}

/// (current, peak) memory of this process in MB.
#[cfg(windows)]
fn memory() -> (f32, f32) {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_non_paged_pool: usize,
        quota_non_paged_pool: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters {
        cb: size_of::<Counters>() as u32,
        ..Default::default()
    };
    // SAFETY: the pseudo handle of this process and a buffer of the size given.
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    let mb = |b: usize| b as f32 / 1_048_576.0;
    (mb(c.working_set), mb(c.peak_working_set))
}

/// (current, peak) memory of this process in MB.
#[cfg(not(windows))]
fn memory() -> (f32, f32) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<f32>().ok())
            .map_or(0.0, |kb| kb / 1024.0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}
