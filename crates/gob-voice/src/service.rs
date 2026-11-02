//! [`VoiceService`]: voice commands on a thread of their own. It loads the speech model, opens a
//! microphone while it is told to listen (or keeps the profile's microphones open, see
//! [`VoiceConfig::keep_open`]), feeds the recognizer and reports each utterance when listening
//! stops. Nothing on the caller's thread waits for audio or the model.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Capture, Error, Heard, Recognizer, Vosk};

/// How often the thread looks for commands while audio comes in.
const POLL: Duration = Duration::from_millis(10);

/// Less audio than this (16 kHz samples, 0.15 s) is a slip of the button, not an utterance.
const MIN_SAMPLES: usize = 2_400;

/// Audio kept from before listening starts on an open microphone (0.3 s), so a word started
/// just before the press is heard whole.
const PRE_ROLL: usize = 4_800;

/// What the service needs to run.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceConfig {
    /// Folders to look for libvosk in, before the system's library path.
    pub library_dirs: Vec<PathBuf>,
    /// The speech model's folder.
    pub model: PathBuf,
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

/// Something that turns audio into a phrase index; Vosk's recognizer, or a fake in tests.
trait Recognize: Send {
    fn feed(&mut self, samples: &[i16]);
    fn finish(&mut self) -> Heard;
}

impl Recognize for Recognizer {
    fn feed(&mut self, samples: &[i16]) {
        Recognizer::feed(self, samples);
    }

    fn finish(&mut self) -> Heard {
        Recognizer::finish(self)
    }
}

/// Makes a recognizer for the phrases.
type MakeRecognizer = Box<dyn FnMut(&[String]) -> Result<Box<dyn Recognize>, Error> + Send>;
/// Opens a microphone with a gain; it sends its samples to the sink until the result is dropped.
type OpenMicrophone =
    Box<dyn FnMut(&str, f32, Box<dyn FnMut(&[i16]) + Send>) -> Result<Box<dyn Send>, Error> + Send>;

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
    /// libvosk and the model are loaded (or failed to): the thread then ends quickly.
    set_up: Arc<AtomicBool>,
}

