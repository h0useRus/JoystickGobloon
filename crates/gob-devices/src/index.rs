//! The list of device plugins the program can download (`plugins/devices/index.gob` in the
//! source): each plugin's id, name, the devices it fits and its files with their sizes and
//! SHA-256, so a connected device without a plugin can get one and every downloaded file is
//! checked. The program ships without plugins; [`Index::build`] makes the list from the
//! plugins' folders (example `device_index`), a test keeps it current.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalog::Catalog;
use crate::plugin::{Match, fit};

/// The index's file name, in the plugins' root folder.
pub const INDEX: &str = "index.gob";

/// Most bytes one plugin file may have.
pub const FILE_MAX: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Index {
    #[serde(default, rename = "device")]
    pub devices: Vec<Entry>,
}

/// One downloadable plugin.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub brand: String,
    #[serde(default)]
    pub version: String,
    /// Its folder under the plugins' root (`vkb/gladiator-nxt-evo-l`).
    pub path: String,
    #[serde(default, rename = "match")]
    pub matches: Vec<Match>,
    #[serde(default, rename = "file")]
    pub files: Vec<File>,
}

/// One file of a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct File {
    /// Its path in the plugin's folder, with `/` (`views/main.png`).
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

impl Index {
    pub fn parse(text: &str) -> Result<Self, String> {
        let index: Self = toml::from_str(text).map_err(|e| e.message().to_string())?;
        for e in &index.devices {
            if !is_plain(&e.path) {
                return Err(format!("{}: folder `{}` is not a plain path", e.id, e.path));
            }
            if let Some(f) = e.files.iter().find(|f| !is_plain(&f.path)) {
                return Err(format!("{}: file `{}` is not a plain path", e.id, f.path));
            }
        }
        Ok(index)
    }

    pub fn to_text(&self) -> String {
        format!(
            "# The device plugins Joystick Gobloon downloads for connected devices.\n\
             # Made by `cargo run -p gob-devices --example device_index`; do not edit.\n\n{}",
            toml::to_string(self).unwrap_or_default()
        )
    }

    /// The list of every plugin under `root` (plugins that do not load are left out).
    pub fn build(root: &Path) -> Result<Self, String> {
        let catalog = Catalog::load(&[root.to_path_buf()]);
        let mut devices = Vec::new();
        for p in &catalog.plugins {
            let path = relative(root, &p.dir)?;
            let mut files = Vec::new();
            collect(&p.dir, &p.dir, &mut files)?;
            files.sort_by(|a, b| a.path.cmp(&b.path));
            devices.push(Entry {
                id: p.id().to_string(),
                name: p.manifest.name.clone(),
                brand: p.manifest.brand.clone(),
                version: p.manifest.version.clone(),
                path,
                matches: p.manifest.matches.clone(),
                files,
            });
        }
        devices.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Self { devices })
    }

    /// The plugin fitting a connected device best, if any (as [`Catalog::find`] chooses).
    pub fn find(&self, vid: u16, pid: u16, name: &str) -> Option<&Entry> {
        self.devices
            .iter()
            .filter_map(|e| fit(&e.matches, vid, pid, name).map(|f| (f, e)))
            .max_by_key(|(f, _)| *f)
            .map(|(_, e)| e)
    }
}

impl File {
    /// Whether `bytes` are this file (text files compared without carriage returns: a checkout
    /// on Windows may have them).
    pub fn matches(&self, bytes: &[u8]) -> bool {
        let (size, hash) = digest(&self.path, bytes);
        size == self.size && hash == self.sha256
    }
}

/// A plain relative path: names only, no `..`, root or drive (no `:` on any system: `C:/x` is
/// a plain name to Linux, and on Windows a `:` inside a name opens a file stream).
pub fn is_plain(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains(':')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn is_text(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or_default();
    matches!(ext, "gob" | "svg" | "txt" | "md")
}

/// Size and SHA-256 (lowercase hex) of a file's content as the repository has it.
fn digest(path: &str, bytes: &[u8]) -> (u64, String) {
    let normal;
    let bytes = if is_text(path) && bytes.contains(&b'\r') {
        normal = String::from_utf8_lossy(bytes)
            .replace("\r\n", "\n")
            .into_bytes();
        &normal[..]
    } else {
        bytes
    };
    let hash = Sha256::digest(bytes);
    let hex = hash.iter().map(|b| format!("{b:02x}")).collect();
    (bytes.len() as u64, hex)
}

fn relative(root: &Path, dir: &Path) -> Result<String, String> {
    let rel = dir
        .strip_prefix(root)
        .map_err(|_| format!("{} is not under {}", dir.display(), root.display()))?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Ok(parts.join("/"))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<File>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out)?;
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rel = relative(root, &path)?;
        let (size, sha256) = digest(&rel, &bytes);
        if size > FILE_MAX {
            return Err(format!("{}: over {FILE_MAX} bytes", path.display()));
        }
        out.push(File {
            path: rel,
            size,
            sha256,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paths_only() {
        assert!(is_plain("vkb/stick"));
        assert!(is_plain("views/main.png"));
        for bad in ["", "../x", "/x", "a/../b", "C:/x", "a\\b", "views/a.png:s"] {
            assert!(!is_plain(bad), "{bad}");
        }
    }

    #[test]
    fn text_files_match_without_carriage_returns() {
        let lf = b"id = \"x\"\nname = \"X\"\n";
        let (size, sha256) = digest("device.gob", lf);
        let f = File {
            path: "device.gob".into(),
            size,
            sha256,
        };
        assert!(f.matches(lf));
        assert!(f.matches(b"id = \"x\"\r\nname = \"X\"\r\n"));
        assert!(!f.matches(b"id = \"y\"\n"));
    }
}
