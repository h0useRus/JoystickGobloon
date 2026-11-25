//! Device plugins for the app: the catalog (shipped `devices/` next to the program, then the
//! user's `<data dir>/devices`, with the user's taught numbering over the defaults) and the
//! pictures as textures, made once per view and theme.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use gob_devices::{Catalog, DevicePlugin, Palette, Side};

use crate::style;

/// Folders plugins load from; debug builds take the repository's plugins first.
pub fn plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(debug_assertions) {
        dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices"));
    }
    if let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("devices")))
    {
        dirs.push(exe);
    }
    if let Some(data) = user_dir() {
        dirs.push(data);
    }
    dirs
}

/// The user's plugins and taught numbering.
pub fn user_dir() -> Option<PathBuf> {
    crate::settings::data_dir().map(|d| d.join("devices"))
}

#[derive(Default)]
pub struct DevicePictures {
    catalog: Option<Catalog>,
    /// (plugin id, view id, dark) -> texture.
    textures: HashMap<(String, String, bool), egui::TextureHandle>,
}

impl DevicePictures {
    /// Every plugin, loaded the first time it is asked for.
    pub fn catalog(&mut self) -> &Catalog {
        self.catalog.get_or_insert_with(load)
    }

    /// Loads the plugins again (after a plugin's files changed).
    pub fn reload(&mut self) {
        self.catalog = None;
        self.textures.clear();
    }

    /// The view's picture for the current theme and its size in hotspot units.
    pub fn picture(
        &mut self,
        ctx: &egui::Context,
        plugin: &str,
        view: &str,
        dark: bool,
    ) -> Option<(egui::TextureHandle, egui::Vec2)> {
        let catalog = self.catalog.get_or_insert_with(load);
        let picture = catalog.get(plugin)?.pictures.get(view)?;
        let [_, _, w, h] = picture.bounds()?;
        let size = egui::vec2(w, h);
        let key = (plugin.to_string(), view.to_string(), dark);
        if let Some(texture) = self.textures.get(&key) {
            return Some((texture.clone(), size));
        }
        let palette = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        // Drawings rasterized sharper than their viewBox: they are scaled up on large windows.
        let rgba = match picture.rasterize(&palette, 2.0) {
            Ok(rgba) => rgba,
            Err(e) => {
                log::warn!("device plugin {plugin}: picture {view}: {e}");
                return None;
            }
        };
        let [pw, ph] = rgba.size;
        let mut pixels = rgba.pixels;
        lift_blacks(&mut pixels);
        let image = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], &pixels);
        let texture = ctx.load_texture(
            format!("device-{plugin}-{view}-{dark}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(key, texture.clone());
        Some((texture, size))
    }
}

/// The windows are transparent, and Windows shows what is behind them through pixels that are
/// (nearly) black: the darkest parts of a photo would show as specks. A floor a little above
/// black keeps them dark and opaque.
fn lift_blacks(pixels: &mut [u8]) {
    const FLOOR: u16 = 16;
    for p in pixels.as_chunks_mut::<4>().0 {
        let floor = (FLOOR * u16::from(p[3]) / 255) as u8;
        for c in &mut p[..3] {
            *c = (*c).max(floor);
        }
    }
}

fn load() -> Catalog {
    let mut catalog = Catalog::load(&plugin_dirs());
    for (dir, e) in &catalog.errors {
        log::warn!("device plugin {}: {e}", dir.display());
    }
    if let Some(dir) = user_dir() {
        for (path, e) in catalog.load_taught(&dir) {
            log::warn!("taught numbering {}: {e}", path.display());
        }
    }
    log::debug!("{} device plugins", catalog.plugins.len());
    catalog
}

/// One callout per control of `view`: its name, and the lines `lines` gives for it.
pub fn callouts(
    plugin: &DevicePlugin,
    view: &str,
    mut lines: impl FnMut(&gob_devices::Control) -> (Vec<String>, bool),
) -> Vec<style::Callout> {
    plugin
        .manifest
        .controls
        .iter()
        .filter(|c| c.view == view)
        .map(|c| {
            let (lines, lit) = lines(c);
            style::Callout {
                at: egui::pos2(c.at[0], c.at[1]),
                side: match c.side {
                    Side::Left => style::CalloutSide::Left,
                    Side::Right => style::CalloutSide::Right,
                },
                title: c.name.clone(),
                lines,
                group: c.group.clone(),
                lit,
            }
        })
        .collect()
}
