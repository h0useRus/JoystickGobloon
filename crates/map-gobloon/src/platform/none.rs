use std::path::{Path, PathBuf};

pub fn default_data_dir() -> Option<PathBuf> {
    None
}

pub fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
    Vec::new()
}

pub fn open_folder(_dir: &Path) {}
