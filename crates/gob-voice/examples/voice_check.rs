//! Lists microphones and recognizes phrases, push-to-talk with Enter or from a WAV file.
//!
//! voice_check                                        list microphones
//! voice_check --lib <dir> --model <dir> [--mic <id>] [--wav <file>] <phrase>...
//!
//! Without --wav: Enter starts listening, Enter again ends the phrase, q quits.
//! --wav: a 16 kHz mono 16-bit PCM file, recognized as one phrase.

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::mpsc;

use gob_voice::{Capture, Recognizer, Vosk, microphones};

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut lib, mut model, mut mic, mut wav, mut phrases) = (None, None, None, None, Vec::new());
    while let Some(a) = args.next() {
        match a.as_str() {
            "--lib" => lib = args.next().map(PathBuf::from),
            "--model" => model = args.next().map(PathBuf::from),
            "--mic" => mic = args.next(),
            "--wav" => wav = args.next().map(PathBuf::from),
            _ => phrases.push(a),
        }
    }
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
        None => listen(&mut recognizer, mic.as_deref(), &phrases),
    }
}

fn listen(recognizer: &mut Recognizer, mic: Option<&str>, phrases: &[String]) {
    println!("Enter: start / end a phrase, q: quit");
    let mut lines = std::io::stdin().lock().lines();
    loop {
        match lines.next() {
            Some(Ok(l)) if l.trim() == "q" => return,
            Some(Ok(_)) => {}
            _ => return,
        }
        let (tx, rx) = mpsc::channel::<Vec<i16>>();
        let capture = Capture::open(mic, move |s| {
            let _ = tx.send(s.to_vec());
        })
        .unwrap_or_else(|e| fail(e));
        println!("listening...");
        let _ = lines.next();
        let level = capture.level();
        drop(capture);
        let mut samples = 0;
        for chunk in rx.try_iter() {
            samples += chunk.len();
            recognizer.feed(&chunk);
        }
        println!(
            "{:.1} s of audio, last level {level:.2}",
            samples as f32 / gob_voice::SAMPLE_RATE as f32
        );
        report(recognizer, phrases);
    }
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
