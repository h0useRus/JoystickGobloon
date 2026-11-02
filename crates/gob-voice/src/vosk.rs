//! Offline recognition with Vosk in grammar mode: the recognizer listens only for the given
//! phrases (plus `[unk]` for anything else), which keeps it accurate and stops it firing on other
//! speech. libvosk is loaded at run time through its C API.

use std::ffi::{CStr, CString, c_char, c_float, c_int, c_short, c_void};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libloading::Library;
use serde::Deserialize;

use crate::Error;
use crate::audio::SAMPLE_RATE;

type ModelNew = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type ModelFree = unsafe extern "C" fn(*mut c_void);
type ModelFindWord = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type RecognizerNewGrm = unsafe extern "C" fn(*mut c_void, c_float, *const c_char) -> *mut c_void;
type RecognizerSetWords = unsafe extern "C" fn(*mut c_void, c_int);
type AcceptWaveformS = unsafe extern "C" fn(*mut c_void, *const c_short, c_int) -> c_int;
type FinalResult = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type RecognizerReset = unsafe extern "C" fn(*mut c_void);
type RecognizerFree = unsafe extern "C" fn(*mut c_void);
type SetLogLevel = unsafe extern "C" fn(c_int);

struct Api {
    model_new: ModelNew,
    model_free: ModelFree,
    model_find_word: ModelFindWord,
    recognizer_new_grm: RecognizerNewGrm,
    recognizer_set_words: RecognizerSetWords,
    accept_waveform_s: AcceptWaveformS,
    final_result: FinalResult,
    recognizer_reset: RecognizerReset,
    recognizer_free: RecognizerFree,
    // Last: the function pointers above point into it.
    _lib: Library,
}

/// The loaded libvosk.
#[derive(Clone)]
pub struct Vosk(Arc<Api>);

impl Vosk {
    /// Loads libvosk from the first of `dirs` that has it, else from the system's library path.
    pub fn load(dirs: &[PathBuf]) -> Result<Self, Error> {
        let lib = dirs
            .iter()
            .map(|d| d.join(crate::platform::LIBRARY))
            .filter(|p| p.is_file())
            .find_map(|p| match crate::platform::load(&p) {
                Ok(lib) => Some(lib),
                Err(e) => {
                    log::warn!("{}: {e}", p.display());
                    None
                }
            });
        let lib = match lib {
            Some(lib) => lib,
            None => crate::platform::load(Path::new(crate::platform::LIBRARY))
                .map_err(|e| Error::Library(e.to_string()))?,
        };
        // SAFETY: the types match vosk_api.h; the pointers stay valid while `_lib` is loaded.
        unsafe {
            let set_log_level: SetLogLevel = symbol(&lib, b"vosk_set_log_level\0")?;
            // Errors only: Kaldi writes every model detail to stderr otherwise.
            set_log_level(-1);
            Ok(Self(Arc::new(Api {
                model_new: symbol(&lib, b"vosk_model_new\0")?,
                model_free: symbol(&lib, b"vosk_model_free\0")?,
                model_find_word: symbol(&lib, b"vosk_model_find_word\0")?,
                recognizer_new_grm: symbol(&lib, b"vosk_recognizer_new_grm\0")?,
                recognizer_set_words: symbol(&lib, b"vosk_recognizer_set_words\0")?,
                accept_waveform_s: symbol(&lib, b"vosk_recognizer_accept_waveform_s\0")?,
                final_result: symbol(&lib, b"vosk_recognizer_final_result\0")?,
                recognizer_reset: symbol(&lib, b"vosk_recognizer_reset\0")?,
                recognizer_free: symbol(&lib, b"vosk_recognizer_free\0")?,
                _lib: lib,
            })))
        }
    }

    /// Loads the speech model in `dir` (an unpacked Vosk model: am/, conf/, graph/...). Takes a
    /// second or two and 100-200 MB of memory.
    pub fn model(&self, dir: &Path) -> Result<Model, Error> {
        let path = crate::platform::native_path(dir).ok_or_else(|| {
            Error::Model(format!(
                "{} cannot be opened by the speech library: move the data folder to a path with                  only Latin letters",
                dir.display()
            ))
        })?;
        // SAFETY: a valid C string; null on failure.
        let ptr = unsafe { (self.0.model_new)(path.as_ptr()) };
        if ptr.is_null() {
            return Err(Error::Model(format!(
                "{} is not a speech model",
                dir.display()
            )));
        }
        Ok(Model(Arc::new(ModelPtr {
            api: self.0.clone(),
            ptr,
        })))
    }
}

/// # Safety
/// `T` must be the function pointer type of `name`.
unsafe fn symbol<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, Error> {
    // SAFETY: the caller names the right type.
    unsafe { lib.get::<T>(name) }
        .map(|s| *s)
        .map_err(|e| Error::Library(e.to_string()))
}

struct ModelPtr {
    api: Arc<Api>,
    ptr: *mut c_void,
}

// SAFETY: a Vosk model is read-only after loading and shared by recognizers on any thread.
unsafe impl Send for ModelPtr {}
unsafe impl Sync for ModelPtr {}

impl Drop for ModelPtr {
    fn drop(&mut self) {
        // SAFETY: made by vosk_model_new; recognizers keep the model alive.
        unsafe { (self.api.model_free)(self.ptr) }
    }
}

/// A loaded speech model, shared by its recognizers.
#[derive(Clone)]
pub struct Model(Arc<ModelPtr>);

