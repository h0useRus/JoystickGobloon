//! Sharing a device's picture and mapping: a plugin folder packed into one .zip file (the
//! folder inside, so unpacking it by hand gives a plugin folder too) and unpacked into the
//! user's plugins. Only a plugin's own kinds of files are taken from a file, each inside the
//! plugin's folder and not too big; it must load as a plugin before it goes in.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use gob_devices::DevicePlugin;
use gob_devices::plugin::MANIFEST;

use super::store::Store;

/// The largest file taken from a shared file, and all of them together.
const FILE_MAX: u64 = 32 << 20;
const TOTAL_MAX: u64 = 64 << 20;
/// What a plugin folder holds.
const KINDS: &[&str] = &["gob", "png", "jpg", "jpeg", "svg", "txt", "md"];

/// Packs plugin `id`'s folder into `to`.
pub fn export(store: &Store, id: &str, to: &Path) -> Result<(), String> {
    let plugin = store.get(id).ok_or_else(|| format!("no device {id}"))?;
    let file = std::fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut files = Vec::new();
    collect(&plugin.dir, &plugin.dir, &mut files).map_err(|e| e.to_string())?;
    files.sort();
    for relative in files {
        let name = format!("{id}/{}", relative.to_string_lossy().replace('\\', "/"));
        let bytes = std::fs::read(plugin.dir.join(&relative)).map_err(|e| e.to_string())?;
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(())
}

/// The plugin files below `dir` (relative to `root`): its kinds only, no backups.
fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if kind_ok(&path) {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
    Ok(())
}

fn kind_ok(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| KINDS.contains(&e.to_lowercase().as_str()))
}

/// A shared file unpacked and loaded, waiting to go in.
pub struct Unpacked {
    /// Where it was unpacked (removed when it goes in or is dropped).
    temp: PathBuf,
    pub id: String,
    pub name: String,
    /// The user's plugin it replaces.
    pub replaces: Option<PathBuf>,
    target: PathBuf,
}

/// Unpacks shared file `from` next to the user's plugins and checks it loads.
pub fn unpack(store: &Store, from: &Path) -> Result<Unpacked, String> {
    let file = std::fs::File::open(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", from.display()))?;
    // The plugin's folder: where the shallowest device.gob is.
    let mut prefix: Option<PathBuf> = None;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        if path.file_name().is_some_and(|n| n == MANIFEST) {
            let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
            if prefix
                .as_ref()
                .is_none_or(|p| parent.components().count() < p.components().count())
            {
                prefix = Some(parent);
            }
        }
    }
    let prefix = prefix.ok_or_else(|| format!("{}: no {MANIFEST} in it", from.display()))?;
    let temp = store
        .write_dir
        .join(format!(".import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let unpacked = (|| {
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(path) = entry.enclosed_name() else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(&prefix) else {
                continue;
            };
            if entry.is_dir() || !kind_ok(relative) {
                continue;
            }
            let too_big = || format!("{}: too big", from.display());
            if entry.size() > FILE_MAX {
                return Err(too_big());
            }
            let target = temp.join(relative);
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            // The sizes a file declares may lie: what is read counts.
            let mut bytes = Vec::new();
            (&mut entry)
                .take(FILE_MAX + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            total += bytes.len() as u64;
            if bytes.len() as u64 > FILE_MAX || total > TOTAL_MAX {
                return Err(too_big());
            }
            std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
        }
        DevicePlugin::load(&temp).map_err(|e| format!("{}: {e}", from.display()))
    })();
    let plugin = match unpacked {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(e);
        }
    };
    let id = plugin.id().to_string();
    let replaces = store
        .get(&id)
        .map(|p| p.dir.clone())
        .filter(|d| d.starts_with(&store.write_dir));
    let target = replaces.clone().unwrap_or_else(|| {
        // A new plugin never takes the folder of another one (of another id).
        let base = store
            .write_dir
            .join(gob_devices::edit::slug(&plugin.manifest.brand))
            .join(gob_devices::edit::slug(&plugin.manifest.name));
        let mut target = base.clone();
        let mut n = 2;
        while target.exists() {
            let mut name = base.as_os_str().to_owned();
            name.push(format!("-{n}"));
            target = PathBuf::from(name);
            n += 1;
        }
        target
    });
    Ok(Unpacked {
        temp,
        id,
        name: format!("{} {}", plugin.manifest.brand, plugin.manifest.name),
        replaces,
        target,
    })
}

impl Unpacked {
    /// Puts the plugin in (in place of the one it replaces).
    pub fn install(self, store: &mut Store) -> Result<String, String> {
        if self.target.exists() {
            std::fs::remove_dir_all(&self.target)
                .map_err(|e| format!("{}: {e}", self.target.display()))?;
        }
        if let Some(dir) = self.target.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::rename(&self.temp, &self.target)
            .map_err(|e| format!("{}: {e}", self.target.display()))?;
        store.reload();
        log::info!("device plugin {} imported", self.id);
        Ok(self.id.clone())
    }

    /// Drops it.
    pub fn discard(self) {}
}

impl Drop for Unpacked {
    /// The unpacked copy goes (gone already when it went in).
    fn drop(&mut self) {
        if self.temp.exists() {
            let _ = std::fs::remove_dir_all(&self.temp);
        }
    }
}
