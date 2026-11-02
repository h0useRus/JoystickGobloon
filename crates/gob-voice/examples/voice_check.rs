//! Lists microphones and recognizes phrases, push-to-talk with Enter or from a WAV file.
//!
//! voice_check                                        list microphones
//! voice_check --meter [--mic <id>] [--gain <percent>]  microphone test: live level, no recognition
//! voice_check --lib <dir> --model <dir> [--mic <id>] [--gain <percent>] [--wav <file> | --save <file>] <phrase>...
//! voice_check --lib <dir> --models <dir> [--lang <code>] ...   the same with the language's model
//!     from <dir>, downloaded (and checked) there first when it is missing; --lang defaults to en
//!
//! Without --wav: Enter starts listening, Enter again ends the phrase, q quits.
//! While the microphone is open a bar shows its level after the gain; "+" or "-" and Enter raise
//! or lower the gain by 10 % (up to 400 %) and keep listening. Speak at the loudness you would use
//! and aim for a peak around 0.5-0.8: 1.00 (CLIP) is too loud.
//! --wav: a 16 kHz mono 16-bit PCM file, recognized as one phrase.
//! --save: writes each phrase heard from the microphone to this WAV file, to listen to it.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use gob_voice::{Capture, MAX_GAIN, Recognizer, Vosk, microphones, models};

type Lines<'a> = std::io::Lines<std::io::StdinLock<'a>>;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut lib, mut model, mut mic, mut wav, mut save) = (None, None, None, None, None);
    let (mut gain, mut meter) = (1.0, false);
    let (mut models, mut lang) = (None, "en".to_string());
    let mut phrases = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--meter" => meter = true,
            "--gain" => {
                gain = args
                    .next()
                    .and_then(|g| g.trim_end_matches('%').parse::<f32>().ok())
                    .unwrap_or_else(|| fail("--gain takes a percentage, e.g. 150"))
                    / 100.0
            }
            "--lib" => lib = args.next().map(PathBuf::from),
            "--model" => model = args.next().map(PathBuf::from),
            "--models" => models = args.next().map(PathBuf::from),
            "--lang" => {
                lang = args
                    .next()
                    .unwrap_or_else(|| fail("--lang takes a code, e.g. de"))
            }
            "--mic" => mic = args.next(),
            "--wav" => wav = args.next().map(PathBuf::from),
            "--save" => save = args.next().map(PathBuf::from),
            _ => phrases.push(a),
        }
    }
    if meter {
        let capture = Capture::open(mic.as_deref(), gain, |_| {}).unwrap_or_else(|e| fail(e));
        println!(
            "testing {}: speak; +/- and Enter change the gain, Enter quits",
            capture.name()
        );
        hold(&capture, &mut std::io::stdin().lock().lines());
        return;
    }
    let model = model.or_else(|| models.map(|dir| model_for(&dir, &lang)));
    let Some(model) = model else {
        for m in microphones() {
            println!(
                "{} {}\n    {}",
                if m.default { "*" } else { " " },
                m.name,
                m.id
            );
        }
        return;
    };
    let vosk = Vosk::load(&lib.into_iter().collect::<Vec<_>>()).unwrap_or_else(|e| fail(e));
    let started = std::time::Instant::now();
    let model = vosk.model(&model).unwrap_or_else(|e| fail(e));
    println!("model loaded in {:?}", started.elapsed());
    for p in &phrases {
        let unknown = model.unknown_words(p);
        if !unknown.is_empty() {
            println!("{p:?}: the model does not know {unknown:?}");
        }
    }
    let mut recognizer = model.recognizer(&phrases).unwrap_or_else(|e| fail(e));
    match wav {
        Some(wav) => {
            recognizer.feed(&read_wav(&wav));
            report(&mut recognizer, &phrases);
        }
        None => listen(
            &mut recognizer,
            mic.as_deref(),
            gain,
            save.as_ref(),
            &phrases,
        ),
    }
}

fn listen(
    recognizer: &mut Recognizer,
    mic: Option<&str>,
    mut gain: f32,
    save: Option<&PathBuf>,
    phrases: &[String],
) {
    println!("Enter: start / end a phrase, q: quit");
    let mut lines = std::io::stdin().lock().lines();
    loop {
        match lines.next() {
            Some(Ok(l)) if l.trim() == "q" => return,
            Some(Ok(_)) => {}
            _ => return,
        }
        let (tx, rx) = mpsc::channel::<Vec<i16>>();
        let capture = Capture::open(mic, gain, move |s| {
            let _ = tx.send(s.to_vec());
        })
        .unwrap_or_else(|e| fail(e));
        println!("listening on {}...", capture.name());
        hold(&capture, &mut lines);
        gain = capture.gain();
        drop(capture);
        let samples: Vec<i16> = rx.try_iter().flatten().collect();
        recognizer.feed(&samples);
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        println!(
            "{:.1} s of audio, loudest {:.2}",
            samples.len() as f32 / gob_voice::SAMPLE_RATE as f32,
            f32::from(peak) / f32::from(i16::MAX)
        );
        if let Some(path) = save {
            write_wav(path, &samples);
            println!("saved to {}", path.display());
        }
        report(recognizer, phrases);
    }
}

