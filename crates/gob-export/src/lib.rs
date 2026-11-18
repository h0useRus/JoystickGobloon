//! Game export: plugins that turn a profile's virtual outputs into a game's own bindings.
//!
//! A plugin is a folder in `export/`: `plugin.gob` (TOML: id, name, version, supported game
//! versions), `plugin.luau` (hooks: targets, version, read, write), `catalog/<version>.gob` (the
//! game's actions per game version) and `renames.gob` (action ids the game changed). The code runs
//! in a sandboxed Luau state like profile scripts: no files, programs or network, every hook
//! stopped after [`CALL_BUDGET`]. Plugins only turn text into text; the host reads and writes the
//! game's files. Game names, formats and paths live only in the plugins.

pub mod catalog;
mod helpers;
pub mod host;
mod plugin;
pub mod types;
pub mod version;
pub mod xml;

use std::path::{Path, PathBuf};

pub use catalog::{ActionKind, Catalog, Catalogs, Fit, GameAction, Rename, Renames};
pub use helpers::parse_ini;
pub use plugin::{
    API, CALL_BUDGET, CATALOG_DIR, CODE, Hook, LOAD_BUDGET, MANIFEST, Manifest, Plugin,
    PluginError, RENAMES,
};
pub use types::*;

/// Where a plugin folder came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Installed with the program.
    Bundled,
    /// Put there by the user.
    User,
}

/// A plugin folder found by [`discover`].
#[derive(Debug)]
pub struct Found {
    pub dir: PathBuf,
    pub origin: Origin,
    pub plugin: Result<Plugin, PluginError>,
}

/// Loads every plugin folder (a folder holding `plugin.gob`) in `dirs`, in order. A plugin whose
/// id an earlier one already has is not loaded.
pub fn discover(dirs: &[(PathBuf, Origin)]) -> Vec<Found> {
    let mut found: Vec<Found> = Vec::new();
    for (dir, origin) in dirs {
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
            let plugin = Plugin::load(&folder).and_then(|p| match taken(&found, p.id()) {
                Some(other) => Err(PluginError {
                    script: p.id().to_string(),
                    line: None,
                    message: format!("the plugin in {} has the same id", other.display()),
                }),
                None => Ok(p),
            });
            if let Err(e) = &plugin {
                log::warn!("export plugin {}: {e}", folder.display());
            }
            found.push(Found {
                dir: folder,
                origin: *origin,
                plugin,
            });
        }
    }
    found
}

fn taken<'a>(found: &'a [Found], id: &str) -> Option<&'a Path> {
    found
        .iter()
        .find(|f| f.plugin.as_ref().is_ok_and(|p| p.id() == id))
        .map(|f| f.dir.as_path())
}
