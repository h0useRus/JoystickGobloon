//! Where voice commands find what they need: libvosk next to the exe (or in the voice folder) and
//! the speech models in `<data folder>/voice`, downloaded there by gob-voice's `models`.

use std::path::PathBuf;

use gob_engine::VoiceConfig;

use crate::settings::Settings;

/// Utterances heard less sure than this count as not recognized.
const MIN_CONFIDENCE: f32 = 0.5;

/// The folder of the speech models (and of libvosk when it is not next to the exe).
pub fn dir() -> Option<PathBuf> {
    crate::settings::data_dir().map(|d| d.join("voice"))
}

/// How voice commands run with these settings; `None` while the model of the language is not
/// installed (voice-input actions then do nothing).
pub fn config(settings: &Settings) -> Option<VoiceConfig> {
    let dir = dir()?;
    let Some(model) = gob_voice::models::for_language(&settings.language) else {
        log::info!("voice commands: no speech model for {}", settings.language);
        return None;
    };
    let Some(path) = gob_voice::models::installed(&dir, model) else {
        log::info!(
            "voice commands: speech model {} not installed in {}",
            model.name,
            dir.display()
        );
        return None;
    };
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from));
    Some(VoiceConfig {
        library_dirs: exe_dir.into_iter().chain([dir]).collect(),
        model: path,
        gain: 1.0,
        min_confidence: MIN_CONFIDENCE,
    })
}
