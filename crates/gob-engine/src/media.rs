//! Sound playback (play-sound) and speech (text-to-speech) on background threads,
//! started on first use. Failures are logged; they never stop the engine.

use std::sync::mpsc::{Sender, channel};

use crate::platform::SpeechEngine;

/// Gremlin's TTS queue modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechQueue {
    /// After everything queued.
    Back,
    /// Next, after the current sentence (SAPI cannot reorder: treated like `Back`).
    Front,
    /// Stops what is spoken and drops the queue.
    Interrupt,
}

impl SpeechQueue {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "interrupt" => Self::Interrupt,
            "queue-front" => Self::Front,
            _ => Self::Back,
        }
    }
}

/// One text-to-speech request; rate and pitch -1..1 (0 = normal), volume 0..1 (Qt's ranges).
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechRequest {
    pub text: String,
    /// Voice name as [`voices`] lists it; `None` or unknown: the system default.
    pub voice: Option<String>,
    pub rate: f32,
    pub volume: f32,
    pub pitch: f32,
    pub queue: SpeechQueue,
}

/// Sound and speech output: used by the runner and for previews in the editor.
#[derive(Default)]
pub struct Media {
    audio: Audio,
    speech: Speech,
}

impl Media {
    pub fn play(&mut self, file: &str, volume: u8) {
        self.audio.play(file, volume);
    }

    pub fn speak(&mut self, request: SpeechRequest) {
        self.audio.keep_awake();
        self.speech.speak(request);
    }
}

/// Plays sound files one after another (Gremlin's default "Sequential" mode).
#[derive(Default)]
pub struct Audio(Option<Sender<(String, u8)>>);

impl Audio {
    /// `volume` 0..=100.
    pub fn play(&mut self, file: &str, volume: u8) {
        let _ = self.start().send((file.to_string(), volume.min(100)));
    }

    /// Opens the output stream (it plays silence while idle) and keeps it open. Many sound
    /// devices fade in when a stream starts on an idle device; speech opens a new stream for
    /// every phrase, so its first words would be quiet.
    pub fn keep_awake(&mut self) {
        self.start();
    }

    fn start(&mut self) -> &Sender<(String, u8)> {
        self.0.get_or_insert_with(|| {
            let (tx, rx) = channel::<(String, u8)>();
            let spawned = std::thread::Builder::new()
                .name("gob-audio".into())
                .spawn(move || audio_thread(rx));
            if let Err(e) = spawned {
                log::error!("audio thread: {e}");
            }
            tx
        })
    }
}

fn audio_thread(rx: std::sync::mpsc::Receiver<(String, u8)>) {
    let (_stream, handle) = match rodio::OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            log::error!("no audio output: {e}");
            return;
        }
    };
    let sink = match rodio::Sink::try_new(&handle) {
        Ok(s) => s,
        Err(e) => {
            log::error!("audio sink: {e}");
            return;
        }
    };
    for (file, volume) in rx {
        let decoded = std::fs::File::open(&file)
            .map_err(|e| e.to_string())
            .and_then(|f| {
                rodio::Decoder::new(std::io::BufReader::new(f)).map_err(|e| e.to_string())
            });
        match decoded {
            Ok(source) => {
                use rodio::Source;
                sink.append(source.amplify(f32::from(volume) / 100.0));
            }
            Err(e) => log::error!("play-sound {file}: {e}"),
        }
    }
}

/// The platform's speech engine on its own thread (it may need one, e.g. SAPI and COM).
#[derive(Default)]
pub struct Speech(Option<Sender<SpeechRequest>>);

impl Speech {
    pub fn speak(&mut self, request: SpeechRequest) {
        let tx = self.0.get_or_insert_with(|| {
            let (tx, rx) = channel::<SpeechRequest>();
            let spawned = std::thread::Builder::new()
                .name("gob-speech".into())
                .spawn(move || speech_thread(rx));
            if let Err(e) = spawned {
                log::error!("speech thread: {e}");
            }
            tx
        });
        let _ = tx.send(request);
    }
}

/// Installed voice names, sorted. Enumerated once, on a separate thread.
pub fn voices() -> &'static [String] {
    static VOICES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    VOICES.get_or_init(|| {
        std::thread::spawn(|| {
            let mut names = <crate::platform::Speech as SpeechEngine>::voices();
            names.sort();
            names.dedup();
            names
        })
        .join()
        .unwrap_or_default()
    })
}

fn speech_thread(rx: std::sync::mpsc::Receiver<SpeechRequest>) {
    let Some(mut engine) = <crate::platform::Speech as SpeechEngine>::new() else {
        log::warn!("text-to-speech is not available on this system");
        for r in rx {
            log::warn!("text-to-speech skipped: {}", r.text);
        }
        return;
    };
    for request in rx {
        engine.speak(&request);
    }
}
