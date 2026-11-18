//! Sound playback (play-sound) and speech (text-to-speech) on background threads,
//! started on first use. Failures are logged; they never stop the engine.
//!
//! Speech goes to the platform's engine (SAPI, speech-dispatcher), or to a natural (Piper)
//! voice when the request names one or names none and a natural voice is the default: the app
//! tells which are installed with [`set_natural_voices`].

use std::path::PathBuf;
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::platform::SpeechEngine;

/// A natural voice not used this long is unloaded (it takes 60-80 MB).
const NATURAL_IDLE: Duration = Duration::from_secs(5 * 60);

/// The natural voices installed, and which voice speaks when a request names none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NaturalVoices {
    /// The folder of sherpa-onnx with text to speech; `None` = not installed (no natural voice
    /// speaks).
    pub library: Option<PathBuf>,
    /// (name as listed and stored in profiles, folder) of each installed voice.
    pub voices: Vec<(String, PathBuf)>,
    /// The voice of requests that name none; `None` = the platform's default voice.
    pub default: Option<String>,
}

fn natural() -> &'static Mutex<NaturalVoices> {
    static NATURAL: OnceLock<Mutex<NaturalVoices>> = OnceLock::new();
    NATURAL.get_or_init(Mutex::default)
}

/// Tells which natural voices are installed (from now on, for every request).
pub fn set_natural_voices(voices: NaturalVoices) {
    *natural().lock().unwrap_or_else(|e| e.into_inner()) = voices;
}

/// Gremlin's TTS queue modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechQueue {
    /// After everything queued.
    Back,
    /// Next, after the current sentence (SAPI and speech-dispatcher cannot reorder: treated
    /// like `Back`).
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
    let mut device = match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(s) => s,
        Err(e) => {
            log::error!("no audio output: {e}");
            return;
        }
    };
    // Closing on exit is no news; rodio would print it to stderr.
    device.log_on_drop(false);
    let player = rodio::Player::connect_new(device.mixer());
    for (file, volume) in rx {
        let decoded = std::fs::File::open(&file)
            .map_err(|e| e.to_string())
            .and_then(|f| rodio::Decoder::try_from(f).map_err(|e| e.to_string()));
        match decoded {
            Ok(source) => {
                use rodio::Source;
                player.append(source.amplify(f32::from(volume) / 100.0));
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

/// Installed voice names: the natural voices first, then the platform's, sorted. The
/// platform's are enumerated once, on a separate thread.
pub fn voices() -> Vec<String> {
    static SYSTEM: OnceLock<Vec<String>> = OnceLock::new();
    let system = SYSTEM.get_or_init(|| {
        std::thread::spawn(|| {
            let mut names = <crate::platform::Speech as SpeechEngine>::voices();
            names.sort();
            names.dedup();
            names
        })
        .join()
        .unwrap_or_default()
    });
    let natural = natural().lock().unwrap_or_else(|e| e.into_inner());
    let mut names: Vec<String> = if natural.library.is_some() {
        natural.voices.iter().map(|(n, _)| n.clone()).collect()
    } else {
        Vec::new()
    };
    names.extend(system.iter().cloned());
    names
}

/// The natural voice loaded on the speech thread and what plays it.
struct Natural {
    name: String,
    voice: gob_voice::tts::Voice,
}

fn speech_thread(rx: std::sync::mpsc::Receiver<SpeechRequest>) {
    // Each made on first use: a profile may speak only with natural voices, or only the
    // platform's.
    let mut platform: Option<Option<crate::platform::Speech>> = None;
    let mut loaded: Option<Natural> = None;
    let mut output: Option<(rodio::MixerDeviceSink, rodio::Player)> = None;
    loop {
        let request = match rx.recv_timeout(NATURAL_IDLE) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => {
                if let Some(n) = loaded.take() {
                    log::debug!("natural voice {} unloaded after a quiet while", n.name);
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let natural = natural().lock().unwrap_or_else(|e| e.into_inner()).clone();
        let name = request.voice.clone().or(natural.default.clone());
        let found = natural.library.as_ref().and_then(|lib| {
            let name = name.as_ref()?;
            let (_, dir) = natural.voices.iter().find(|(n, _)| n == name)?;
            Some((lib, name, dir))
        });
        if let Some((library, name, dir)) = found
            && speak_natural(&request, library, name, dir, &mut loaded, &mut output)
        {
            continue;
        }
        // The platform's engine; a natural voice's name means nothing to it.
        let engine = platform.get_or_insert_with(|| {
            let engine = <crate::platform::Speech as SpeechEngine>::new();
            if engine.is_none() {
                log::warn!("text-to-speech is not available on this system");
            }
            engine
        });
        let Some(engine) = engine else {
            log::debug!("text-to-speech skipped: {}", request.text);
            continue;
        };
        let mut request = request;
        if request
            .voice
            .as_ref()
            .is_some_and(|v| natural.voices.iter().any(|(n, _)| n == v))
        {
            request.voice = None;
        }
        engine.speak(&request);
    }
}

/// Speaks with a natural voice; false when it could not (then the platform's voice speaks).
fn speak_natural(
    request: &SpeechRequest,
    library: &std::path::Path,
    name: &str,
    dir: &std::path::Path,
    loaded: &mut Option<Natural>,
    output: &mut Option<(rodio::MixerDeviceSink, rodio::Player)>,
) -> bool {
    if loaded.as_ref().is_none_or(|n| n.name != name) {
        *loaded = None;
        let started = std::time::Instant::now();
        match gob_voice::tts::Voice::load(library, dir) {
            Ok(voice) => {
                log::info!("natural voice {name} loaded in {:?}", started.elapsed());
                *loaded = Some(Natural {
                    name: name.to_string(),
                    voice,
                });
            }
            Err(e) => {
                log::error!("natural voice {name}: {e}");
                return false;
            }
        }
    }
    let Some(n) = loaded.as_ref() else {
        return false;
    };
    if output.is_none() {
        match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut device) => {
                // Closing on exit is no news; rodio would print it to stderr.
                device.log_on_drop(false);
                let player = rodio::Player::connect_new(device.mixer());
                *output = Some((device, player));
            }
            Err(e) => {
                log::error!("no audio output for speech: {e}");
                return false;
            }
        }
    }
    let Some((_, player)) = output.as_ref() else {
        return false;
    };
    // Rate -1..1 as twice slower .. twice faster.
    let speed = 2f32.powf(request.rate.clamp(-1.0, 1.0));
    let Some((samples, rate)) = n.voice.speak(&request.text, speed) else {
        log::warn!("natural voice {name} said nothing for {:?}", request.text);
        return true;
    };
    if request.queue == SpeechQueue::Interrupt {
        player.clear();
        player.play();
    }
    let (Some(channels), Some(rate)) = (std::num::NonZero::new(1), std::num::NonZero::new(rate))
    else {
        return true;
    };
    use rodio::Source;
    let source = rodio::buffer::SamplesBuffer::new(channels, rate, samples)
        .amplify(request.volume.clamp(0.0, 1.0));
    player.append(source);
    true
}