impl VoiceService {
    /// Starts the thread, which loads libvosk and the model (reported as [`VoiceEvent::Ready`]
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
        let setup = move || -> Result<(MakeRecognizer, OpenMicrophone), Error> {
            let vosk = Vosk::load(&config.library_dirs)?;
            let started = std::time::Instant::now();
            let model = vosk.model(&config.model)?;
            log::info!(
                "voice: speech model {} loaded in {:?}",
                config.model.display(),
                started.elapsed()
            );
            let make: MakeRecognizer = Box::new(move |phrases| {
                let r = model.recognizer(phrases)?;
                Ok(Box::new(r) as Box<dyn Recognize>)
            });
            let open: OpenMicrophone = Box::new(move |microphone, gain, sink| {
                // A chosen microphone that is not connected: the default one instead.
                let known = microphone.is_empty()
                    || crate::microphones().iter().any(|m| m.id == microphone);
                if !known {
                    log::warn!("voice: microphone {microphone:?} not found, using the default one");
                }
                let id = (known && !microphone.is_empty()).then_some(microphone);
                Ok(Box::new(Capture::open(id, gain, sink)?) as Box<dyn Send>)
            });
            Ok((make, open))
        };
        Self::spawn(setup, settings, phrases, events)
    }

    fn spawn(
        setup: impl FnOnce() -> Result<(MakeRecognizer, OpenMicrophone), Error> + Send + 'static,
        settings: Settings,
        phrases: Vec<(u32, String)>,
        events: Sender<VoiceEvent>,
    ) -> Self {
        let (requests, inbox) = mpsc::channel();
        let set_up = Arc::new(AtomicBool::new(false));
        let done = set_up.clone();
        let thread = std::thread::Builder::new()
            .name("gob-voice".into())
            .spawn(move || match setup() {
                Ok((make, open)) => {
                    done.store(true, Ordering::Release);
                    let _ = events.send(VoiceEvent::Ready);
                    Worker {
                        make,
                        open,
                        settings,
                        phrases: Arc::new(phrases),
                        events,
                        mics: HashMap::new(),
                        warm: Vec::new(),
                    }
                    .run(&inbox);
                }
                Err(e) => {
                    done.store(true, Ordering::Release);
                    log::warn!("voice commands off: {e}");
                    let _ = events.send(VoiceEvent::Failed(e.to_string()));
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
    _device: Box<dyn Send>,
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
                    u.samples += s.len();
                    let peak = s.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
                    u.peak = u.peak.max(peak);
                    u.recognizer.feed(&s);
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
}

impl Worker {
    fn run(mut self, inbox: &Receiver<Request>) {
        loop {
            // Nothing open: sleep until a request; else also take the audio every POLL.
            let request = if self.mics.is_empty() {
                inbox.recv().map_err(|_| RecvTimeoutError::Disconnected)
            } else {
                inbox.recv_timeout(POLL)
            };
            match request {
                Ok(r) => self.handle(r),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            for mic in self.mics.values_mut() {
                mic.take_audio();
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
                for m in self.warm.clone() {
                    self.open_mic(&m);
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

    /// Opens a microphone unless it is open; false when it cannot be opened.
    fn open_mic(&mut self, microphone: &str) -> bool {
        if self.mics.contains_key(microphone) {
            return true;
        }
        let (tx, audio) = mpsc::channel();
        let sink = Box::new(move |s: &[i16]| {
            let _ = tx.send(s.to_vec());
        });
        match (self.open)(microphone, self.settings.gain, sink) {
            Ok(device) => {
                self.mics.insert(
                    microphone.to_string(),
                    OpenMic {
                        _device: device,
                        audio,
                        recent: VecDeque::new(),
                        utterance: None,
                    },
                );
                true
            }
            Err(e) => {
                log::warn!("voice: {e}");
                false
            }
        }
    }

    fn start(&mut self, microphone: String) {
        if self
            .mics
            .get(&microphone)
            .is_some_and(|m| m.utterance.is_some())
        {
            return;
        }
        let texts: Vec<String> = self.phrases.iter().map(|(_, p)| p.clone()).collect();
        let mut recognizer = match (self.make)(&texts) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("voice: {e}");
                return;
            }
        };
        if !self.open_mic(&microphone) {
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
        log::info!(
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
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    type Sinks = Arc<Mutex<HashMap<String, Box<dyn FnMut(&[i16]) + Send>>>>;

    /// Hears phrase `samples[0] - 1` with confidence `samples[1] / 100` of the first chunk fed
    /// that is not silence (`None` until then).
    struct Fake {
        said: Option<(i16, i16)>,
    }

    impl Recognize for Fake {
        fn feed(&mut self, samples: &[i16]) {
            if self.said.is_none() && samples.len() > 1 && (samples[0], samples[1]) != (0, 0) {
                self.said = Some((samples[0], samples[1]));
            }
        }
        fn finish(&mut self) -> Heard {
            let (phrase, confidence) = self.said.unwrap_or_default();
            Heard {
                phrase: usize::try_from(phrase - 1).ok(),
                text: format!("phrase {phrase}"),
                confidence: f32::from(confidence) / 100.0,
            }
        }
    }

    /// A microphone that closes when dropped: its sink leaves `Sinks`.
    struct Mic(String, Sinks);

    impl Drop for Mic {
        fn drop(&mut self) {
            self.1.lock().unwrap().remove(&self.0);
        }
    }

    struct Rig {
        service: VoiceService,
        events: Receiver<VoiceEvent>,
        sinks: Sinks,
        /// The gain of each microphone opened.
        gains: Arc<Mutex<Vec<f32>>>,
    }

    impl Rig {
        fn new(phrases: &[(u32, &str)]) -> Self {
            Self::with(phrases, false)
        }

        /// With [`VoiceConfig::keep_open`].
        fn warm(phrases: &[(u32, &str)]) -> Self {
            Self::with(phrases, true)
        }

        fn with(phrases: &[(u32, &str)], keep_open: bool) -> Self {
            let sinks: Sinks = Arc::default();
            let gains: Arc<Mutex<Vec<f32>>> = Arc::default();
            let (tx, events) = mpsc::channel();
            let (s, g) = (sinks.clone(), gains.clone());
            let setup = move || -> Result<(MakeRecognizer, OpenMicrophone), Error> {
                let make: MakeRecognizer =
                    Box::new(|_| Ok(Box::new(Fake { said: None }) as Box<dyn Recognize>));
                let open: OpenMicrophone = Box::new(move |microphone, gain, sink| {
                    if microphone == "broken" {
                        return Err(Error::Audio("broken".into()));
                    }
                    g.lock().unwrap().push(gain);
                    s.lock().unwrap().insert(microphone.to_string(), sink);
                    Ok(Box::new(Mic(microphone.to_string(), s.clone())) as Box<dyn Send>)
                });
                Ok((make, open))
            };
            let phrases = phrases.iter().map(|(id, p)| (*id, p.to_string())).collect();
            let settings = Settings {
                gain: 1.5,
                min_confidence: 0.5,
                keep_open,
            };
            let service = VoiceService::spawn(setup, settings, phrases, tx);
            let rig = Self {
                service,
                events,
                sinks,
                gains,
            };
            assert_eq!(rig.next(), VoiceEvent::Ready);
            rig
        }

        fn next(&self) -> VoiceEvent {
            self.events
                .recv_timeout(Duration::from_secs(5))
                .expect("an event")
        }

        /// Says phrase `phrase` (1-based, 0 = none) with `confidence` percent into `microphone`.
        fn say(&self, microphone: &str, phrase: i16, confidence: i16, samples: usize) {
            let mut audio = vec![0i16; samples];
            audio[0] = phrase;
            audio[1] = confidence;
            let mut sinks = self.sinks.lock().unwrap();
            (sinks.get_mut(microphone).expect("open"))(&audio);
        }

        /// Waits until the open microphones are these.
        fn open_mics(&self, expected: &[&str]) {
            let mut expected: Vec<&str> = expected.to_vec();
            expected.sort_unstable();
            let started = std::time::Instant::now();
            loop {
                let mut open: Vec<String> = self.sinks.lock().unwrap().keys().cloned().collect();
                open.sort_unstable();
                if open == expected {
                    return;
                }
                assert!(started.elapsed() < Duration::from_secs(5), "open: {open:?}");
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn opened(&self, microphone: &str, on: bool) {
            assert_eq!(
                self.next(),
                VoiceEvent::Listening {
                    microphone: microphone.into(),
                    on
                }
            );
        }

        fn utterance(&self, microphone: &str, phrase: i16, confidence: i16) -> VoiceEvent {
            self.service.listen(microphone, true);
            self.opened(microphone, true);
            self.say(microphone, phrase, confidence, MIN_SAMPLES);
            self.service.listen(microphone, false);
            self.opened(microphone, false);
            self.next()
        }
    }

    fn heard(command: Option<u32>, phrase: i16, confidence: f32) -> VoiceEvent {
        VoiceEvent::Heard {
            command,
            text: format!("phrase {phrase}"),
            confidence,
        }
    }

    #[test]
    fn an_utterance_becomes_its_command() {
        let rig = Rig::new(&[(7, "lights on"), (7, "lights up"), (3, "gear")]);
        assert_eq!(
            rig.utterance("", 2, 90),
            heard(Some(7), 2, 0.9),
            "second phrase of 7"
        );
        assert_eq!(rig.utterance("", 3, 100), heard(Some(3), 3, 1.0));
        assert_eq!(
            rig.utterance("", 0, 100),
            heard(None, 0, 1.0),
            "not a phrase"
        );
        assert_eq!(rig.utterance("", 1, 40), heard(None, 1, 0.4), "too unsure");
        assert!(
            rig.sinks.lock().unwrap().is_empty(),
            "the microphone is closed"
        );
        assert_eq!(*rig.gains.lock().unwrap(), [1.5; 4]);
    }

    #[test]
    fn a_short_press_hears_nothing() {
        let rig = Rig::new(&[(1, "fire")]);
        rig.service.listen("", true);
        rig.opened("", true);
        rig.say("", 1, 100, MIN_SAMPLES - 1);
        rig.service.listen("", false);
        rig.opened("", false);
        // The next event is from the next utterance: the short one reported nothing.
        assert_eq!(rig.utterance("", 1, 100), heard(Some(1), 1, 1.0));
    }

    #[test]
    fn two_microphones_at_once() {
        let rig = Rig::new(&[(1, "a"), (2, "b")]);
        rig.service.listen("left", true);
        rig.service.listen("right", true);
        rig.service.listen("left", true);
        rig.opened("left", true);
        rig.opened("right", true);
        rig.say("left", 1, 100, MIN_SAMPLES);
        rig.say("right", 2, 100, MIN_SAMPLES);
        rig.service.listen("right", false);
        rig.opened("right", false);
        assert_eq!(rig.next(), heard(Some(2), 2, 1.0));
        rig.service.listen("left", false);
        rig.opened("left", false);
        assert_eq!(rig.next(), heard(Some(1), 1, 1.0));
    }

    #[test]
    fn phrases_and_gain_change_for_the_next_utterance() {
        let rig = Rig::new(&[(1, "a")]);
        rig.service
            .set_phrases(vec![(5, "x".into()), (6, "y".into())]);
        rig.service.set_gain(0.5);
        assert_eq!(rig.utterance("", 2, 100), heard(Some(6), 2, 1.0));
        assert_eq!(*rig.gains.lock().unwrap(), [0.5]);
    }

    #[test]
    fn an_utterance_keeps_the_phrases_it_started_with() {
        let rig = Rig::new(&[(1, "a"), (2, "b")]);
        rig.service.listen("", true);
        rig.opened("", true);
        rig.say("", 2, 100, MIN_SAMPLES);
        rig.service.set_phrases(vec![(9, "x".into())]);
        rig.service.listen("", false);
        rig.opened("", false);
        assert_eq!(
            rig.next(),
            heard(Some(2), 2, 1.0),
            "the old list's second phrase"
        );
        assert_eq!(rig.utterance("", 1, 100), heard(Some(9), 1, 1.0));
    }

    #[test]
    fn dropping_does_not_wait_for_the_model() {
        let (tx, events) = mpsc::channel();
        let (release, wait) = mpsc::channel::<()>();
        let service = VoiceService::spawn(
            move || {
                let _ = wait.recv();
                Err(Error::Model("slow".into()))
            },
            Settings {
                gain: 1.0,
                min_confidence: 0.5,
                keep_open: false,
            },
            Vec::new(),
            tx,
        );
        let started = std::time::Instant::now();
        drop(service);
        assert!(started.elapsed() < Duration::from_secs(1));
        release.send(()).unwrap();
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)),
            Ok(VoiceEvent::Failed(_))
        ));
    }

    #[test]
    fn a_microphone_that_does_not_open_is_skipped() {
        let rig = Rig::new(&[(1, "a")]);
        rig.service.listen("broken", true);
        rig.service.listen("broken", false);
        assert_eq!(rig.utterance("", 1, 100), heard(Some(1), 1, 1.0));
    }

    #[test]
    fn a_failed_setup_is_reported() {
        let (tx, events) = mpsc::channel();
        let service = VoiceService::spawn(
            || Err(Error::Model("missing".into())),
            Settings {
                gain: 1.0,
                min_confidence: 0.5,
                keep_open: true,
            },
            Vec::new(),
            tx,
        );
        service.listen("", true);
        assert_eq!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            VoiceEvent::Failed("speech model: missing".into())
        );
        drop(service);
        assert!(events.recv().is_err(), "the thread ended");
    }

    #[test]
    fn warm_microphones_stay_open() {
        let rig = Rig::warm(&[(1, "a")]);
        rig.service.set_microphones(vec!["".into(), "left".into()]);
        rig.open_mics(&["", "left"]);
        assert_eq!(rig.utterance("", 1, 100), heard(Some(1), 1, 1.0));
        rig.open_mics(&["", "left"]);
        assert_eq!(rig.utterance("right", 1, 100), heard(Some(1), 1, 1.0));
        rig.open_mics(&["", "left"]);
        rig.service.set_microphones(vec!["left".into()]);
        rig.open_mics(&["left"]);
        assert_eq!(*rig.gains.lock().unwrap(), [1.5; 3], "opened once each");
    }

    #[test]
    fn a_word_started_before_the_press_is_heard() {
        let rig = Rig::warm(&[(1, "a"), (2, "b"), (3, "c")]);
        rig.service.set_microphones(vec!["".into()]);
        rig.open_mics(&[""]);
        rig.say("", 3, 100, 1_000);
        std::thread::sleep(Duration::from_millis(50));
        rig.service.listen("", true);
        rig.opened("", true);
        rig.say("", 0, 0, MIN_SAMPLES);
        rig.service.listen("", false);
        rig.opened("", false);
        assert_eq!(rig.next(), heard(Some(3), 3, 1.0));
    }

    #[test]
    fn microphones_stay_closed_without_keep_open() {
        let rig = Rig::new(&[(1, "a")]);
        rig.service.set_microphones(vec!["".into()]);
        assert_eq!(rig.utterance("", 1, 100), heard(Some(1), 1, 1.0));
        rig.open_mics(&[]);
    }

    #[test]
    fn dropping_closes_the_microphone() {
        let rig = Rig::new(&[(1, "a")]);
        rig.service.listen("", true);
        rig.opened("", true);
        let sinks = rig.sinks.clone();
        drop(rig);
        assert!(sinks.lock().unwrap().is_empty());
    }
}
