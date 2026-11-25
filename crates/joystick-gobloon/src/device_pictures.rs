//! Device plugins for the app: the catalog (shipped `devices/` next to the program, then the
//! user's `<data dir>/devices`, with the user's taught numbering over the defaults) and the
//! pictures as textures, made once per view and theme.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use gob_devices::{Catalog, Control, DevicePlugin, Kind, Palette, Side};
use gob_i18n::I18n;

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

/// What a control part shows on its line.
#[derive(Default)]
pub struct PartLine {
    pub text: String,
    /// Held now.
    pub lit: bool,
    /// Nothing on it yet.
    pub empty: bool,
    /// Where the axis is now, -1 to 1, for axis parts that are read.
    pub value: Option<f32>,
}

/// Callouts of a view and which control part each of their lines is.
pub struct Callouts {
    pub list: Vec<style::Callout>,
    /// Per callout, per line: (control id, part).
    pub parts: Vec<Vec<(String, String)>>,
}

impl Callouts {
    /// The control part of a callout's line.
    pub fn part(&self, callout: usize, line: usize) -> Option<(&str, &str)> {
        let (c, p) = self.parts.get(callout)?.get(line)?;
        Some((c, p))
    }

    /// The controls a callout shows (one, or a group's).
    pub fn controls(&self, callout: usize) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::new();
        for (c, _) in self.parts.get(callout).into_iter().flatten() {
            if !ids.contains(&c.as_str()) {
                ids.push(c);
            }
        }
        ids
    }
}

/// A part's short name on its line: arrows for directions, words for the rest.
pub fn part_label(i18n: Option<&I18n>, part: &str) -> String {
    match part {
        "" => String::new(),
        "up" => style::icon::ARROW_UP.into(),
        "right" => style::icon::ARROW_RIGHT.into(),
        "down" => style::icon::ARROW_DOWN.into(),
        "left" => style::icon::ARROW_LEFT.into(),
        "x" => "X".into(),
        "y" => "Y".into(),
        other => i18n.map_or_else(
            || other.to_string(),
            |i| i.tr(&format!("device-part-{other}")),
        ),
    }
}

/// What a part's badge shows.
fn badge(kind: Kind, part: &str) -> String {
    match (kind, part) {
        (Kind::Trigger, "first") => "1".into(),
        (Kind::Trigger, "second") => "2".into(),
        (_, "fwd") => style::icon::CARET_UP.into(),
        (_, "back") => style::icon::CARET_DOWN.into(),
        (_, "cw") => style::icon::ARROW_CLOCKWISE.into(),
        (_, "ccw") => style::icon::ARROW_COUNTER_CLOCKWISE.into(),
        (_, "up") => style::icon::ARROW_UP.into(),
        (_, "down") => style::icon::ARROW_DOWN.into(),
        (_, "left") => style::icon::ARROW_LEFT.into(),
        (_, "right") => style::icon::ARROW_RIGHT.into(),
        _ => String::new(),
    }
}

/// A control's short name for a badge: kept when it fits ("F2"), else none (a dot).
fn short_name(name: &str) -> String {
    if name.chars().count() <= 3 {
        name.to_string()
    } else {
        String::new()
    }
}

fn side(s: Side) -> style::CalloutSide {
    match s {
        Side::Left => style::CalloutSide::Left,
        Side::Right => style::CalloutSide::Right,
    }
}

