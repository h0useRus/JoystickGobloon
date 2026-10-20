//! Windows speech (SAPI): classic SAPI voices and the Windows OneCore ones.

use windows::Win32::Media::Speech::{
    ISpObjectToken, ISpObjectTokenCategory, ISpVoice, SPCAT_VOICES, SPF_ASYNC, SPF_IS_XML,
    SPF_PURGEBEFORESPEAK, SpObjectTokenCategory, SpVoice,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::core::{HSTRING, PCWSTR};

use crate::media::{SpeechQueue, SpeechRequest};

const ONECORE_VOICES: PCWSTR =
    windows::core::w!("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech_OneCore\\Voices");

/// A SAPI voice, created and used on the speech thread (COM is initialised there).
pub(crate) struct Speech {
    voice: ISpVoice,
    tokens: Vec<(String, ISpObjectToken)>,
    current_voice: Option<String>,
}

impl crate::platform::SpeechEngine for Speech {
    fn new() -> Option<Self> {
        // SAFETY: COM is initialised for this thread before the voice is created and used only
        // here.
        let voice: ISpVoice = unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            match CoCreateInstance(&SpVoice, None, CLSCTX_ALL) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("text-to-speech voice: {e}");
                    return None;
                }
            }
        };
        Some(Self {
            voice,
            tokens: tokens(),
            current_voice: None,
        })
    }

    fn voices() -> Vec<String> {
        tokens().into_iter().map(|(n, _)| n).collect()
    }

    fn speak(&mut self, request: &SpeechRequest) {
        if request.voice != self.current_voice {
            let token = request
                .voice
                .as_ref()
                .and_then(|v| self.tokens.iter().find(|(n, _)| n == v))
                .map(|(_, t)| t);
            // SAFETY: the token comes from this thread's enumeration; `None` restores the default.
            if let Err(e) = unsafe { self.voice.SetVoice(token) } {
                log::error!("text-to-speech voice {:?}: {e}", request.voice);
            }
            self.current_voice = request.voice.clone();
        }
        let mut flags = SPF_ASYNC.0 | SPF_IS_XML.0;
        if request.queue == SpeechQueue::Interrupt {
            flags |= SPF_PURGEBEFORESPEAK.0;
        }
        let text = HSTRING::from(speech_xml(request));
        // SAFETY: `text` outlives the call; SAPI copies it for asynchronous speech.
        if let Err(e) = unsafe { self.voice.Speak(&text, flags as u32, None) } {
            log::error!("text-to-speech: {e}");
        }
    }
}

/// (name, token) of every voice; COM is initialised for the calling thread.
fn tokens() -> Vec<(String, ISpObjectToken)> {
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
