//! [`VoiceService`]: voice commands on a thread of their own. It loads the speech model, opens a
//! microphone while it is told to listen, feeds the recognizer and reports each utterance when
//! listening stops. Nothing on the caller's thread waits for audio or the model.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Capture, Error, Heard, Recognizer, Vosk};

/// How often the thread looks for commands while audio comes in.
const POLL: Duration = Duration::from_millis(10);

/// Less audio than this (16 kHz samples, 0.15 s) is a slip of the button, not an utterance.
const MIN_SAMPLES: usize = 2_400;

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
}

/// What the service reports.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
    /// The model is loaded: listening works from now on.
    Ready,
    /// The service cannot listen (no library, no model, ...); it ignores requests from now on.
    Failed(String),
    /// A microphone opened (`true`) or closed.
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
}

/// The voice thread. Dropping it closes every microphone and ends the thread.
pub struct VoiceService {
    requests: Option<Sender<Request>>,
    thread: Option<JoinHandle<()>>,
}

impl VoiceService {
    /// Starts the thread, which loads libvosk and the model (reported as [`VoiceEvent::Ready`]
    /// or [`VoiceEvent::Failed`]) and then listens for `phrases`: (command id, phrase).
    pub fn start(
        config: VoiceConfig,
        phrases: Vec<(u32, String)>,
        events: Sender<VoiceEvent>,
    ) -> Self {
        let gain = config.gain;
        let min_confidence = config.min_confidence;
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
                let id = (!microphone.is_empty()).then_some(microphone);
                Ok(Box::new(Capture::open(id, gain, sink)?) as Box<dyn Send>)
            });
            Ok((make, open))
        };
        Self::spawn(setup, (gain, min_confidence), phrases, events)
    }

    fn spawn(
        setup: impl FnOnce() -> Result<(MakeRecognizer, OpenMicrophone), Error> + Send + 'static,
        (gain, min_confidence): (f32, f32),
        phrases: Vec<(u32, String)>,
        events: Sender<VoiceEvent>,
    ) -> Self {
        let (requests, inbox) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("gob-voice".into())
            .spawn(move || match setup() {
                Ok((make, open)) => {
                    let _ = events.send(VoiceEvent::Ready);
                    Worker {
                        make,
                        open,
                        gain,
                        min_confidence,
                        phrases,
                        events,
                        open_mics: HashMap::new(),
                    }
                    .run(&inbox);
                }
                Err(e) => {
                    log::warn!("voice commands off: {e}");
                    let _ = events.send(VoiceEvent::Failed(e.to_string()));
                }
            })
            .ok();
        Self {
            requests: Some(requests),
            thread,
        }
    }

    /// Opens (`on`) or closes a microphone (its id; empty = the default one). Closing it ends
    /// the utterance: what was heard comes as [`VoiceEvent::Heard`].
    pub fn listen(&self, microphone: &str, on: bool) {
        self.send(Request::Listen {
            microphone: microphone.to_string(),
            on,
        });
    }

    /// Listens for these phrases from the next utterance on.
    pub fn set_phrases(&self, phrases: Vec<(u32, String)>) {
        self.send(Request::Phrases(phrases));
    }

    /// The gain of microphones opened from now on.
    pub fn set_gain(&self, gain: f32) {
        self.send(Request::Gain(gain));
    }

    fn send(&self, request: Request) {
        if let Some(r) = &self.requests {
            let _ = r.send(request);
        }
    }
}

