//! [`VoiceService`]: voice commands on a thread of their own (below normal priority). It loads the
//! speech model (sherpa-onnx), opens a
//! microphone while it is told to listen (or keeps the profile's microphones open, see
//! [`VoiceConfig::keep_open`]), feeds the recognizer and reports each utterance when listening
//! stops. Nothing on the caller's thread waits for audio or the model. A kept-open microphone
//! that stops working (a headset reconnecting) is opened again as soon as it is back.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::matcher::{self, Heard};
use crate::sherpa::{ModelSpec, Sherpa};
use crate::{Capture, Engine, Error};

/// How often the thread looks for commands while audio comes in.
const POLL: Duration = Duration::from_millis(10);

/// How often open microphones are checked, and missing kept-open ones retried.
const CHECK: Duration = Duration::from_millis(500);

/// How often a microphone opened in place of a missing chosen one looks for the chosen one.
const LOOK_FOR_CHOSEN: Duration = Duration::from_secs(2);

/// The longest wait between two tries to open a microphone that fails or keeps stopping: each
/// failure doubles the wait from [`CHECK`] up to this.
const MAX_RETRY: Duration = Duration::from_secs(30);

/// A microphone that has worked this long is fine again: its next stop is retried at once.
const STABLE: Duration = Duration::from_secs(30);

/// Less audio than this (16 kHz samples, 0.15 s) is a slip of the button, not an utterance.
const MIN_SAMPLES: usize = 2_400;

/// The most audio one utterance keeps (30 s): a button held down by mistake must not fill the
/// memory or keep the voice thread busy decoding minutes of audio; a command is said well within.
const MAX_SAMPLES: usize = 30 * 16_000;

/// Audio kept from before listening starts on an open microphone (0.3 s), so a word started
/// just before the press is heard whole.
const PRE_ROLL: usize = 4_800;

/// What the service needs to run.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceConfig {
    /// The speech recognizer: Vosk or sherpa-onnx.
    pub engine: Engine,
    /// Folders to look for the engine's library in, before the system's library path.
    pub library_dirs: Vec<PathBuf>,
    /// The speech model's folder (a model of `engine`).
    pub model: PathBuf,
    /// The language spoken (`en`, `de`...), for models that take one.
    pub language: String,
    /// Microphone gain (see [`Capture::set_gain`]).
    pub gain: f32,
    /// Utterances heard with a lower confidence (0.0..=1.0) count as not recognized.
    pub min_confidence: f32,
    /// Keep the microphones of [`VoiceService::set_microphones`] open between utterances: some
    /// give silence for a second or two after they open, which would swallow short commands.
    /// Only audio while listening is recognized.
    pub keep_open: bool,
}

/// What the service reports.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
    /// The model is loaded: listening works from now on.
    Ready,
    /// The service cannot listen (no library, no model, ...); it ignores requests from now on.
    Failed(String),
    /// Listening on a microphone started (`true`) or stopped.
    Listening { microphone: String, on: bool },
    /// An utterance ended. `command` is the id of the command whose phrase was heard, `None`
    /// when it was none of them or too unsure.
    Heard {
        command: Option<u32>,
        text: String,
        confidence: f32,
    },
}

/// Something that turns audio into a phrase index: Vosk with the phrases as its grammar,
/// sherpa-onnx and the matcher, or a fake in tests.
trait Recognize: Send {
    fn feed(&mut self, samples: &[i16]);
    fn finish(&mut self) -> Heard;
}

/// sherpa-onnx's recognizer with the phrases its text is matched against.
struct PhraseRecognizer {
    recognizer: crate::sherpa::Recognizer,
    phrases: Phrases,
}

impl Recognize for PhraseRecognizer {
    fn feed(&mut self, samples: &[i16]) {
        self.recognizer.feed(samples);
    }

    fn finish(&mut self) -> Heard {
        matcher::hear(&self.recognizer.finish(), &self.phrases)
    }
}

