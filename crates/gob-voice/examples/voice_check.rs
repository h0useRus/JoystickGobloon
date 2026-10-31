//! Lists microphones and recognizes phrases, push-to-talk with Enter or from a WAV file.
//!
//! voice_check                                        list microphones
//! voice_check --lib <dir> --model <dir> [--mic <id>] [--wav <file> | --save <file>] <phrase>...
//!
//! Without --wav: Enter starts listening, Enter again ends the phrase, q quits.
//! --wav: a 16 kHz mono 16-bit PCM file, recognized as one phrase.
//! --save: writes each phrase heard from the microphone to this WAV file, to listen to it.

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::mpsc;

use gob_voice::{Capture, Recognizer, Vosk, microphones};

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut lib, mut model, mut mic, mut wav, mut save) = (None, None, None, None, None);
    let mut phrases = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--lib" => lib = args.next().map(PathBuf::from),
            "--model" => model = args.next().map(PathBuf::from),
            "--mic" => mic = args.next(),
            "--wav" => wav = args.next().map(PathBuf::from),
            "--save" => save = args.next().map(PathBuf::from),
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
        None => listen(&mut recognizer, mic.as_deref(), save.as_ref(), &phrases),
    }
}

fn listen(
    recognizer: &mut Recognizer,
    mic: Option<&str>,
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
        let capture = Capture::open(mic, move |s| {
            let _ = tx.send(s.to_vec());
        })
        .unwrap_or_else(|e| fail(e));
        println!("listening on {}...", capture.name());
        let _ = lines.next();
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