/// The language's model in `dir`, installed first when it is missing.
fn model_for(dir: &std::path::Path, lang: &str) -> PathBuf {
    let info = models::for_language(lang).unwrap_or_else(|| fail(format!("no model for {lang}")));
    if let Some(path) = models::installed(dir, info) {
        return path;
    }
    println!(
        "downloading {} ({} MB)...",
        info.url(),
        info.size / 1_000_000
    );
    let mut shown = 0;
    let path = models::install(
        dir,
        info,
        &mut |done, total| {
            let percent = done * 100 / total.max(1);
            if percent >= shown + 10 || done == total {
                shown = percent;
                println!("  {percent:>3} %");
            }
        },
        &AtomicBool::new(false),
    )
    .unwrap_or_else(|e| fail(e));
    println!("installed in {}", path.display());
    path
}

/// Shows the microphone's level until a line other than "+" / "-" (which change the gain) is
/// entered or the input ends.
fn hold(capture: &Capture, lines: &mut Lines) {
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut loudest = 0.0f32;
            while !done.load(Ordering::Relaxed) {
                let level = capture.level();
                loudest = loudest.max(level);
                print!("\r{}  ", bar(level, loudest, capture.gain()));
                let _ = std::io::stdout().flush();
                std::thread::sleep(Duration::from_millis(50));
            }
            println!();
        });
        loop {
            let step = match lines.next() {
                Some(Ok(l)) if l.trim() == "+" => 0.1,
                Some(Ok(l)) if l.trim() == "-" => -0.1,
                _ => break,
            };
            capture.set_gain(((capture.gain() + step) * 10.0).round() / 10.0);
        }
        done.store(true, Ordering::Relaxed);
    });
}

/// `[#########|.....   ] 0.42 loudest 0.61 gain 150 %`: `#` = now, `|` = the loudest so far.
fn bar(level: f32, loudest: f32, gain: f32) -> String {
    const WIDTH: usize = 30;
    let cells = |v: f32| ((v * WIDTH as f32).round() as usize).min(WIDTH);
    let (now, top) = (cells(level), cells(loudest));
    let bar: String = (0..WIDTH)
        .map(|i| match i {
            i if i < now => '#',
            i if i + 1 == top => '|',
            _ => ' ',
        })
        .collect();
    let clip = if level >= 1.0 { " CLIP" } else { "" };
    format!(
        "[{bar}] {level:.2} loudest {loudest:.2} gain {:>3.0} % (max {:.0}){clip}",
        gain * 100.0,
        MAX_GAIN * 100.0
    )
}

fn report(recognizer: &mut Recognizer, phrases: &[String]) {
    let started = std::time::Instant::now();
    let heard = recognizer.finish();
    let what = match heard.phrase {
        Some(i) => format!("phrase {i} {:?}", phrases[i]),
        None => "not recognized".into(),
    };
    println!(
        "{what}: heard {:?}, confidence {:.2} ({:?})",
        heard.text,
        heard.confidence,
        started.elapsed()
    );
}

/// Writes 16 kHz mono samples as a 16-bit PCM WAV file.
fn write_wav(path: &PathBuf, samples: &[i16]) {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&gob_voice::SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(gob_voice::SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    samples
        .iter()
        .for_each(|s| out.extend_from_slice(&s.to_le_bytes()));
    std::fs::write(path, out).unwrap_or_else(|e| fail(e));
}

/// The samples of a 16 kHz mono 16-bit PCM WAV file.
fn read_wav(path: &PathBuf) -> Vec<i16> {
    let data = std::fs::read(path).unwrap_or_else(|e| fail(e));
    let mut pos = 12;
    let (mut format, mut samples) = (None, None);
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = &data[pos + 8..(pos + 8 + len).min(data.len())];
        match id {
            b"fmt " => {
                format = Some((
                    u16::from_le_bytes([body[2], body[3]]),
                    u32::from_le_bytes(body[4..8].try_into().unwrap()),
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
        (Some((1, 16_000, 16)), Some(s)) => s,
        (f, _) => fail(format!(
            "not 16 kHz mono 16-bit PCM: (channels, rate, bits) = {f:?}"
        )),
    }
}

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}
