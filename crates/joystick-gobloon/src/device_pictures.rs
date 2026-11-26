//! Device plugins (pictures of controllers with their controls placed by Map Gobloon) and their
//! pictures as textures. Plugins load from `devices/` next to the program (debug builds: the
//! repository's plugins), then the user's `<data dir>/devices`, which wins by id. The plugins
//! load again when one of their files changes, so a device mapped in Map Gobloon shows here
//! while both run.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use gob_devices::{Catalog, Control, Inputs, Palette};
use gob_input::DeviceInfo;

/// How often the plugin files are checked for changes.
const CHECK_EVERY: Duration = Duration::from_secs(2);

fn plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(debug_assertions) {
        dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices"));
    } else if let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("devices")))
    {
        dirs.push(exe);
    }
    if let Some(data) = crate::settings::data_dir() {
        dirs.push(data.join("devices"));
    }
    dirs
}

/// A device plugin's controls, as the viewer draws them.
pub struct Mapped {
    pub id: String,
    pub views: Vec<String>,
    pub controls: Vec<Control>,
    pub inputs: Inputs,
    pub credit: Option<String>,
}

#[derive(Default)]
pub struct DevicePictures {
    catalog: Option<Catalog>,
    /// When each plugin's files last changed, as loaded.
    stamps: Vec<(PathBuf, Option<SystemTime>)>,
    checked: Option<Instant>,
    /// (plugin id, view id, dark) -> texture and the picture's size in control units.
    textures: HashMap<(String, String, bool), (egui::TextureHandle, egui::Vec2)>,
}

impl DevicePictures {
    fn catalog(&mut self) -> &Catalog {
        let changed = self.checked.is_none_or(|t| t.elapsed() >= CHECK_EVERY) && {
            self.checked = Some(Instant::now());
            self.stamps.iter().any(|(dir, at)| stamp(dir) != *at)
        };
        if changed {
            self.catalog = None;
            self.textures.clear();
        }
        self.catalog.get_or_insert_with(|| {
            let catalog = Catalog::load(&plugin_dirs());
            for (dir, e) in &catalog.errors {
                log::warn!("device plugin {}: {e}", dir.display());
            }
            log::debug!("{} device plugins", catalog.plugins.len());
            self.stamps = catalog
                .plugins
                .iter()
                .map(|p| (p.dir.clone(), stamp(&p.dir)))
                .collect();
            catalog
        })
    }

    /// What the plugin for a connected device shows, if there is one.
    pub fn plugin_for(&mut self, info: &DeviceInfo) -> Option<Mapped> {
        if info.is_own() {
            return None;
        }
        let p = self
            .catalog()
            .find(info.vendor_id, info.product_id, &info.name)?;
        Some(Mapped {
            id: p.id().to_string(),
            views: p.manifest.views.iter().map(|v| v.id.clone()).collect(),
            controls: p.manifest.controls.clone(),
            inputs: p.inputs.clone(),
            credit: p.manifest.credit.clone(),
        })
    }

    /// A view's picture for the theme and its size in control units.
    pub fn picture(
        &mut self,
        ctx: &egui::Context,
        id: &str,
        view: &str,
        dark: bool,
    ) -> Option<(egui::TextureHandle, egui::Vec2)> {
        let key = (id.to_string(), view.to_string(), dark);
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let plugin = self.catalog.as_ref()?.get(id)?;
        let picture = plugin.pictures.get(view)?;
        let [_, _, w, h] = picture.bounds()?;
        let palette = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mut rgba = match picture.rasterize(&palette, 2.0) {
            Ok(rgba) => rgba,
            Err(e) => {
                log::warn!("device plugin {}: picture {view}: {e}", plugin.id());
                return None;
            }
        };
        // The window is transparent: near-black pixels would show what is behind it.
        rgba.lift_blacks();
        let [pw, ph] = rgba.size;
        let image =
            egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], &rgba.pixels);
        let texture = ctx.load_texture(
            format!("device-{}-{view}-{dark}", plugin.id()),
            image,
            egui::TextureOptions::LINEAR,
        );
        let entry = (texture, egui::vec2(w, h));
        self.textures.insert(key, entry.clone());
        Some(entry)
    }
}

/// When a plugin's control files last changed.
fn stamp(dir: &std::path::Path) -> Option<SystemTime> {
    [gob_devices::plugin::MANIFEST, gob_devices::plugin::INPUTS]
        .iter()
        .filter_map(|f| std::fs::metadata(dir.join(f)).ok()?.modified().ok())
        .max()
}
