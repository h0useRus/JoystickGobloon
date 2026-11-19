//! Voice commands: microphone capture (`audio`), offline speech recognition with one of two
//! engines, both loaded at run time so the program runs without them (`vosk`: listens only for
//! the phrases; `sherpa`: sherpa-onnx writes down any speech and `matcher` finds the phrase it
//! means), and the speech models per engine and language downloaded on demand (`models`).

pub mod audio;
pub mod download;
pub mod matcher;
pub mod models;
mod platform;
mod service;
pub mod sherpa;
pub mod tts;
pub mod vosk;

pub use audio::{Capture, MAX_GAIN, Microphone, SAMPLE_RATE, microphones};
pub use matcher::Heard;
pub use models::{Engine, ModelInfo};
pub use service::{VoiceConfig, VoiceEvent, VoiceService};
pub use sherpa::Sherpa;
pub use vosk::Vosk;

/// Why voice input is not available.
#[derive(Debug)]
pub enum Error {
    /// The speech library (sherpa-onnx or libvosk) was not found, is another version or is
    /// missing a function.
    Library(String),
    /// The speech model folder could not be loaded.
    Model(String),
    /// The microphone could not be opened.
    Audio(String),
    /// A speech model could not be downloaded or unpacked.
    Download(String),
    /// The download was cancelled.
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Library(e) => write!(f, "speech library: {e}"),
            Self::Model(e) => write!(f, "speech model: {e}"),
            Self::Audio(e) => write!(f, "microphone: {e}"),
            Self::Download(e) => write!(f, "speech model download: {e}"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}
