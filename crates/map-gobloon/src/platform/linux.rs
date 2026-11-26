use std::path::{Path, PathBuf};

pub fn default_data_dir() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config.join("JoystickGobloon"))
}

/// Noto CJK where distributions put it.
pub fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
    [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
    .map(|p| vec![("cjk", p)])
    .unwrap_or_default()
}

pub fn open_folder(dir: &Path) {
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
}
