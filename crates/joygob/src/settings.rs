use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    #[default]
    Dark,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub language: String,
    /// Reopened on startup.
    pub last_profile: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            language: gob_i18n::DEFAULT_LANG.into(),
            last_profile: None,
        }
    }
}

impl Settings {
    fn path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|p| {
            PathBuf::from(p)
                .join("JoystickGobloon")
                .join("settings.toml")
        })
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }
}
