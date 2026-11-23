//! Measures a sherpa-onnx speech model the way voice commands use it: load time, memory, decode
//! time per utterance (real-time factor) and how often the phrase matcher picks the right phrase.
//!
//! speech_bench --lib <dir> --model <dir> [--lang <code>] [--threads <n>] --phrases <dir> [--gain <percent>]
//!     <dir> holds phrases.txt and other.txt (one per line) and 16 kHz mono 16-bit WAV files named
//!     phrases-NN-*.wav / other-NN-*.wav (NN = line, from 00): each phrase file should match its
//!     line, each other file (speech that is no command) should match nothing.
//! speech_bench --lib <dir> --model <dir> [--lang <code>] [--threads <n>] --wavs <dir>
//!     prints what it hears in every WAV file of <dir> (e.g. the model's test_wavs).
//! speech_bench --lib <dir> --model <dir> [--lang <code>] [--threads <n>] --listen <phrases.txt> [--gain <percent>]
//!     [--vosk-lib <dir> --vosk-model <dir>] [--save <dir>]
//!     push to talk on the default microphone, kept open like the app does: Enter starts a
//!     phrase, Enter again ends it; prints what was heard and the phrase it matches. q quits.
//!     With a Vosk model (grammar mode, how 0.15 worked) "1" + Enter starts a phrase for Vosk,
//!     "2" + Enter for the sherpa-onnx model (as plain Enter), "3" + Enter for both on the same
//!     audio. --save keeps every phrase as NNN.wav (as recorded, before the gain) in <dir>.
//! speech_bench --record <dir> [--rounds <n>]
//!     records test files for --phrases on the default microphone (no model needed): shows each
//!     line of <dir>/phrases.txt and <dir>/other.txt in turn, n rounds (3), and saves what was
//!     said as phrases-NN-rK.wav / other-NN-rK.wav, as recorded (no gain) with the 0.3 s before
//!     Enter like the app. Enter starts, Enter ends, s + Enter skips, q + Enter quits.
//!
//! Prints one summary line at the end: load s | memory MB after load / peak | mean RTF | right
//! phrases | rejected other speech. Memory is the whole process (working set).

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use gob_voice::sherpa::{ModelSpec, Sherpa};
use gob_voice::{Capture, SAMPLE_RATE, matcher};

mod common {
    pub mod vosk;
}