/// Vosk's recognizers: each listens for the phrases only. A phrase with a word the model does not
/// know can never be heard: logged once.
fn vosk_recognizers(config: &VoiceConfig) -> Result<MakeRecognizer, Error> {
    let model = crate::Vosk::load(&config.library_dirs)?.model(&config.model)?;
    let mut reported: HashSet<String> = HashSet::new();
    Ok(Box::new(move |phrases: &Phrases| {
        for (_, phrase) in phrases.iter() {
            let unknown = model.unknown_words(phrase);
            if !unknown.is_empty() && reported.insert(phrase.clone()) {
                log::warn!(
                    "voice: the speech model does not know {unknown:?}: phrase {phrase:?} will not be heard"
                );
            }
        }
        let texts: Vec<String> = phrases.iter().map(|(_, p)| p.clone()).collect();
        Ok(Box::new(VoskRecognizer(model.recognizer(&texts)?)) as Box<dyn Recognize>)
    }))
}

/// sherpa-onnx's recognizers, their text matched against the phrases.
fn sherpa_recognizers(config: &VoiceConfig) -> Result<MakeRecognizer, Error> {
    let sherpa = Sherpa::load(&config.library_dirs)?;
    // One thread per decode: a phrase decodes in a fraction of its length, and the game keeps
    // the other cores.
    let spec = ModelSpec::detect(&config.model, &config.language, 1)?;
    let model = sherpa.model(&spec)?;
    Ok(Box::new(move |phrases: &Phrases| {
        Ok(Box::new(PhraseRecognizer {
            recognizer: model.recognizer(),
            phrases: phrases.clone(),
        }) as Box<dyn Recognize>)
    }))
}

/// Vosk's recognizer: its result is already an index into the phrases it was made with.
struct VoskRecognizer(crate::vosk::Recognizer);

impl Recognize for VoskRecognizer {
    fn feed(&mut self, samples: &[i16]) {
        self.0.feed(samples);
    }

    fn finish(&mut self) -> Heard {
        self.0.finish()
    }
}

/// How an open microphone is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    Working,
    /// It stopped working: close it and open it again.
    Lost,
    /// It works, but stands in for the chosen microphone, which is connected now: switch once
    /// that one opens.
    Better,
}

/// An open microphone, as the worker sees it.
trait Mic: Send {
    fn health(&mut self) -> Health;
}

/// A [`Capture`], possibly standing in for a chosen microphone that was not connected.
struct Opened {
    capture: Capture,
    /// The chosen microphone this one stands in for.
    stand_in_for: Option<String>,
    looked: Instant,
}

impl Mic for Opened {
    fn health(&mut self) -> Health {
        if self.capture.lost() {
            return Health::Lost;
        }
        match &self.stand_in_for {
            Some(chosen) if self.looked.elapsed() >= LOOK_FOR_CHOSEN => {
                self.looked = Instant::now();
                if crate::microphones().iter().any(|m| &m.id == chosen) {
                    Health::Better
                } else {
                    Health::Working
                }
            }
            _ => Health::Working,
        }
    }
}

/// Makes a recognizer for the phrases.
type MakeRecognizer = Box<dyn FnMut(&Phrases) -> Result<Box<dyn Recognize>, Error> + Send>;
/// What the voice thread sets up: recognizers and microphones.
type Setup = (MakeRecognizer, OpenMicrophone);
/// Opens a microphone with a gain; it sends its samples to the sink until the result is dropped.
type OpenMicrophone =
    Box<dyn FnMut(&str, f32, Box<dyn FnMut(&[i16]) + Send>) -> Result<Box<dyn Mic>, Error> + Send>;

enum Request {
    Listen { microphone: String, on: bool },
    Phrases(Vec<(u32, String)>),
    Gain(f32),
    Microphones(Vec<String>),
}