impl Drop for VoiceService {
    fn drop(&mut self) {
        self.requests = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// One open microphone and its utterance so far.
struct OpenMic {
    /// `None` once closed.
    device: Option<Box<dyn Send>>,
    audio: Receiver<Vec<i16>>,
    recognizer: Box<dyn Recognize>,
    /// Samples fed so far.
    samples: usize,
    /// Loudest of them.
    peak: u16,
}

impl OpenMic {
    /// Feeds the audio that came in since the last call.
    fn take_audio(&mut self) {
        while let Ok(s) = self.audio.try_recv() {
            self.samples += s.len();
            let peak = s.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
            self.peak = self.peak.max(peak);
            self.recognizer.feed(&s);
        }
    }
}

struct Worker {
    make: MakeRecognizer,
    open: OpenMicrophone,
    gain: f32,
    min_confidence: f32,
    phrases: Vec<(u32, String)>,
    events: Sender<VoiceEvent>,
    open_mics: HashMap<String, OpenMic>,
}

impl Worker {
    fn run(mut self, inbox: &Receiver<Request>) {
        loop {
            // Idle: sleep until a request; listening: also take the audio every POLL.
            let request = if self.open_mics.is_empty() {
                inbox.recv().map_err(|_| RecvTimeoutError::Disconnected)
            } else {
                inbox.recv_timeout(POLL)
            };
            match request {
                Ok(r) => self.handle(r),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            for mic in self.open_mics.values_mut() {
                mic.take_audio();
            }
        }
        for microphone in self.open_mics.keys().cloned().collect::<Vec<_>>() {
            self.close(&microphone);
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
            } => self.close(&microphone),
            Request::Phrases(phrases) => self.phrases = phrases,
            Request::Gain(gain) => self.gain = gain,
        }
    }

    fn start(&mut self, microphone: String) {
        if self.open_mics.contains_key(&microphone) {
            return;
        }
        let texts: Vec<String> = self.phrases.iter().map(|(_, p)| p.clone()).collect();
        let recognizer = match (self.make)(&texts) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("voice: {e}");
                return;
            }
        };
        let (tx, audio) = mpsc::channel();
        let sink = Box::new(move |s: &[i16]| {
            let _ = tx.send(s.to_vec());
        });
        match (self.open)(&microphone, self.gain, sink) {
            Ok(device) => {
                let _ = self.events.send(VoiceEvent::Listening {
                    microphone: microphone.clone(),
                    on: true,
                });
                self.open_mics.insert(
                    microphone,
                    OpenMic {
                        device: Some(device),
                        audio,
                        recognizer,
                        samples: 0,
                        peak: 0,
                    },
                );
            }
            Err(e) => log::warn!("voice: {e}"),
        }
    }

    fn close(&mut self, microphone: &str) {
        let Some(mut mic) = self.open_mics.remove(microphone) else {
            return;
        };
        mic.device = None;
        // With what arrived before the device closed.
        mic.take_audio();
        let _ = self.events.send(VoiceEvent::Listening {
            microphone: microphone.to_string(),
            on: false,
        });
        if mic.samples < MIN_SAMPLES {
            return;
        }
        let loudest = f32::from(mic.peak) / f32::from(i16::MAX);
        if mic.peak == 0 {
            log::warn!(
                "voice: microphone {:?} gave only silence: muted, its level at 0, or blocked by the \
                 system's privacy settings",
                microphone
            );
        }
        let heard = mic.recognizer.finish();
        let command = heard
            .phrase
            .filter(|_| heard.confidence >= self.min_confidence)
            .and_then(|i| self.phrases.get(i))
            .map(|(id, _)| *id);
        log::info!(
            "voice: {:.1} s, loudest {loudest:.2}: heard {:?} ({:.2}) -> {}",
            mic.samples as f32 / crate::SAMPLE_RATE as f32,
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

    /// Hears phrase `samples[0] - 1` with confidence `samples[1] / 100` of its last chunk.
    struct Fake {
        last: Vec<i16>,
    }

    impl Recognize for Fake {
        fn feed(&mut self, samples: &[i16]) {
            self.last = samples.to_vec();
        }
        fn finish(&mut self) -> Heard {
            Heard {
                phrase: usize::try_from(self.last[0] - 1).ok(),
                text: format!("phrase {}", self.last[0]),
                confidence: f32::from(self.last[1]) / 100.0,
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
            let sinks: Sinks = Arc::default();
            let gains: Arc<Mutex<Vec<f32>>> = Arc::default();
            let (tx, events) = mpsc::channel();
            let (s, g) = (sinks.clone(), gains.clone());
            let setup = move || -> Result<(MakeRecognizer, OpenMicrophone), Error> {
                let make: MakeRecognizer =
                    Box::new(|_| Ok(Box::new(Fake { last: vec![0, 0] }) as Box<dyn Recognize>));
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
            let service = VoiceService::spawn(setup, (1.5, 0.5), phrases, tx);
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
            (1.0, 0.5),
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
    fn dropping_closes_the_microphone() {
        let rig = Rig::new(&[(1, "a")]);
        rig.service.listen("", true);
        rig.opened("", true);
        let sinks = rig.sinks.clone();
        drop(rig);
        assert!(sinks.lock().unwrap().is_empty());
    }
}
