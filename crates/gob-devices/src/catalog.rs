//! Every device plugin found: `devices/` next to the program, then the user's own folder
//! (a plugin there with the same id replaces the shipped one).

use std::path::{Path, PathBuf};

use crate::inputs::Inputs;
use crate::plugin::{DevicePlugin, MANIFEST};

#[derive(Debug, Default)]
pub struct Catalog {
    pub plugins: Vec<DevicePlugin>,
    /// Plugin folders that did not load, with why.
    pub errors: Vec<(PathBuf, String)>,
}

impl Catalog {
    /// Loads every folder with a `device.gob` in `dirs`, in order; a later plugin replaces an
    /// earlier one with the same id.
    pub fn load(dirs: &[PathBuf]) -> Self {
        let mut catalog = Self::default();
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            let mut folders: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join(MANIFEST).is_file())
                .collect();
            folders.sort();
            for folder in folders {
                match DevicePlugin::load(&folder) {
                    Ok(plugin) => catalog.add(plugin),
                    Err(e) => catalog.errors.push((folder, e)),
                }
            }
        }
        catalog
    }

    pub fn add(&mut self, plugin: DevicePlugin) {
        self.plugins.retain(|p| p.id() != plugin.id());
        self.plugins.push(plugin);
    }

    pub fn get(&self, id: &str) -> Option<&DevicePlugin> {
        self.plugins.iter().find(|p| p.id() == id)
    }

    /// The plugin fitting a connected device best, if any.
    pub fn find(&self, vid: u16, pid: u16, name: &str) -> Option<&DevicePlugin> {
        self.plugins
            .iter()
            .filter_map(|p| p.score(vid, pid, name).map(|s| (s, p)))
            .max_by_key(|(s, _)| *s)
            .map(|(_, p)| p)
    }

    /// Puts the user's taught numbering (`<dir>/<plugin id>.gob`) over every plugin's defaults.
    /// Returns the files that could not be read.
    pub fn load_taught(&mut self, dir: &Path) -> Vec<(PathBuf, String)> {
        let mut errors = Vec::new();
        for plugin in &mut self.plugins {
            let path = taught_path(dir, plugin.id());
            match Inputs::load(&path) {
                Ok(taught) => plugin.set_taught(&taught),
                Err(e) => errors.push((path, e)),
            }
        }
        errors
    }
}

/// Where the user's taught numbering of a plugin lives.
pub fn taught_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.gob"))
}