/// Below this confidence a phrase counts as not recognized (the app's default).
const MIN_CONFIDENCE: f32 = 0.5;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut lib, mut model, mut phrases, mut wavs, mut listen) = (None, None, None, None, None);
    let (mut lang, mut threads, mut gain) = (String::new(), 1, 1.0);
    let (mut record, mut rounds) = (None, 3);
    let (mut vosk_lib, mut vosk_model, mut save) = (None::<PathBuf>, None::<PathBuf>, None);
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
            "--vosk-lib" => vosk_lib = Some(PathBuf::from(value())),
            "--vosk-model" => vosk_model = Some(PathBuf::from(value())),
            "--save" => save = Some(PathBuf::from(value())),
            "--record" => record = Some(PathBuf::from(value())),
            "--rounds" => rounds = value().parse().unwrap_or_else(|e| fail(e)),
            "--gain" => gain = value().parse::<f32>().unwrap_or_else(|e| fail(e)) / 100.0,
            _ => fail(format!("unknown argument {a}")),
        }
    }
    if let Some(dir) = record {
        record_phrases(&dir, rounds);
        return;
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
        let list = lines(&list);
        let vosk = match (vosk_lib, vosk_model) {
            (Some(l), Some(m)) => {
                Some(common::vosk::Vosk::load(&l, &m, &list).unwrap_or_else(|e| fail(e)))
            }
            (None, None) => None,
            _ => fail("--vosk-lib and --vosk-model go together"),
        };
        if let Some(dir) = &save {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| fail(e));
        }
        talk(&loaded, vosk.as_ref(), &list, gain, save.as_deref());
        return;
    }

    let mut rtf = Vec::new();
    let mut hear = |file: &Path| -> Option<String> {
        let samples: Vec<i16> = read_wav(file)?
            .iter()
            .map(|&s| (f32::from(s) * gain).clamp(-32768.0, 32767.0) as i16)
            .collect();
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

/// Push to talk with Enter on the default microphone, which stays open between phrases; with
/// `vosk`, "1" picks Vosk, "2" the sherpa-onnx model, "3" both.
fn talk(
    model: &gob_voice::sherpa::Model,
    vosk: Option<&common::vosk::Vosk>,
    list: &[String],
    gain: f32,
    save: Option<&Path>,
) {
    // Raw audio: the gain is applied here, so a saved file is as recorded.
    let (tx, rx) = mpsc::channel::<Vec<i16>>();
    let capture = Capture::open(None, 1.0, move |s| {
        let _ = tx.send(s.to_vec());
    })
    .unwrap_or_else(|e| fail(e));
    println!(
        "{}, gain {:.0} %. Phrases: {}",
        capture.name(),
        gain * 100.0,
        list.join(" | ")
    );
    let prompt = if vosk.is_some() {
        "1 + Enter = Vosk, 2 + Enter (or Enter) = sherpa-onnx, 3 + Enter = both; Enter ends the phrase; q quits"
    } else {
        "Enter starts a phrase, Enter again ends it; q quits"
    };
    println!("{prompt}");
    const PRE_ROLL: usize = SAMPLE_RATE as usize * 3 / 10;
    let mut input = std::io::stdin().lock().lines();
    let mut take = 0;
    loop {
        let Some(Ok(line)) = input.next() else { return };
        let (use_vosk, use_sherpa) = match (line.trim(), vosk.is_some()) {
            ("q", _) => return,
            ("1", true) => (true, false),
            ("3", true) => (true, true),
            _ => (false, true),
        };
        // What came in before the press: its last 0.3 s, like the app.
        let before: Vec<i16> = rx.try_iter().flatten().collect();
        let mut raw = before[before.len().saturating_sub(PRE_ROLL)..].to_vec();
        let engines = match (use_vosk, use_sherpa) {
            (true, true) => "Vosk + sherpa-onnx",
            (true, false) => "Vosk",
            _ => "sherpa-onnx",
        };
        println!("listening for {engines}... (Enter ends the phrase)");
        let Some(Ok(_)) = input.next() else { return };
        raw.extend(rx.try_iter().flatten());
        take += 1;
        if let Some(dir) = save {
            let _ = write_wav(&dir.join(format!("{take:03}.wav")), &raw);
        }
        let samples: Vec<i16> = raw
            .iter()
            .map(|&s| (f32::from(s) * gain).clamp(-32768.0, 32767.0) as i16)
            .collect();
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        println!(
            "#{take}: {:.1} s, loudest {:.2}",
            samples.len() as f32 / SAMPLE_RATE as f32,
            f32::from(peak) / f32::from(i16::MAX)
        );
        if use_vosk && let Some(v) = vosk {
            let t = Instant::now();
            let (text, confidence) = v.hear(&samples);
            let shown = v
                .phrase(&text, confidence, MIN_CONFIDENCE)
                .map_or_else(|| "nothing".to_string(), |i| list[i].clone());
            println!(
                "  Vosk        ({:>3} ms): heard {text:?} ({confidence:.2}) -> {shown}",
                t.elapsed().as_millis()
            );
        }
        if use_sherpa {
            let mut r = model.recognizer();
            r.feed(&samples);
            let t = Instant::now();
            let text = r.finish();
            let shown = match matcher::best(&text, &commands(list)) {
                Some((i, c)) if c >= MIN_CONFIDENCE => format!("{} ({c:.2})", list[i]),
                Some((i, c)) => format!("nothing (closest {} at {c:.2})", list[i]),
                None => "nothing".to_string(),
            };
            println!(
                "  sherpa-onnx ({:>3} ms): heard {text:?} -> {shown}",
                t.elapsed().as_millis()
            );
        }
        println!("{prompt}");
    }
}

/// `--record`: prompts every line of phrases.txt and other.txt `rounds` times and saves the audio.
fn record_phrases(dir: &Path, rounds: u32) {
    let sets = [
        ("phrases", lines(&dir.join("phrases.txt"))),
        ("other", lines(&dir.join("other.txt"))),
    ];
    let (tx, rx) = mpsc::channel::<Vec<i16>>();
    let capture = Capture::open(None, 1.0, move |s| {
        let _ = tx.send(s.to_vec());
    })
    .unwrap_or_else(|e| fail(e));
    println!(
        "{}: say each line shown. Enter starts, Enter ends, s skips, q quits.",
        capture.name()
    );
    const PRE_ROLL: usize = SAMPLE_RATE as usize * 3 / 10;
    let mut input = std::io::stdin().lock().lines();
    let mut saved = 0;
    for round in 1..=rounds {
        for (prefix, list) in &sets {
            for (n, line) in list.iter().enumerate() {
                println!(
                    "
[{round}/{rounds}] say: {line:?}   (Enter starts)"
                );
                let Some(Ok(cmd)) = input.next() else { return };
                match cmd.trim() {
                    "q" => {
                        println!("{saved} files saved in {}", dir.display());
                        return;
                    }
                    "s" => continue,
                    _ => {}
                }
                let before: Vec<i16> = rx.try_iter().flatten().collect();
                let mut samples = before[before.len().saturating_sub(PRE_ROLL)..].to_vec();
                println!("  recording... (Enter ends)");
                let Some(Ok(_)) = input.next() else { return };
                samples.extend(rx.try_iter().flatten());
                let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
                let file = dir.join(format!("{prefix}-{n:02}-r{round}.wav"));
                write_wav(&file, &samples).unwrap_or_else(|e| fail(e));
                saved += 1;
                println!(
                    "  {:.1} s, loudest {:.2} -> {}",
                    samples.len() as f32 / SAMPLE_RATE as f32,
                    f32::from(peak) / f32::from(i16::MAX),
                    file.file_name().unwrap().to_string_lossy()
                );
            }
        }
    }
    println!(
        "
done: {saved} files saved in {}",
        dir.display()
    );
}

/// Writes 16 kHz mono 16-bit PCM.
fn write_wav(path: &Path, samples: &[i16]) -> std::io::Result<()> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend(b"RIFF");
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(SAMPLE_RATE.to_le_bytes());
    out.extend((SAMPLE_RATE * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    for s in samples {
        out.extend(s.to_le_bytes());
    }
    std::fs::write(path, out)
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
