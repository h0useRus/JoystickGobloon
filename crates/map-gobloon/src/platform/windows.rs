use std::path::{Path, PathBuf};

pub fn default_data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("JoystickGobloon"))
}

pub fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
    let windows = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
    let dir = Path::new(&windows).join("Fonts");
    vec![
        ("cjk-zh", dir.join("msyh.ttc")),
        ("cjk-ko", dir.join("malgun.ttf")),
    ]
}

pub fn open_folder(dir: &Path) {
    let _ = std::process::Command::new("explorer.exe").arg(dir).spawn();
}