/// The callouts of `view`: one per control, drawn by its kind (a hat as a compass, stick axes
/// as a ring, one axis as a gauge, the rest as badges); with `merge`, the controls of one group
/// ("Base buttons") share one callout with a badge and a leader each. `line` gives what each
/// part shows.
pub fn callouts(
    plugin: &DevicePlugin,
    view: &str,
    i18n: Option<&I18n>,
    merge: bool,
    mut line: impl FnMut(&Control, &str) -> PartLine,
) -> Callouts {
    let mut out = Callouts {
        list: Vec::new(),
        parts: Vec::new(),
    };
    let mut groups: Vec<(String, Side, usize)> = Vec::new();
    for c in plugin.manifest.controls.iter().filter(|c| c.view == view) {
        let at = egui::pos2(c.at[0], c.at[1]);
        if merge && let Some(group) = &c.group {
            let n = match groups.iter().find(|(g, s, _)| g == group && *s == c.side) {
                Some((_, _, n)) => *n,
                None => {
                    out.list.push(style::Callout {
                        at,
                        side: side(c.side),
                        title: group.clone(),
                        ..Default::default()
                    });
                    out.parts.push(Vec::new());
                    groups.push((group.clone(), c.side, out.list.len() - 1));
                    out.list.len() - 1
                }
            };
            for part in c.parts() {
                let shown = line(c, part);
                let label = match part_label(i18n, part) {
                    l if l.is_empty() => c.name.clone(),
                    l => format!("{} {l}", c.name),
                };
                let callout = &mut out.list[n];
                callout.lit |= shown.lit;
                callout.lines.push(style::CalloutLine {
                    label,
                    badge: match badge(c.kind, part) {
                        b if b.is_empty() => short_name(&c.name),
                        b => b,
                    },
                    text: shown.text,
                    lit: shown.lit,
                    empty: shown.empty,
                    at: Some(at),
                });
                out.parts[n].push((c.id.clone(), part.to_string()));
            }
            // The group's spot for ordering: the middle of its controls.
            let spots: Vec<egui::Pos2> = out.list[n].lines.iter().filter_map(|l| l.at).collect();
            let y = spots.iter().map(|p| p.y).sum::<f32>() / spots.len().max(1) as f32;
            out.list[n].at.y = y;
            continue;
        }
        let parts = c.parts();
        let mini_stick = c.look == Some(gob_devices::Look::Stick) && c.kind == Kind::Hat;
        let glyph = match c.kind {
            _ if mini_stick => style::CalloutGlyph::Stick,
            Kind::Hat if parts.starts_with(&["up", "right", "down", "left"]) => {
                style::CalloutGlyph::Hat
            }
            Kind::Stick => style::CalloutGlyph::Stick,
            Kind::Axis => style::CalloutGlyph::Gauge,
            _ => style::CalloutGlyph::Badges,
        };
        let mut callout = style::Callout {
            at,
            side: side(c.side),
            title: c.name.clone(),
            glyph,
            ..Default::default()
        };
        let mut values = Vec::new();
        let mut known = false;
        for part in &parts {
            let shown = line(c, part);
            callout.lit |= shown.lit;
            // The glyph's axes: x, y and twist in that order, a push of a mini stick is no axis.
            if c.kind.is_axis_part(part) {
                known |= shown.value.is_some();
                values.push(shown.value.unwrap_or(0.0));
            }
            callout.lines.push(style::CalloutLine {
                label: part_label(i18n, part),
                badge: badge(c.kind, part),
                text: shown.text,
                lit: shown.lit,
                empty: shown.empty,
                at: None,
            });
        }
        if known {
            callout.axes = values;
        }
        // A mini stick on a hat: its dot leans the way the hat points.
        if mini_stick {
            let lit = |p: &str| {
                parts
                    .iter()
                    .position(|q| *q == p)
                    .is_some_and(|n| callout.lines[n].lit)
            };
            let axis = |minus: &str, plus: &str| match (lit(minus), lit(plus)) {
                (true, false) => -1.0,
                (false, true) => 1.0,
                _ => 0.0,
            };
            let (x, y) = (axis("left", "right"), axis("up", "down"));
            // A diagonal stays on the ring like a straight push.
            let k = if x != 0.0 && y != 0.0 {
                std::f32::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            callout.axes = vec![x * k, y * k];
        }
        out.list.push(callout);
        out.parts.push(
            parts
                .iter()
                .map(|p| (c.id.clone(), p.to_string()))
                .collect(),
        );
    }
    out
}
