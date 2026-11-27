use std::sync::{Arc, Mutex};

use super::*;

type Sinks = Arc<Mutex<HashMap<String, Box<dyn FnMut(&[i16]) + Send>>>>;
/// Microphones that are disconnected: open ones report lost, new ones fail to open.
type Unplugged = Arc<Mutex<HashSet<String>>>;
/// How every open microphone reports itself, unless unplugged.
type Reports = Arc<Mutex<HashMap<String, Health>>>;

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
struct FakeMic(String, Sinks, Unplugged, Reports);

impl Mic for FakeMic {
    fn health(&mut self) -> Health {
        if self.2.lock().unwrap().contains(&self.0) {
            return Health::Lost;
        }
        let reports = self.3.lock().unwrap();
        reports.get(&self.0).copied().unwrap_or(Health::Working)
    }
}

impl Drop for FakeMic {
    fn drop(&mut self) {
        self.1.lock().unwrap().remove(&self.0);
    }
}

struct Rig {
    service: VoiceService,
    events: Receiver<VoiceEvent>,
    sinks: Sinks,
    unplugged: Unplugged,
    reports: Reports,
    /// Microphones that fail to open, while open ones keep working.
    refused: Unplugged,
    /// Tries to open a microphone, failed ones too.
    attempts: Arc<Mutex<usize>>,
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
        let unplugged: Unplugged = Arc::default();
        let reports: Reports = Arc::default();
        let attempts: Arc<Mutex<usize>> = Arc::default();
        let gains: Arc<Mutex<Vec<f32>>> = Arc::default();
        let (tx, events) = mpsc::channel();
        let (s, u, g) = (sinks.clone(), unplugged.clone(), gains.clone());
        let refused: Unplugged = Arc::default();
        let (h, a, f) = (reports.clone(), attempts.clone(), refused.clone());
        let setup = move || -> Result<Setup, Error> {
            let make: MakeRecognizer =
                Box::new(|_| Ok(Box::new(Fake { said: None }) as Box<dyn Recognize>));
            let open: OpenMicrophone = Box::new(move |microphone, gain, sink| {
                *a.lock().unwrap() += 1;
                if microphone == "broken"
                    || u.lock().unwrap().contains(microphone)
                    || f.lock().unwrap().contains(microphone)
                {
                    return Err(Error::Audio("broken".into()));
                }
                g.lock().unwrap().push(gain);
                s.lock().unwrap().insert(microphone.to_string(), sink);
                let mic = FakeMic(microphone.to_string(), s.clone(), u.clone(), h.clone());
                Ok(Box::new(mic) as Box<dyn Mic>)
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
            unplugged,
            reports,
            refused,
            attempts,
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

    fn plug(&self, microphone: &str, connected: bool) {
        let mut unplugged = self.unplugged.lock().unwrap();
        if connected {
            unplugged.remove(microphone);
        } else {
            unplugged.insert(microphone.to_string());
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
fn a_kept_open_microphone_reconnects() {
    let rig = Rig::warm(&[(1, "a")]);
    rig.service.set_microphones(vec!["".into(), "left".into()]);
    rig.open_mics(&["", "left"]);
    rig.plug("left", false);
    rig.open_mics(&[""]);
    rig.plug("left", true);
    rig.open_mics(&["", "left"]);
    assert_eq!(rig.utterance("left", 1, 100), heard(Some(1), 1, 1.0));
    assert_eq!(*rig.gains.lock().unwrap(), [1.5; 3], "left opened twice");
}

#[test]
fn a_microphone_lost_while_listening_waits_for_the_utterance() {
    let rig = Rig::warm(&[(1, "a")]);
    rig.service.set_microphones(vec!["".into()]);
    rig.open_mics(&[""]);
    rig.service.listen("", true);
    rig.opened("", true);
    rig.say("", 1, 100, MIN_SAMPLES);
    rig.plug("", false);
    std::thread::sleep(CHECK * 2);
    rig.open_mics(&[""]);
    rig.service.listen("", false);
    rig.opened("", false);
    assert_eq!(rig.next(), heard(Some(1), 1, 1.0));
    rig.open_mics(&[]);
    rig.plug("", true);
    rig.open_mics(&[""]);
}

#[test]
fn a_kept_open_microphone_missing_at_start_opens_when_connected() {
    let rig = Rig::warm(&[(1, "a")]);
    rig.plug("left", false);
    rig.service.set_microphones(vec!["left".into()]);
    std::thread::sleep(CHECK * 2);
    rig.open_mics(&[]);
    rig.plug("left", true);
    rig.open_mics(&["left"]);
}

#[test]
fn a_stand_in_stays_until_the_chosen_microphone_opens() {
    let rig = Rig::warm(&[(1, "a")]);
    rig.service.set_microphones(vec!["left".into()]);
    rig.open_mics(&["left"]);
    // Listed again but not opening yet: the working microphone is kept.
    rig.reports
        .lock()
        .unwrap()
        .insert("left".into(), Health::Better);
    rig.refused.lock().unwrap().insert("left".into());
    let opens = || *rig.attempts.lock().unwrap();
    let before = opens();
    while opens() == before {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        rig.gains.lock().unwrap().len(),
        1,
        "the new one did not open"
    );
    rig.open_mics(&["left"]);
    rig.refused.lock().unwrap().remove("left");
    let started = Instant::now();
    while rig.gains.lock().unwrap().len() < 2 {
        assert!(started.elapsed() < Duration::from_secs(5), "switched");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_microphone_that_keeps_stopping_is_tried_less_often() {
    let rig = Rig::warm(&[(1, "a")]);
    rig.reports.lock().unwrap().insert("".into(), Health::Lost);
    rig.service.set_microphones(vec!["".into()]);
    std::thread::sleep(Duration::from_millis(3_000));
    // Every CHECK it would be 7; backing off: open, at once again, then 1 s and 2 s later.
    let opens = *rig.attempts.lock().unwrap();
    assert!((3..=5).contains(&opens), "{opens} opens");
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
