//! Voice commands: microphone capture (`audio`) and offline speech recognition with a fixed phrase
//! list (`vosk`, libvosk loaded at run time, so the program runs without it), with the speech
//! models per language downloaded on demand (`models`).

pub mod audio;
pub mod models;
mod platform;
mod service;
pub mod vosk;

pub use audio::{Capture, MAX_GAIN, Microphone, SAMPLE_RATE, microphones};
pub use models::ModelInfo;
pub use service::{VoiceConfig, VoiceEvent, VoiceService};
pub use vosk::{Heard, Model, Recognizer, Vosk};

/// Why voice input is not available.
#[derive(Debug)]
pub enum Error {
    /// libvosk was not found or is missing a function.
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
