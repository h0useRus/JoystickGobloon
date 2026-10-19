//! Sound playback (play-sound) and speech (text-to-speech) on background threads,
//! started on first use. Failures are logged; they never stop the engine.

use std::sync::mpsc::{Sender, channel};

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

/// Windows speech (SAPI) voice on its own COM thread.
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

/// SAPI XML so every request carries its own rate, volume and pitch.
fn speech_xml(r: &SpeechRequest) -> String {
    let escaped = r
        .text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let scale = |v: f32| (v.clamp(-1.0, 1.0) * 10.0).round() as i32;
    format!(
        r#"<rate absspeed="{}"/><volume level="{}"/><pitch absmiddle="{}"/>{escaped}"#,
        scale(r.rate),
        (r.volume.clamp(0.0, 1.0) * 100.0).round() as i32,
        scale(r.pitch)
    )
}

/// Installed voice names (classic SAPI voices and the Windows OneCore ones), sorted.
/// Enumerated once, on a separate COM thread.
pub fn voices() -> &'static [String] {
    static VOICES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    VOICES.get_or_init(|| {
        std::thread::spawn(|| {
            let mut names: Vec<String> = sapi::tokens().into_iter().map(|(n, _)| n).collect();
            names.sort();
            names.dedup();
            names
        })
        .join()
        .unwrap_or_default()
    })
}

#[cfg(windows)]
mod sapi {
    use windows::Win32::Media::Speech::{
        ISpObjectToken, ISpObjectTokenCategory, SPCAT_VOICES, SpObjectTokenCategory,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    };
    use windows::core::PCWSTR;

    const ONECORE_VOICES: PCWSTR =
        windows::core::w!("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech_OneCore\\Voices");

    /// (name, token) of every voice; COM is initialised for the calling thread.
    pub fn tokens() -> Vec<(String, ISpObjectToken)> {
        let mut out = Vec::new();
        // SAFETY: plain COM calls on this thread; returned strings are freed with CoTaskMemFree.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            for category in [SPCAT_VOICES, ONECORE_VOICES] {
                let Ok(cat) = CoCreateInstance::<_, ISpObjectTokenCategory>(
                    &SpObjectTokenCategory,
                    None,
                    CLSCTX_ALL,
                ) else {
                    continue;
                };
                if cat.SetId(category, false).is_err() {
                    continue;
                }
                let Ok(list) = cat.EnumTokens(PCWSTR::null(), PCWSTR::null()) else {
                    continue;
                };
                let mut count = 0;
                if list.GetCount(&mut count).is_err() {
                    continue;
                }
                for i in 0..count {
                    let Ok(token) = list.Item(i) else { continue };
                    if let Ok(name) = token.GetStringValue(PCWSTR::null()) {
                        let text = name.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(name.0 as *const _));
                        if !text.is_empty() {
                            out.push((text, token));
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(not(windows))]
mod sapi {
    pub fn tokens() -> Vec<(String, ())> {
        Vec::new()
    }
}

#[cfg(windows)]
fn speech_thread(rx: std::sync::mpsc::Receiver<SpeechRequest>) {
    use windows::Win32::Media::Speech::{
        ISpVoice, SPF_ASYNC, SPF_IS_XML, SPF_PURGEBEFORESPEAK, SpVoice,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::core::HSTRING;

    // SAFETY: COM is initialised for this thread before the voice is created and used only here.
    let voice: ISpVoice = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        match CoCreateInstance(&SpVoice, None, CLSCTX_ALL) {
            Ok(v) => v,
            Err(e) => {
                log::error!("text-to-speech voice: {e}");
                return;
            }
        }
    };
    let tokens = sapi::tokens();
    let mut current_voice: Option<String> = None;
    for request in rx {
        if request.voice != current_voice {
            let token = request
                .voice
                .as_ref()
                .and_then(|v| tokens.iter().find(|(n, _)| n == v))
                .map(|(_, t)| t);
            // SAFETY: the token comes from this thread's enumeration; `None` restores the default.
            if let Err(e) = unsafe { voice.SetVoice(token) } {
                log::error!("text-to-speech voice {:?}: {e}", request.voice);
            }
            current_voice = request.voice.clone();
        }
        let mut flags = SPF_ASYNC.0 | SPF_IS_XML.0;
        if request.queue == SpeechQueue::Interrupt {
            flags |= SPF_PURGEBEFORESPEAK.0;
        }
        let text = HSTRING::from(speech_xml(&request));
        // SAFETY: `text` outlives the call; SAPI copies it for asynchronous speech.
        if let Err(e) = unsafe { voice.Speak(&text, flags as u32, None) } {
            log::error!("text-to-speech: {e}");
        }
    }
}

#[cfg(not(windows))]
fn speech_thread(rx: std::sync::mpsc::Receiver<SpeechRequest>) {
    for r in rx {
        log::warn!("text-to-speech is only available on Windows: {}", r.text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_xml_scales_and_escapes() {
        let r = SpeechRequest {
            text: "Mode <Combat> & go".into(),
            voice: None,
            rate: 0.5,
            volume: 0.8,
            pitch: -1.0,
            queue: SpeechQueue::Back,
        };
        assert_eq!(
            speech_xml(&r),
            r#"<rate absspeed="5"/><volume level="80"/><pitch absmiddle="-10"/>Mode &lt;Combat&gt; &amp; go"#
        );
    }
}