/// The commands and their phrases: (command id, phrase).
type Phrases = Arc<Vec<(u32, String)>>;

/// The voice thread. Dropping it closes every microphone and ends the thread.
pub struct VoiceService {
    requests: Option<Sender<Request>>,
    thread: Option<JoinHandle<()>>,
    /// sherpa-onnx and the model are loaded (or failed to): the thread then ends quickly.
    set_up: Arc<AtomicBool>,
}

impl VoiceService {
    /// Starts the thread, which loads sherpa-onnx and the model (reported as [`VoiceEvent::Ready`]
    /// or [`VoiceEvent::Failed`]) and then listens for `phrases`: (command id, phrase).
    pub fn start(
        config: VoiceConfig,
        phrases: Vec<(u32, String)>,
        events: Sender<VoiceEvent>,
    ) -> Self {
        let settings = Settings {
            gain: config.gain,
            min_confidence: config.min_confidence,
            keep_open: config.keep_open,
        };
        let setup = move || -> Result<Setup, Error> {
            let started = std::time::Instant::now();
            let make: MakeRecognizer = match config.engine {
                Engine::Vosk => vosk_recognizers(&config)?,
                Engine::Sherpa => sherpa_recognizers(&config)?,
            };
            log::info!(
                "voice: {} speech model {} loaded in {:?}",
                config.engine.name(),
                config.model.display(),
                started.elapsed()
            );
            // Chosen microphones already reported missing: said once, not on every reopen.
            let mut reported = HashSet::new();
            let open: OpenMicrophone = Box::new(move |microphone, gain, sink| {
                // A chosen microphone that is not connected: the default one instead.
                let known = microphone.is_empty()
                    || crate::microphones().iter().any(|m| m.id == microphone);
                if known {
                    reported.remove(microphone);
                } else if reported.insert(microphone.to_string()) {
                    log::warn!("voice: microphone {microphone:?} not found, using the default one");
                }
                let id = (known && !microphone.is_empty()).then_some(microphone);
                Ok(Box::new(Opened {
                    capture: Capture::open(id, gain, sink)?,
                    stand_in_for: (!known).then(|| microphone.to_string()),
                    looked: Instant::now(),
                }) as Box<dyn Mic>)
            });
            Ok((make, open))
        };
        Self::spawn(setup, settings, phrases, events)
    }

    fn spawn(
        setup: impl FnOnce() -> Result<Setup, Error> + Send + 'static,
        settings: Settings,
        phrases: Vec<(u32, String)>,
        events: Sender<VoiceEvent>,
    ) -> Self {
        let (requests, inbox) = mpsc::channel();
        let set_up = Arc::new(AtomicBool::new(false));
        let done = set_up.clone();
        let thread = std::thread::Builder::new()
            .name("gob-voice".into())
            .spawn(move || {
                crate::platform::lower_priority();
                match setup() {
                    Ok((make, open)) => {
                        done.store(true, Ordering::Release);
                        let _ = events.send(VoiceEvent::Ready);
                        let worker = Worker {
                            make,
                            open,
                            settings,
                            phrases: Arc::new(phrases),
                            events,
                            mics: HashMap::new(),
                            warm: Vec::new(),
                            retry: HashMap::new(),
                            checked: Instant::now(),
                        };
                        worker.run(&inbox);
                    }
                    Err(e) => {
                        done.store(true, Ordering::Release);
                        log::error!("voice commands off: {e}");
                        let _ = events.send(VoiceEvent::Failed(e.to_string()));
                    }
                }
            })
            .ok();
        Self {
            requests: Some(requests),
            thread,
            set_up,
        }
    }

    /// Starts (`on`) or stops listening on a microphone (its id; empty = the default one).
    /// Stopping ends the utterance: what was heard comes as [`VoiceEvent::Heard`].
    pub fn listen(&self, microphone: &str, on: bool) {
        self.send(Request::Listen {
            microphone: microphone.to_string(),
            on,
        });
    }

