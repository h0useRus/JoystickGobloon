//! Speaks a sentence with a Piper voice into a WAV file and prints how long it took and the
//! memory it needed.
//!
//! tts_check --lib <dir with sherpa-onnx with text to speech> --voice <unpacked voice dir>
//!     [--speed <x>] [--out <file.wav>] <text>

use std::path::PathBuf;
use std::time::Instant;

use gob_voice::tts::Voice;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut lib, mut voice, mut out, mut speed) = (None, None, PathBuf::from("tts.wav"), 1.0);
    let mut text = Vec::new();
    while let Some(a) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| fail(format!("{a} needs a value")))
        };
        match a.as_str() {
            "--lib" => lib = Some(PathBuf::from(value())),
            "--voice" => voice = Some(PathBuf::from(value())),
            "--out" => out = PathBuf::from(value()),
            "--speed" => speed = value().parse().unwrap_or_else(|e| fail(e)),
            _ => text.push(a),
        }
    }
    let (Some(lib), Some(voice)) = (lib, voice) else {
        fail("--lib and --voice are needed")
    };
    let t = Instant::now();
    let v = Voice::load(&lib, &voice).unwrap_or_else(|e| fail(e));
    println!("loaded in {:.2} s", t.elapsed().as_secs_f32());
    let t = Instant::now();
    let Some((samples, rate)) = v.speak(&text.join(" "), speed) else {
        fail("nothing spoken")
    };
    let took = t.elapsed().as_secs_f32();
    let length = samples.len() as f32 / rate as f32;
    println!(
        "{length:.2} s of speech at {rate} Hz in {took:.2} s (RTF {:.3})",
        took / length
    );
    write_wav(&out, &samples, rate);
    println!("written to {}", out.display());
}

fn write_wav(path: &PathBuf, samples: &[f32], rate: u32) {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, out).unwrap_or_else(|e| fail(e));
}

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}
