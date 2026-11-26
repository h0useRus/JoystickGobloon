//! The device plugins Map Gobloon shows and changes: shipped ones (`devices/` next to the
//! program) and the user's own (`<data dir>/devices`), in folders per brand. Changes always go
//! to the user's folder: a shipped plugin is copied there the first time it changes, and
//! Joystick Gobloon then uses the copy. Debug builds work on the repository's plugins instead.

use std::path::{Path, PathBuf};

use gob_devices::edit::{self, NewDevice};
use gob_devices::plugin::{INPUTS, MANIFEST};
use gob_devices::{Catalog, DevicePlugin, Inputs};

pub struct Store {
    /// Folders plugins load from, in order (a later plugin replaces one with the same id).
    pub dirs: Vec<PathBuf>,
    /// Where changes and new plugins go.
    pub write_dir: PathBuf,
    pub catalog: Catalog,
}

impl Store {
    pub fn new() -> Self {
        let (dirs, write_dir) = folders();
        Self::with(dirs, write_dir)
    }

    /// Plugins from `dirs`, changes into `write_dir`.
    pub fn with(dirs: Vec<PathBuf>, write_dir: PathBuf) -> Self {
        let catalog = Catalog::load(&dirs);
        Self {
            dirs,
            write_dir,
            catalog,
        }
    }

    pub fn reload(&mut self) {
        self.catalog = Catalog::load(&self.dirs);
    }

    pub fn get(&self, id: &str) -> Option<&DevicePlugin> {
        self.catalog.get(id)
    }

    /// Every brand, sorted.
    pub fn brands(&self) -> Vec<String> {
        let mut brands: Vec<String> = self
            .catalog
            .plugins
            .iter()
            .map(|p| p.manifest.brand.clone())
            .collect();
        brands.sort_by_key(|b| b.to_lowercase());
        brands.dedup();
        brands
    }

    /// The devices of `brand`, sorted by name: (id, name).
    pub fn devices_of(&self, brand: &str) -> Vec<(String, String)> {
        let mut devices: Vec<(String, String)> = self
            .catalog
            .plugins
            .iter()
            .filter(|p| p.manifest.brand == brand)
            .map(|p| (p.id().to_string(), p.manifest.name.clone()))
            .collect();
        devices.sort_by_key(|(_, name)| name.to_lowercase());
        devices
    }

    /// Whether plugin `id` changes in place (else a change copies it to the user's folder).
    pub fn is_own(&self, id: &str) -> bool {
        self.get(id)
            .is_some_and(|p| p.dir.starts_with(&self.write_dir))
    }

    /// The folder of plugin `id` that changes go to: a plugin outside the user's folder is
    /// copied there first (same brand/device folders).
    fn editable(&mut self, id: &str) -> Result<PathBuf, String> {
        let plugin = self.get(id).ok_or_else(|| format!("no device {id}"))?;
        let dir = plugin.dir.clone();
        if dir.starts_with(&self.write_dir) {
            return Ok(dir);
        }
        let relative = self
            .dirs
            .iter()
            .find_map(|root| dir.strip_prefix(root).ok())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(id));
        let target = self.write_dir.join(relative);
        copy_dir(&dir, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        self.load_plugin(&target)?;
        Ok(target)
    }