    /// Listens for these phrases from the next utterance on; one under way keeps its own.
    pub fn set_phrases(&self, phrases: Vec<(u32, String)>) {
        self.send(Request::Phrases(phrases));
    }

    /// The gain of microphones opened from now on.
    pub fn set_gain(&self, gain: f32) {
        self.send(Request::Gain(gain));
    }

    /// The microphones the profile listens on: with [`VoiceConfig::keep_open`] they open now and
    /// stay open, the others close.
    pub fn set_microphones(&self, microphones: Vec<String>) {
        self.send(Request::Microphones(microphones));
    }

    fn send(&self, request: Request) {
        if let Some(r) = &self.requests {
            let _ = r.send(request);
        }
    }
}

impl Drop for VoiceService {
    /// Waits for the thread to close the microphones, unless it is still loading the model
    /// (seconds on a slow disk): then it is left to end by itself once loaded.
    fn drop(&mut self) {
        self.requests = None;
        if let Some(t) = self.thread.take()
            && self.set_up.load(Ordering::Acquire)
        {
            let _ = t.join();
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    gain: f32,
    min_confidence: f32,
    keep_open: bool,
}

/// An utterance being heard.
struct Utterance {
    recognizer: Box<dyn Recognize>,
    /// The phrases the recognizer was made for: its result is an index into them.
    phrases: Phrases,
    /// Samples fed since listening started (the pre-roll not counted).
    samples: usize,
    /// Loudest of them.
    peak: u16,
}

/// An open microphone.
struct OpenMic {
    device: Box<dyn Mic>,
    opened: Instant,
    audio: Receiver<Vec<i16>>,
    /// The last [`PRE_ROLL`] samples while nobody listens.
    recent: VecDeque<i16>,
    utterance: Option<Utterance>,
}

impl OpenMic {
    /// Takes the audio that came in: to the utterance, else into `recent`.
    fn take_audio(&mut self) {
        while let Ok(s) = self.audio.try_recv() {
            match &mut self.utterance {
                Some(u) => {
                    let room = MAX_SAMPLES.saturating_sub(u.samples);
                    if room == 0 {
                        continue;
                    }
                    let s = &s[..s.len().min(room)];
                    u.samples += s.len();
                    let peak = s.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
                    u.peak = u.peak.max(peak);
                    u.recognizer.feed(s);
                }
                None => {
                    self.recent.extend(&s);
                    let extra = self.recent.len().saturating_sub(PRE_ROLL);
                    self.recent.drain(..extra);
                }
            }
        }
    }
}

struct Worker {
    make: MakeRecognizer,
    open: OpenMicrophone,
    settings: Settings,
    phrases: Phrases,
    events: Sender<VoiceEvent>,
    mics: HashMap<String, OpenMic>,
    /// Microphones kept open ([`VoiceConfig::keep_open`]).
    warm: Vec<String>,
    /// Microphones that failed to open or stopped lately, and when to try them again.
    retry: HashMap<String, Retry>,
    checked: Instant,
}

/// When to try a microphone again.
struct Retry {
    at: Instant,
    /// The wait before `at`, doubled on each failure.
    delay: Duration,
    /// Its failure to open is in the log: said again only once it opens.
    warned: bool,
}

impl Retry {
    fn due(&self) -> bool {
        Instant::now() >= self.at
    }

    fn back_off(&mut self) {
        self.delay = (self.delay * 2).clamp(CHECK, MAX_RETRY);
        self.at = Instant::now() + self.delay;
    }
}

impl Worker {
    fn run(mut self, inbox: &Receiver<Request>) {
        loop {
            // Nothing open: sleep until a request (or the next retry of a kept-open microphone);
            // else also take the audio every POLL.
            let request = if !self.mics.is_empty() {
                inbox.recv_timeout(POLL)
            } else if !self.warm.is_empty() {
                inbox.recv_timeout(CHECK)
            } else {
                inbox.recv().map_err(|_| RecvTimeoutError::Disconnected)
            };
            match request {
                Ok(r) => self.handle(r),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            for mic in self.mics.values_mut() {
                mic.take_audio();
            }
            if self.checked.elapsed() >= CHECK {
                self.checked = Instant::now();
                self.check();
            }
        }
        self.warm.clear();
        for microphone in self.mics.keys().cloned().collect::<Vec<_>>() {
            self.stop(&microphone);
        }
    }

    fn handle(&mut self, request: Request) {
        match request {
            Request::Listen {
                microphone,
                on: true,
            } => self.start(microphone),
            Request::Listen {
                microphone,
                on: false,
            } => self.stop(&microphone),
            Request::Phrases(phrases) => self.phrases = Arc::new(phrases),
            Request::Gain(gain) => self.settings.gain = gain,
            Request::Microphones(microphones) => {
                self.warm = if self.settings.keep_open {
                    microphones
                } else {
                    Vec::new()
                };
                self.retry.clear();
                for m in self.warm.clone() {
                    self.open_warm(&m);
                }
                let idle: Vec<String> = self
                    .mics
                    .iter()
                    .filter(|(m, open)| open.utterance.is_none() && !self.warm.contains(m))
                    .map(|(m, _)| m.clone())
                    .collect();
                for m in idle {
                    self.mics.remove(&m);
                }
            }
        }
    }

    /// Closes microphones that stopped working, switches stand-ins to the chosen microphone and
    /// opens the kept-open ones again, each failure waiting longer for the next try. One that is
    /// in an utterance waits for it to end.
    fn check(&mut self) {
        let mut lost = Vec::new();
        let mut better = Vec::new();
        for (m, open) in &mut self.mics {
            if open.utterance.is_some() {
                continue;
            }
            match open.device.health() {
                Health::Working => {
                    // Fine for a while: a later stop is retried at once again.
                    if open.opened.elapsed() >= STABLE
                        && self.retry.get(m).is_some_and(|r| !r.warned)
                    {
                        self.retry.remove(m);
                    }
                }
                Health::Lost => lost.push(m.clone()),
                Health::Better => better.push(m.clone()),
            }
        }
        for m in lost {
            self.mics.remove(&m);
            match self.retry.get_mut(&m) {
                None => {
                    log::info!("voice: microphone {m:?} stopped, opening it again");
                    let retry = Retry {
                        at: Instant::now(),
                        delay: Duration::ZERO,
                        warned: false,
                    };
                    self.retry.insert(m, retry);
                }
                // Stopped again soon after opening: give it longer.
                Some(r) => {
                    r.back_off();
                    log::debug!(
                        "voice: microphone {m:?} stopped again, next try in {:?}",
                        r.delay
                    );
                }
            }
        }
        for m in better {
            if self.retry.get(&m).is_some_and(|r| !r.due()) {
                continue;
            }
            // The stand-in stays until the chosen microphone really opens.
            match self.open(&m) {
                Ok(open) => {
                    log::info!("voice: microphone {m:?} is connected again");
                    self.mics.insert(m.clone(), open);
                    self.opened(&m);
                }
                Err(e) => self.failed(&m, &e),
            }
        }
        for m in self.warm.clone() {
            self.open_warm(&m);
        }
    }

    /// Opens a kept-open microphone unless it is open or waits for its next try.
    fn open_warm(&mut self, microphone: &str) {
        if self.mics.contains_key(microphone)
            || self.retry.get(microphone).is_some_and(|r| !r.due())
        {
            return;
        }
        if let Err(e) = self.open_mic(microphone) {
            self.failed(microphone, &e);
        }
    }

    /// A microphone did not open: logged once until it opens, tried again later.
    fn failed(&mut self, microphone: &str, e: &Error) {
        let r = self
            .retry
            .entry(microphone.to_string())
            .or_insert_with(|| Retry {
                at: Instant::now(),
                delay: Duration::ZERO,
                warned: false,
            });
        r.back_off();
        if r.warned {
            log::debug!("voice: {e}; next try in {:?}", r.delay);
        } else {
            r.warned = true;
            log::warn!("voice: {e}");
        }
    }

    /// A microphone opened: if its failure was logged, so is its return.
    fn opened(&mut self, microphone: &str) {
        if let Some(r) = self.retry.get_mut(microphone)
            && r.warned
        {
            r.warned = false;
            log::info!("voice: microphone {microphone:?} is back");
        }
    }

    /// Opens a microphone unless it is open.
    fn open_mic(&mut self, microphone: &str) -> Result<(), Error> {
        if self.mics.contains_key(microphone) {
            return Ok(());
        }
        let open = self.open(microphone)?;
        self.mics.insert(microphone.to_string(), open);
        self.opened(microphone);
        Ok(())
    }

    fn open(&mut self, microphone: &str) -> Result<OpenMic, Error> {
        let (tx, audio) = mpsc::channel();
        let sink = Box::new(move |s: &[i16]| {
            let _ = tx.send(s.to_vec());
        });
        let device = (self.open)(microphone, self.settings.gain, sink)?;
        Ok(OpenMic {
            device,
            opened: Instant::now(),
            audio,
            recent: VecDeque::new(),
            utterance: None,
        })
    }

    fn start(&mut self, microphone: String) {
        if self
            .mics
            .get(&microphone)
            .is_some_and(|m| m.utterance.is_some())
        {
            return;
        }
        let mut recognizer = match (self.make)(&self.phrases) {
            Ok(r) => r,
            Err(e) => {
                log::error!("voice: {e}");
                return;
            }
        };
        if let Err(e) = self.open_mic(&microphone) {
            log::error!("voice: {e}");
            return;
        }
        let mic = self.mics.get_mut(&microphone).expect("just opened");
        mic.take_audio();
        let recent: Vec<i16> = mic.recent.drain(..).collect();
        recognizer.feed(&recent);
        mic.utterance = Some(Utterance {
            recognizer,
            phrases: self.phrases.clone(),
            samples: 0,
            peak: 0,
        });
        let _ = self.events.send(VoiceEvent::Listening {
            microphone,
            on: true,
        });
    }

    fn stop(&mut self, microphone: &str) {
        let Some(mic) = self.mics.get_mut(microphone) else {
            return;
        };
        mic.take_audio();
        let Some(mut u) = mic.utterance.take() else {
            return;
        };
        if !self.warm.iter().any(|m| m == microphone) {
            self.mics.remove(microphone);
        }
        let _ = self.events.send(VoiceEvent::Listening {
            microphone: microphone.to_string(),
            on: false,
        });
        if u.samples < MIN_SAMPLES {
            return;
        }
        let loudest = f32::from(u.peak) / f32::from(i16::MAX);
        if u.peak == 0 {
            log::warn!(
                "voice: microphone {:?} gave only silence: muted, its level at 0, blocked by the \
                 system's privacy settings, or silent for a while after it opens",
                microphone
            );
        }
        let heard = u.recognizer.finish();
        let command = heard
            .phrase
            .filter(|_| heard.confidence >= self.settings.min_confidence)
            .and_then(|i| u.phrases.get(i))
            .map(|(id, _)| *id);
        log::debug!(
            "voice: {:.1} s, loudest {loudest:.2}: heard {:?} ({:.2}) -> {}",
            u.samples as f32 / crate::SAMPLE_RATE as f32,
            heard.text,
            heard.confidence,
            command.map_or("nothing".to_string(), |c| format!("command {c}"))
        );
        let _ = self.events.send(VoiceEvent::Heard {
            command,
            text: heard.text,
            confidence: heard.confidence,
        });
    }
}

#[cfg(test)]
mod tests;
