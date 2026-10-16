//! Sound playback (play-sound) and speech (text-to-speech) on background threads,
//! started on first use. Failures are logged; they never stop the engine.

use std::sync::mpsc::{channel, Sender};

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
    pub rate: f32,
    pub volume: f32,
    pub pitch: f32,
    pub queue: SpeechQueue,
}

/// Plays sound files one after another (Gremlin's default "Sequential" mode).
#[derive(Default)]
pub struct Audio(Option<Sender<(String, u8)>>);

impl Audio {
    /// `volume` 0..=100.
    pub fn play(&mut self, file: &str, volume: u8) {
        let tx = self.0.get_or_insert_with(|| {
            let (tx, rx) = channel::<(String, u8)>();
            let spawned = std::thread::Builder::new()
                .name("gob-audio".into())
                .spawn(move || audio_thread(rx));
            if let Err(e) = spawned {
                log::error!("audio thread: {e}");
            }
            tx
        });
        let _ = tx.send((file.to_string(), volume.min(100)));
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

#[cfg(windows)]
fn speech_thread(rx: std::sync::mpsc::Receiver<SpeechRequest>) {
    use windows::core::HSTRING;
    use windows::Win32::Media::Speech::{
        ISpVoice, SpVoice, SPF_ASYNC, SPF_IS_XML, SPF_PURGEBEFORESPEAK,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

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
    for request in rx {
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