    fn load_plugin(&mut self, dir: &Path) -> Result<(), String> {
        let plugin = DevicePlugin::load(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        self.catalog.add(plugin);
        Ok(())
    }

    /// Writes plugin `id`'s numbering.
    pub fn save_inputs(&mut self, id: &str, inputs: &Inputs) -> Result<(), String> {
        let dir = self.editable(id)?;
        let path = dir.join(INPUTS);
        inputs
            .save(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        self.load_plugin(&dir)
    }

    /// Changes plugin `id`'s `device.gob` text with `change`.
    pub fn edit_manifest(
        &mut self,
        id: &str,
        change: impl FnOnce(&str) -> String,
    ) -> Result<(), String> {
        let dir = self.editable(id)?;
        let path = dir.join(MANIFEST);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let changed = change(&text);
        // A change that would not load is not written.
        toml::from_str::<gob_devices::Manifest>(&changed)
            .map_err(|e| format!("{}: {}", path.display(), e.message()))?;
        write_text(&path, &changed)?;
        self.load_plugin(&dir)
    }

    /// Makes a new plugin in `<user folder>/<brand>/<name>/` with `picture` as its only view.
    /// Returns its id.
    pub fn create(
        &mut self,
        brand: &str,
        name: &str,
        picture: &Path,
        device: Option<(u16, &str)>,
    ) -> Result<String, String> {
        let (brand_slug, name_slug) = (edit::slug(brand), edit::slug(name));
        if brand_slug.is_empty() || name_slug.is_empty() {
            return Err("empty brand or name".into());
        }
        let id = format!("{brand_slug}-{name_slug}");
        let dir = self.write_dir.join(&brand_slug).join(&name_slug);
        if dir.join(MANIFEST).exists() || self.get(&id).is_some() {
            return Err(format!("{} exists", dir.display()));
        }
        let extension = picture
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let image = format!("views/main.{extension}");
        let views = dir.join("views");
        std::fs::create_dir_all(&views).map_err(|e| format!("{}: {e}", views.display()))?;
        std::fs::copy(picture, dir.join(&image))
            .map_err(|e| format!("{}: {e}", picture.display()))?;
        let text = edit::new_manifest(&NewDevice {
            id: &id,
            name,
            brand,
            vid: device.map(|(vid, _)| vid),
            pid: None,
            product: device.map(|(_, product)| product),
            image: &image,
        });
        write_text(&dir.join(MANIFEST), &text)?;
        self.load_plugin(&dir)?;
        Ok(id)
    }

    /// Puts plugin `from`'s controls and inputs on plugin `to` in place of its own: each
    /// control keeps its place relative to the picture (mirrored left to right with `mirror`,
    /// for the other hand's device), on the view with the same id or else the first one.
    pub fn copy_controls(&mut self, from: &str, to: &str, mirror: bool) -> Result<(), String> {
        let source = self.get(from).ok_or_else(|| format!("no device {from}"))?;
        let target = self.get(to).ok_or_else(|| format!("no device {to}"))?;
        let size = |plugin: &DevicePlugin, view: &str| {
            plugin
                .pictures
                .get(view)
                .and_then(|p| p.bounds())
                .map(|[_, _, w, h]| [w, h])
        };
        let first_view = target
            .manifest
            .views
            .first()
            .map(|v| v.id.clone())
            .ok_or("the device has no picture")?;
        let old: Vec<String> = target
            .manifest
            .controls
            .iter()
            .map(|c| c.id.clone())
            .collect();
        let mut controls = Vec::new();
        for c in &source.manifest.controls {
            let view = if target.manifest.views.iter().any(|v| v.id == c.view) {
                c.view.clone()
            } else {
                first_view.clone()
            };
            let ([fw, fh], [tw, th]) = match (size(source, &c.view), size(target, &view)) {
                (Some(f), Some(t)) => (f, t),
                _ => continue,
            };
            let mut x = c.at[0] / fw * tw;
            let y = c.at[1] / fh * th;
            let mut side = c.side;
            if mirror {
                x = tw - x;
                side = match side {
                    gob_devices::Side::Left => gob_devices::Side::Right,
                    gob_devices::Side::Right => gob_devices::Side::Left,
                };
            }
            let mut c = c.clone();
            c.view = view;
            c.at = [x.round(), y.round()];
            c.side = side;
            controls.push(c);
        }
        let inputs = source.defaults.clone();
        self.edit_manifest(to, |text| {
            let mut text = text.to_string();
            for id in &old {
                text = edit::remove_control(&text, id);
            }
            for c in &controls {
                text = edit::set_control(&text, &c.id, c);
            }
            text
        })?;
        self.save_inputs(to, &inputs)
    }

    /// The folder to show for plugin `id` (the user's folder when there is none).
    pub fn folder_of(&self, id: Option<&str>) -> PathBuf {
        id.and_then(|id| self.get(id))
            .map(|p| p.dir.clone())
            .unwrap_or_else(|| self.write_dir.clone())
    }
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    gob_core::file::write_whole(path, text.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// (folders to load from, folder changes go to).
fn folders() -> (Vec<PathBuf>, PathBuf) {
    if cfg!(debug_assertions) {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices");
        let repo = repo.canonicalize().unwrap_or(repo);
        return (vec![repo.clone()], repo);
    }
    let mut dirs = Vec::new();
    if let Some(shipped) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("devices")))
    {
        dirs.push(shipped);
    }
    let user = crate::settings::data_dir()
        .map(|d| d.join("devices"))
        .unwrap_or_else(|| PathBuf::from("devices"));
    dirs.push(user.clone());
    (dirs, user)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
