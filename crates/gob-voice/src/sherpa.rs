//! Offline speech recognition with sherpa-onnx, loaded at run time through its C API (the
//! program runs without it). Push to talk: [`Recognizer::feed`] keeps the audio while the button
//! is held, [`Recognizer::finish`] decodes it once and returns the text. Which phrase that text
//! means is up to the caller.
//!
//! The `#[repr(C)]` structs mirror `sherpa-onnx/c-api/c-api.h` of sherpa-onnx v2.0.1 field by
//! field (layout as in the Apache-2.0 `sherpa-onnx-sys` crate of that version); a library of
//! another version may lay them out differently, so [`Sherpa::load`] checks its version.

use std::ffi::{CStr, CString, c_char, c_float};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;

use libloading::Library;
use serde::Deserialize;

use crate::Error;
use crate::audio::SAMPLE_RATE;

/// The sherpa-onnx version whose C structs this file mirrors.
pub const VERSION: &str = "2.0.1";

#[repr(C)]
#[derive(Clone, Copy)]
struct Transducer {
    encoder: *const c_char,
    decoder: *const c_char,
    joiner: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OneModel {
    model: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Whisper {
    encoder: *const c_char,
    decoder: *const c_char,
    language: *const c_char,
    task: *const c_char,
    tail_paddings: i32,
    enable_token_timestamps: i32,
    enable_segment_timestamps: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Canary {
    encoder: *const c_char,
    decoder: *const c_char,
    src_lang: *const c_char,
    tgt_lang: *const c_char,
    use_pnc: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EncoderDecoder {
    encoder: *const c_char,
    decoder: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Moonshine {
    preprocessor: *const c_char,
    encoder: *const c_char,
    uncached_decoder: *const c_char,
    cached_decoder: *const c_char,
    merged_decoder: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SenseVoice {
    model: *const c_char,
    language: *const c_char,
    use_itn: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FunAsrNano {
    encoder_adaptor: *const c_char,
    llm: *const c_char,
    embedding: *const c_char,
    tokenizer: *const c_char,
    system_prompt: *const c_char,
    user_prompt: *const c_char,
    max_new_tokens: i32,
    temperature: c_float,
    top_p: c_float,
    seed: i32,
    language: *const c_char,
    itn: i32,
    hotwords: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Qwen3Asr {
    conv_frontend: *const c_char,
    encoder: *const c_char,
    decoder: *const c_char,
    tokenizer: *const c_char,
    max_total_len: i32,
    max_new_tokens: i32,
    temperature: c_float,
    top_p: c_float,
    seed: i32,
    hotwords: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CohereTranscribe {
    encoder: *const c_char,
    decoder: *const c_char,
    language: *const c_char,
    use_punct: i32,
    use_itn: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ModelConfig {
    transducer: Transducer,
    paraformer: OneModel,
    nemo_ctc: OneModel,
    whisper: Whisper,
    tdnn: OneModel,
    tokens: *const c_char,
    num_threads: i32,
    debug: i32,
    provider: *const c_char,
    model_type: *const c_char,
    modeling_unit: *const c_char,
    bpe_vocab: *const c_char,
    telespeech_ctc: *const c_char,
    sense_voice: SenseVoice,
    moonshine: Moonshine,
    fire_red_asr: EncoderDecoder,
    dolphin: OneModel,
    zipformer_ctc: OneModel,
    canary: Canary,
    wenet_ctc: OneModel,
    omnilingual: OneModel,
    medasr: OneModel,
    funasr_nano: FunAsrNano,
    fire_red_asr_ctc: OneModel,
    qwen3_asr: Qwen3Asr,
    cohere_transcribe: CohereTranscribe,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FeatureConfig {
    sample_rate: i32,
    feature_dim: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LmConfig {
    model: *const c_char,
    scale: c_float,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct HomophoneReplacer {
    dict_dir: *const c_char,
    lexicon: *const c_char,
    rule_fsts: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CtcFstDecoder {
    graph: *const c_char,
    max_active: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RecognizerConfig {
    feat_config: FeatureConfig,
    model_config: ModelConfig,
    lm_config: LmConfig,
    decoding_method: *const c_char,
    max_active_paths: i32,
    hotwords_file: *const c_char,
    hotwords_score: c_float,
    rule_fsts: *const c_char,
    rule_fars: *const c_char,
    blank_penalty: c_float,
    hr: HomophoneReplacer,
    ctc_fst_decoder_config: CtcFstDecoder,
}

/// Opaque `SherpaOnnxOfflineRecognizer` / `SherpaOnnxOfflineStream`.
type Handle = *const std::ffi::c_void;

type VersionStr = unsafe extern "C" fn() -> *const c_char;
type CreateRecognizer = unsafe extern "C" fn(*const RecognizerConfig) -> Handle;
type DestroyRecognizer = unsafe extern "C" fn(Handle);
type CreateStream = unsafe extern "C" fn(Handle) -> Handle;
type DestroyStream = unsafe extern "C" fn(Handle);
type AcceptWaveform = unsafe extern "C" fn(Handle, i32, *const f32, i32);
type Decode = unsafe extern "C" fn(Handle, Handle);
type ResultJson = unsafe extern "C" fn(Handle) -> *const c_char;
type FreeJson = unsafe extern "C" fn(*const c_char);

struct Api {
    create_recognizer: CreateRecognizer,
    destroy_recognizer: DestroyRecognizer,
    create_stream: CreateStream,
    destroy_stream: DestroyStream,
    accept_waveform: AcceptWaveform,
    decode: Decode,
    result_json: ResultJson,
    free_json: FreeJson,
    version: String,
    // Last: the function pointers above point into it.
    _lib: Library,
}

/// The loaded sherpa-onnx library.
#[derive(Clone)]
pub struct Sherpa(Arc<Api>);

impl Sherpa {
    /// Loads sherpa-onnx from the first of `dirs` that has it (its ONNX Runtime next to it), else
    /// from the system's library path. A library of another version than [`VERSION`] is refused:
    /// its structs may not match.
    pub fn load(dirs: &[PathBuf]) -> Result<Self, Error> {
        let file = crate::platform::SHERPA_LIBRARY;
        let lib = dirs
            .iter()
            .map(|d| d.join(file))
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
            None => {
                crate::platform::load(Path::new(file)).map_err(|e| Error::Library(e.to_string()))?
            }
        };
        // SAFETY: the types match c-api.h; the pointers stay valid while `_lib` is loaded.
        unsafe {
            let version: VersionStr = symbol(&lib, b"SherpaOnnxGetVersionStr\0")?;
            let version = text(version());
            if version != VERSION {
                return Err(Error::Library(format!(
                    "sherpa-onnx {version} found, {VERSION} needed"
                )));
            }
            Ok(Self(Arc::new(Api {
                create_recognizer: symbol(&lib, b"SherpaOnnxCreateOfflineRecognizer\0")?,
                destroy_recognizer: symbol(&lib, b"SherpaOnnxDestroyOfflineRecognizer\0")?,
                create_stream: symbol(&lib, b"SherpaOnnxCreateOfflineStream\0")?,
                destroy_stream: symbol(&lib, b"SherpaOnnxDestroyOfflineStream\0")?,
                accept_waveform: symbol(&lib, b"SherpaOnnxAcceptWaveformOffline\0")?,
                decode: symbol(&lib, b"SherpaOnnxDecodeOfflineStream\0")?,
                result_json: symbol(&lib, b"SherpaOnnxGetOfflineStreamResultAsJson\0")?,
                free_json: symbol(&lib, b"SherpaOnnxDestroyOfflineStreamResultJson\0")?,
                version,
                _lib: lib,
            })))
        }
    }

    pub fn version(&self) -> &str {
        &self.0.version
    }

    /// Loads the model `spec` describes: seconds on a slow disk, its size in memory.
    pub fn model(&self, spec: &ModelSpec) -> Result<Model, Error> {
        let mut strings = Strings::default();
        let path = |s: &mut Strings, file: &Option<String>| match file {
            Some(f) => s.path(&spec.dir.join(f)),
            None => Ok(ptr::null()),
        };
        let none = ptr::null();
        let one = |p| OneModel { model: p };
        let mut model = ModelConfig {
            transducer: Transducer {
                encoder: none,
                decoder: none,
                joiner: none,
            },
            paraformer: one(none),
            nemo_ctc: one(none),
            whisper: Whisper {
                encoder: none,
                decoder: none,
                language: none,
                task: none,
                tail_paddings: -1,
                enable_token_timestamps: 0,
                enable_segment_timestamps: 0,
            },
            tdnn: one(none),
            tokens: strings.path(&spec.dir.join(&spec.tokens))?,
            num_threads: spec.threads.max(1),
            debug: 0,
            provider: strings.text("cpu")?,
            model_type: none,
            modeling_unit: none,
            bpe_vocab: none,
            telespeech_ctc: none,
            sense_voice: SenseVoice {
                model: none,
                language: none,
                use_itn: 0,
            },
            moonshine: Moonshine {
                preprocessor: none,
                encoder: none,
                uncached_decoder: none,
                cached_decoder: none,
                merged_decoder: none,
            },
            fire_red_asr: EncoderDecoder {
                encoder: none,
                decoder: none,
            },
            dolphin: one(none),
            zipformer_ctc: one(none),
            canary: Canary {
                encoder: none,
                decoder: none,
                src_lang: none,
                tgt_lang: none,
                use_pnc: 0,
            },
            wenet_ctc: one(none),
            omnilingual: one(none),
            medasr: one(none),
            funasr_nano: FunAsrNano {
                encoder_adaptor: none,
                llm: none,
                embedding: none,
                tokenizer: none,
                system_prompt: none,
                user_prompt: none,
                max_new_tokens: 0,
                temperature: 0.0,
                top_p: 0.0,
                seed: 0,
                language: none,
                itn: 0,
                hotwords: none,
            },
            fire_red_asr_ctc: one(none),
            qwen3_asr: Qwen3Asr {
                conv_frontend: none,
                encoder: none,
                decoder: none,
                tokenizer: none,
                max_total_len: 0,
                max_new_tokens: 0,
                temperature: 0.0,
                top_p: 0.0,
                seed: 0,
                hotwords: none,
            },
            cohere_transcribe: CohereTranscribe {
                encoder: none,
                decoder: none,
                language: none,
                use_punct: 0,
                use_itn: 0,
            },
        };
        match &spec.kind {
            ModelKind::Transducer {
                encoder,
                decoder,
                joiner,
            } => {
                model.transducer = Transducer {
                    encoder: strings.path(&spec.dir.join(encoder))?,
                    decoder: strings.path(&spec.dir.join(decoder))?,
                    joiner: strings.path(&spec.dir.join(joiner))?,
                };
            }
            ModelKind::NemoCtc { model: m } => {
                model.nemo_ctc = one(strings.path(&spec.dir.join(m))?)
            }
            ModelKind::ZipformerCtc { model: m } => {
                model.zipformer_ctc = one(strings.path(&spec.dir.join(m))?)
            }
            ModelKind::Whisper {
                encoder,
                decoder,
                language,
            } => {
                model.whisper.encoder = strings.path(&spec.dir.join(encoder))?;
                model.whisper.decoder = strings.path(&spec.dir.join(decoder))?;
                model.whisper.language = strings.text(language)?;
                model.whisper.task = strings.text("transcribe")?;
            }
            ModelKind::SenseVoice { model: m, language } => {
                model.sense_voice = SenseVoice {
                    model: strings.path(&spec.dir.join(m))?,
                    language: strings.text(language)?,
                    use_itn: 0,
                };
            }
            ModelKind::Moonshine {
                preprocessor,
                encoder,
                uncached_decoder,
                cached_decoder,
                merged_decoder,
            } => {
                model.moonshine = Moonshine {
                    preprocessor: path(&mut strings, preprocessor)?,
                    encoder: strings.path(&spec.dir.join(encoder))?,
                    uncached_decoder: path(&mut strings, uncached_decoder)?,
                    cached_decoder: path(&mut strings, cached_decoder)?,
                    merged_decoder: path(&mut strings, merged_decoder)?,
                };
            }
        }
        let config = RecognizerConfig {
            feat_config: FeatureConfig {
                sample_rate: SAMPLE_RATE as i32,
                feature_dim: 80,
            },
            model_config: model,
            lm_config: LmConfig {
                model: none,
                scale: 1.0,
            },
            decoding_method: strings.text("greedy_search")?,
            max_active_paths: 4,
            hotwords_file: none,
            hotwords_score: 1.5,
            rule_fsts: none,
            rule_fars: none,
            blank_penalty: 0.0,
            hr: HomophoneReplacer {
                dict_dir: none,
                lexicon: none,
                rule_fsts: none,
            },
            ctc_fst_decoder_config: CtcFstDecoder {
                graph: none,
                max_active: 3000,
            },
        };
        // SAFETY: every pointer in `config` is null or a C string in `strings`, alive for the
        // call (sherpa-onnx copies what it keeps); null on failure.
        let ptr = unsafe { (self.0.create_recognizer)(&config) };
        drop(strings);
        if ptr.is_null() {
            return Err(Error::Model(format!(
                "{} is not a speech model sherpa-onnx can load",
                spec.dir.display()
            )));
        }
        Ok(Model(Arc::new(RecognizerPtr {
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

/// # Safety
/// `p` is null or a zero-terminated string.
unsafe fn text(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: as the caller says.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// C strings kept alive while a config that points into them is used.
#[derive(Default)]
struct Strings(Vec<CString>);

impl Strings {
    fn keep(&mut self, s: CString) -> *const c_char {
        // The CString's buffer stays where it is when the Vec grows.
        let p = s.as_ptr();
        self.0.push(s);
        p
    }

    fn text(&mut self, s: &str) -> Result<*const c_char, Error> {
        let s = CString::new(s).map_err(|e| Error::Model(e.to_string()))?;
        Ok(self.keep(s))
    }

    fn path(&mut self, p: &Path) -> Result<*const c_char, Error> {
        if !p.exists() {
            return Err(Error::Model(format!("{} is missing", p.display())));
        }
        let s = crate::platform::native_path(p).ok_or_else(|| {
            Error::Model(format!(
                "{} cannot be opened by the speech library: move the data folder to a path with \
                 only Latin letters",
                p.display()
            ))
        })?;
        Ok(self.keep(s))
    }
}

/// Which model files a speech model has, relative to its folder.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelKind {
    Transducer {
        encoder: String,
        decoder: String,
        joiner: String,
    },
    NemoCtc {
        model: String,
    },
    ZipformerCtc {
        model: String,
    },
    Whisper {
        encoder: String,
        decoder: String,
        /// Language code (`en`, `de`...), empty = detect.
        language: String,
    },
    SenseVoice {
        model: String,
        /// `zh`, `en`, `ko`, `ja`, `yue` or `auto`.
        language: String,
    },
    Moonshine {
        preprocessor: Option<String>,
        encoder: String,
        uncached_decoder: Option<String>,
        cached_decoder: Option<String>,
        merged_decoder: Option<String>,
    },
}

/// A speech model on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    pub dir: PathBuf,
    pub kind: ModelKind,
    /// The token list, relative to `dir`.
    pub tokens: String,
    /// Threads for one decode.
    pub threads: i32,
}

impl ModelSpec {
    /// Works out the model in `dir` from its file names, as sherpa-onnx's releases name them
    /// (int8 files preferred): a transducer has encoder, decoder and joiner; Whisper and
    /// Moonshine have an encoder and decoders; SenseVoice and CTC models one `model` file. The
    /// folder name tells Whisper, Moonshine, SenseVoice and CTC models apart. `language` goes to
    /// the models that take one (Whisper, SenseVoice).
    pub fn detect(dir: &Path, language: &str, threads: i32) -> Result<Self, Error> {
        let files: Vec<String> = std::fs::read_dir(dir)
            .map_err(|e| Error::Model(format!("{}: {e}", dir.display())))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        // The file with `part` in its name, int8 first.
        let find = |part: &str| -> Option<String> {
            let mut found: Vec<&String> = files
                .iter()
                .filter(|f| f.contains(part) && (f.ends_with(".onnx") || f.ends_with(".ort")))
                .collect();
            found.sort_by_key(|f| (!f.contains("int8"), f.len()));
            found.first().map(|f| f.to_string())
        };
        let missing = || Error::Model(format!("{} is not a speech model", dir.display()));
        let tokens = files
            .iter()
            .find(|f| f.as_str() == "tokens.txt" || f.ends_with("-tokens.txt"))
            .cloned()
            .ok_or_else(missing)?;
        let kind = if name.contains("whisper") {
            ModelKind::Whisper {
                encoder: find("encoder").ok_or_else(missing)?,
                decoder: find("decoder").ok_or_else(missing)?,
                language: language.to_string(),
            }
        } else if name.contains("moonshine") {
            ModelKind::Moonshine {
                preprocessor: find("preprocess"),
                encoder: find("encode").ok_or_else(missing)?,
                uncached_decoder: find("uncached_decode"),
                cached_decoder: find("cached_decode").filter(|f| !f.contains("uncached")),
                merged_decoder: find("decoder_model_merged"),
            }
        } else if name.contains("sense-voice") {
            ModelKind::SenseVoice {
                model: find("model").ok_or_else(missing)?,
                language: language.to_string(),
            }
        } else if let (Some(encoder), Some(decoder), Some(joiner)) =
            (find("encoder"), find("decoder"), find("joiner"))
        {
            ModelKind::Transducer {
                encoder,
                decoder,
                joiner,
            }
        } else if name.contains("zipformer") {
            ModelKind::ZipformerCtc {
                model: find("model").ok_or_else(missing)?,
            }
        } else {
            ModelKind::NemoCtc {
                model: find("model").ok_or_else(missing)?,
            }
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            kind,
            tokens,
            threads,
        })
    }
}

struct RecognizerPtr {
    api: Arc<Api>,
    ptr: Handle,
}

// SAFETY: an offline recognizer is read-only after creation; each decode uses its own stream.
unsafe impl Send for RecognizerPtr {}
unsafe impl Sync for RecognizerPtr {}

impl Drop for RecognizerPtr {
    fn drop(&mut self) {
        // SAFETY: made by SherpaOnnxCreateOfflineRecognizer; streams hold an Arc to it.
        unsafe { (self.api.destroy_recognizer)(self.ptr) };
    }
}

/// A loaded speech model.
#[derive(Clone)]
pub struct Model(Arc<RecognizerPtr>);

impl Model {
    /// A recognizer for one utterance.
    pub fn recognizer(&self) -> Recognizer {
        Recognizer {
            model: self.0.clone(),
            audio: Vec::new(),
        }
    }
}

/// One utterance: the audio while the button is held, decoded once on [`Recognizer::finish`].
pub struct Recognizer {
    model: Arc<RecognizerPtr>,
    audio: Vec<f32>,
}

impl Recognizer {
    /// Keeps 16 kHz mono samples; nothing is decoded yet.
    pub fn feed(&mut self, samples: &[i16]) {
        self.audio
            .extend(samples.iter().map(|&s| f32::from(s) / 32768.0));
    }

    /// Decodes everything fed so far and starts over: the text heard, lower case, empty when
    /// nothing was.
    pub fn finish(&mut self) -> String {
        let audio = std::mem::take(&mut self.audio);
        if audio.is_empty() {
            return String::new();
        }
        let api = &self.model.api;
        // SAFETY: the recognizer lives while `self.model` does; the stream is made, used and
        // destroyed here; `audio` outlives the call that reads it; the JSON is freed after use.
        unsafe {
            let stream = (api.create_stream)(self.model.ptr);
            if stream.is_null() {
                log::error!("voice: sherpa-onnx could not make a stream");
                return String::new();
            }
            (api.accept_waveform)(
                stream,
                SAMPLE_RATE as i32,
                audio.as_ptr(),
                audio.len() as i32,
            );
            (api.decode)(self.model.ptr, stream);
            let json = (api.result_json)(stream);
            let result = text(json);
            if !json.is_null() {
                (api.free_json)(json);
            }
            (api.destroy_stream)(stream);
            parse(&result)
        }
    }
}

#[derive(Deserialize)]
struct Output {
    #[serde(default)]
    text: String,
}

/// The text of a result JSON, trimmed and in lower case; word-start marks some models leave in
/// (`▁`) become spaces.
fn parse(json: &str) -> String {
    serde_json::from_str::<Output>(json)
        .map(|o| o.text.replace('\u{2581}', " ").trim().to_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_text() {
        assert_eq!(
            parse(r#"{"text": " Landing Gear ", "tokens": []}"#),
            "landing gear"
        );
        assert_eq!(parse(r#"{"tokens": []}"#), "");
        assert_eq!(parse("not json"), "");
        assert_eq!(parse(r#"{"text": "▁조국이 당신을"}"#), "조국이 당신을");
    }

    #[test]
    fn config_layout_matches_c_api() {
        // sizeof/offsetof from c-api.h of VERSION, compiled for x64 with MSVC.
        assert_eq!(size_of::<ModelConfig>(), 504);
        assert_eq!(size_of::<RecognizerConfig>(), 624);
        assert_eq!(std::mem::offset_of!(ModelConfig, sense_voice), 152);
        assert_eq!(std::mem::offset_of!(ModelConfig, cohere_transcribe), 472);
        assert_eq!(std::mem::offset_of!(RecognizerConfig, lm_config), 512);
        assert_eq!(std::mem::offset_of!(RecognizerConfig, hr), 584);
    }
}
