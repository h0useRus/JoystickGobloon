//! Vosk in grammar mode (how voice commands worked up to 0.15), for comparing speech engines in
//! the examples. libvosk is loaded at run time; its folder must also hold the libraries it needs.

use std::ffi::{CStr, CString, c_char, c_float, c_int, c_short, c_void};
use std::path::Path;

use libloading::Library;

type ModelNew = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type RecognizerNewGrm = unsafe extern "C" fn(*mut c_void, c_float, *const c_char) -> *mut c_void;
type RecognizerSetWords = unsafe extern "C" fn(*mut c_void, c_int);
type AcceptWaveformS = unsafe extern "C" fn(*mut c_void, *const c_short, c_int) -> c_int;
type FinalResult = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type RecognizerFree = unsafe extern "C" fn(*mut c_void);
type SetLogLevel = unsafe extern "C" fn(c_int);

/// A loaded Vosk model and the grammar (phrases + `[unk]`) its recognizers listen for.
pub struct Vosk {
    new_grm: RecognizerNewGrm,
    set_words: RecognizerSetWords,
    accept: AcceptWaveformS,
    final_result: FinalResult,
    free: RecognizerFree,
    model: *mut c_void,
    grammar: CString,
    /// The phrases as Vosk spells them, in the order given.
    phrases: Vec<String>,
    // Last: the function pointers above point into it.
    _lib: Library,
}

impl Vosk {
    /// Loads libvosk from `lib_dir` and the model in `model`, listening for `phrases`.
    pub fn load(lib_dir: &Path, model: &Path, phrases: &[String]) -> Result<Self, String> {
        let file = lib_dir.join(if cfg!(windows) {
            "libvosk.dll"
        } else {
            "libvosk.so"
        });
        // SAFETY: the types match vosk_api.h; the pointers stay valid while `_lib` is loaded.
        unsafe {
            let lib = Library::new(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let set_log_level: SetLogLevel = sym(&lib, b"vosk_set_log_level\0")?;
            set_log_level(-1);
            let model_new: ModelNew = sym(&lib, b"vosk_model_new\0")?;
            let path =
                CString::new(model.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
            let ptr = model_new(path.as_ptr());
            if ptr.is_null() {
                return Err(format!("{} is not a Vosk model", model.display()));
            }
            let phrases: Vec<String> = phrases.iter().map(|p| normalize(p)).collect();
            let grammar: Vec<&str> = phrases
                .iter()
                .map(String::as_str)
                .chain(["[unk]"])
                .collect();
            let grammar = CString::new(serde_json::to_string(&grammar).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            Ok(Self {
                new_grm: sym(&lib, b"vosk_recognizer_new_grm\0")?,
                set_words: sym(&lib, b"vosk_recognizer_set_words\0")?,
                accept: sym(&lib, b"vosk_recognizer_accept_waveform_s\0")?,
                final_result: sym(&lib, b"vosk_recognizer_final_result\0")?,
                free: sym(&lib, b"vosk_recognizer_free\0")?,
                model: ptr,
                grammar,
                phrases,
                _lib: lib,
            })
        }
    }

    /// Decodes one utterance of 16 kHz mono samples: what was heard and its words' lowest
    /// confidence.
    pub fn hear(&self, samples: &[i16]) -> (String, f32) {
        // SAFETY: a valid model and grammar; the recognizer is freed before returning.
        let json = unsafe {
            let r = (self.new_grm)(self.model, 16_000.0, self.grammar.as_ptr());
            (self.set_words)(r, 1);
            (self.accept)(r, samples.as_ptr(), samples.len() as c_int);
            let out = CStr::from_ptr((self.final_result)(r))
                .to_string_lossy()
                .into_owned();
            (self.free)(r);
            out
        };
        parse(&json)
    }

    /// The phrase `text` is (its index), when `confidence` reaches `min`.
    pub fn phrase(&self, text: &str, confidence: f32, min: f32) -> Option<usize> {
        self.phrases
            .iter()
            .position(|p| p == text)
            .filter(|_| confidence >= min)
    }
}

/// # Safety
/// `T` must be the function pointer type of `name`.
unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: the caller names the right type.
    unsafe { lib.get::<T>(name) }
        .map(|s| *s)
        .map_err(|e| e.to_string())
}

/// The text and the lowest word confidence of a final result. Vosk may write numbers with the
/// locale's decimal comma.
fn parse(json: &str) -> (String, f32) {
    let c: Vec<char> = json.chars().collect();
    let fixed: String = (0..c.len())
        .map(|i| {
            if c[i] == ','
                && i > 0
                && c[i - 1].is_ascii_digit()
                && c.get(i + 1).is_some_and(char::is_ascii_digit)
            {
                '.'
            } else {
                c[i]
            }
        })
        .collect();
    let v: serde_json::Value = serde_json::from_str(&fixed).unwrap_or_default();
    let text = v["text"].as_str().unwrap_or_default().to_string();
    let confidence = v["result"]
        .as_array()
        .map(|words| {
            words
                .iter()
                .filter_map(|w| w["conf"].as_f64())
                .fold(1.0f64, f64::min) as f32
        })
        .unwrap_or(0.0);
    (text, confidence)
}

/// Lower case words, single spaces: how Vosk spells its results.
fn normalize(phrase: &str) -> String {
    phrase
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}