impl Model {
    /// The words of `phrase` the model does not know: the recognizer would never hear them.
    pub fn unknown_words(&self, phrase: &str) -> Vec<String> {
        words(phrase)
            .filter(|w| {
                let Ok(c) = CString::new(w.as_bytes()) else {
                    return true;
                };
                // SAFETY: valid model and C string.
                unsafe { (self.0.api.model_find_word)(self.0.ptr, c.as_ptr()) < 0 }
            })
            .collect()
    }

    /// A recognizer that hears only `phrases`.
    pub fn recognizer(&self, phrases: &[String]) -> Result<Recognizer, Error> {
        let phrases: Vec<String> = phrases.iter().map(|p| normalize(p)).collect();
        let mut grammar: Vec<&str> = phrases
            .iter()
            .map(String::as_str)
            .filter(|p| !p.is_empty())
            .collect();
        grammar.push(UNKNOWN);
        let json = serde_json::to_string(&grammar).expect("strings");
        let json = CString::new(json).map_err(|_| Error::Model("bad phrase".into()))?;
        let api = &self.0.api;
        // SAFETY: valid model and C string; null on failure.
        let ptr =
            unsafe { (api.recognizer_new_grm)(self.0.ptr, SAMPLE_RATE as c_float, json.as_ptr()) };
        if ptr.is_null() {
            return Err(Error::Model("recognizer not created".into()));
        }
        // SAFETY: a fresh recognizer. Word results carry the confidence.
        unsafe { (api.recognizer_set_words)(ptr, 1) };
        Ok(Recognizer {
            model: self.0.clone(),
            ptr,
            phrases,
        })
    }
}

/// What grammar mode reports for speech outside the phrase list.
const UNKNOWN: &str = "[unk]";

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split_whitespace().map(str::to_lowercase)
}

/// Lower case, single spaces: how Vosk spells its results.
fn normalize(phrase: &str) -> String {
    words(phrase).collect::<Vec<_>>().join(" ")
}

/// The result of one utterance.
#[derive(Debug, Clone, PartialEq)]
pub struct Heard {
    /// Index into the phrases the recognizer was made with; `None` = not one of them.
    pub phrase: Option<usize>,
    /// What was heard, empty for silence.
    pub text: String,
    /// The lowest word confidence, 0.0..=1.0.
    pub confidence: f32,
}

/// Recognizes one utterance at a time: [`feed`](Self::feed) audio while the user speaks,
/// [`finish`](Self::finish) when they stop.
pub struct Recognizer {
    model: Arc<ModelPtr>,
    ptr: *mut c_void,
    phrases: Vec<String>,
}

// SAFETY: a recognizer is used by one thread at a time (`&mut self`).
unsafe impl Send for Recognizer {}

impl Recognizer {
    /// 16 kHz mono samples.
    pub fn feed(&mut self, samples: &[i16]) {
        for chunk in samples.chunks(c_int::MAX as usize) {
            // SAFETY: valid recognizer and buffer of `chunk.len()` samples.
            unsafe {
                (self.model.api.accept_waveform_s)(self.ptr, chunk.as_ptr(), chunk.len() as c_int)
            };
        }
    }

    /// Ends the utterance and returns what was heard; the next `feed` starts a new one.
    pub fn finish(&mut self) -> Heard {
        let api = &self.model.api;
        // SAFETY: valid recognizer; the string lives until the next call on it.
        let json = unsafe { CStr::from_ptr((api.final_result)(self.ptr)) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: valid recognizer.
        unsafe { (api.recognizer_reset)(self.ptr) };
        parse(&json, &self.phrases)
    }
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        // SAFETY: made by vosk_recognizer_new_grm.
        unsafe { (self.model.api.recognizer_free)(self.ptr) }
    }
}

#[derive(Deserialize)]
struct Result_ {
    #[serde(default)]
    text: String,
    #[serde(default)]
    result: Vec<Word>,
}

#[derive(Deserialize)]
struct Word {
    conf: f32,
}

fn parse(json: &str, phrases: &[String]) -> Heard {
    let r: Result_ = serde_json::from_str(json).unwrap_or_else(|e| {
        log::warn!("speech result {json:?}: {e}");
        Result_ {
            text: String::new(),
            result: Vec::new(),
        }
    });
    let confidence = r.result.iter().map(|w| w.conf).fold(1.0f32, f32::min);
    let text = r.text.trim().to_owned();
    let phrase = if text.contains(UNKNOWN) {
        None
    } else {
        phrases.iter().position(|p| *p == text)
    };
    Heard {
        phrase,
        confidence: if r.result.is_empty() { 0.0 } else { confidence },
        text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_are_spelled_like_results() {
        assert_eq!(normalize("  Lower   the Gear "), "lower the gear");
    }

    #[test]
    fn results() {
        let phrases = vec!["gear".to_owned(), "lower the gear".to_owned()];
        let heard = parse(
            r#"{"result":[{"conf":0.9,"word":"lower"},{"conf":0.7,"word":"the"},{"conf":1.0,"word":"gear"}],"text":"lower the gear"}"#,
            &phrases,
        );
        assert_eq!(
            heard,
            Heard {
                phrase: Some(1),
                text: "lower the gear".into(),
                confidence: 0.7
            }
        );
        let heard = parse(
            r#"{"result":[{"conf":1.0,"word":"[unk]"}],"text":"[unk]"}"#,
            &phrases,
        );
        assert_eq!(heard.phrase, None);
        let heard = parse(r#"{"text":""}"#, &phrases);
        assert_eq!(
            heard,
            Heard {
                phrase: None,
                text: String::new(),
                confidence: 0.0
            }
        );
    }
}
