//! "Map controls": re-target profile devices that are not connected onto
//! attached ones (DirectInput GUIDs differ between PCs).

use std::collections::HashMap;

use eframe::egui::{self, RichText};
use gob_core::{Profile, same_device_name};
use gob_i18n::I18n;
use gob_input::DeviceState;

use crate::style::{self, icon};

struct Row {
    old_id: String,
    old_name: String,
    target: Option<String>,
}

pub struct SubstituteDialog {
    rows: Vec<Row>,
    /// Attached physical devices: the candidates. One the profile already knows is a valid
    /// target too: the old device's bindings are merged into it.
    free: Vec<(String, String)>,
}

/// One accepted substitution: old GUID, new GUID, new device name.
pub type Substitution = (String, String, String);

/// Profile devices that no attached controller matches (the logical device never does).
pub fn missing_devices<'a>(
    profile: &'a Profile,
    devices: &[DeviceState],
) -> Vec<&'a gob_core::Device> {
    profile
        .devices
        .iter()
        .filter(|d| {
            !gob_core::is_builtin_device(&d.id)
                && !devices
                    .iter()
                    .any(|s| s.info.guid.eq_ignore_ascii_case(&d.id))
        })
        .collect()
}

impl SubstituteDialog {
    /// Pre-selects an attached device with the same name when it is unambiguous.
    pub fn new(profile: &Profile, devices: &[DeviceState]) -> Self {
        let free: Vec<&DeviceState> = devices.iter().filter(|s| !s.info.is_own()).collect();
        let free_ids = free
            .iter()
            .map(|s| (s.info.guid.clone(), s.info.name.clone()))
            .collect();
        let mut taken: Vec<&str> = Vec::new();
        let rows = missing_devices(profile, devices)
            .into_iter()
            .map(|d| {
                let same: Vec<&&DeviceState> = free
                    .iter()
                    .filter(|s| same_device_name(&s.info.name, &d.name))
                    .collect();
                let target = match same.as_slice() {
                    [one] if !taken.contains(&one.info.guid.as_str()) => {
                        taken.push(&one.info.guid);
                        Some(one.info.guid.clone())
                    }
                    _ => None,
                };
                Row {
                    old_id: d.id.clone(),
                    old_name: d.name.trim().to_string(),
                    target,
                }
            })
            .collect();
        Self {
            rows,
            free: free_ids,
        }
    }

    /// Fills rows the user left on "keep" with the attached device whose name is most
    /// similar; rows without a clear winner stay as they are. Returns how many were mapped.
    fn auto_map(&mut self) -> usize {
        let used = |rows: &[Row], g: &str| rows.iter().any(|r| r.target.as_deref() == Some(g));
        let mut pairs: Vec<(u32, usize, usize)> = Vec::new();
        for (ri, row) in self.rows.iter().enumerate() {
            if row.target.is_some() {
                continue;
            }
            for (di, (guid, name)) in self.free.iter().enumerate() {
                if used(&self.rows, guid) {
                    continue;
                }
                if let Some(score) = name_score(&row.old_name, name) {
                    pairs.push((score, ri, di));
                }
            }
        }
        pairs.sort_by_key(|p| std::cmp::Reverse(p.0));
        let mut mapped = 0;
        for &(score, ri, di) in &pairs {
            let guid = &self.free[di].0;
            if self.rows[ri].target.is_some() || used(&self.rows, guid) {
                continue;
            }
            // Another still free device scores the same for this row: ambiguous, leave it.
            let tie = pairs.iter().any(|&(s, r, d)| {
                s == score && r == ri && d != di && !used(&self.rows, &self.free[d].0)
            });
            if !tie {
                self.rows[ri].target = Some(guid.clone());
                mapped += 1;
            }
        }
        mapped
    }

    /// Returns the chosen substitutions when the user applies; `open` is cleared on apply or close.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &I18n,
        devices: &[DeviceState],
        open: &mut bool,
    ) -> Option<Vec<Substitution>> {
        let physical: Vec<&DeviceState> = devices.iter().filter(|d| !d.info.is_own()).collect();
        let keep = i18n.tr("subst-keep");
        let mut apply = false;
        let title = i18n.tr("subst-title");
        let window = egui::Window::new(&title)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center());
        style::dialog_window(ctx, window, &title, open, |ui| {
            egui::Grid::new("subst")
                .num_columns(3)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    for row in &mut self.rows {
                        ui.label(format!("{}  {}", icon::PLUGS, row.old_name))
                            .on_hover_text(&row.old_id);
                        ui.label(RichText::new(icon::ARROW_RIGHT).size(style::ICON_SIZE));
                        let selected = row
                            .target
                            .as_ref()
                            .and_then(|g| physical.iter().find(|d| &d.info.guid == g))
                            .map_or(keep.clone(), |d| {
                                format!("{}  {}", icon::JOYSTICK, d.info.name)
                            });
                        egui::ComboBox::from_id_salt(&row.old_id)
                            .selected_text(selected)
                            .width(260.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut row.target, None, &keep);
                                for d in &physical {
                                    let text = format!("{}  {}", icon::JOYSTICK, d.info.name);
                                    ui.selectable_value(
                                        &mut row.target,
                                        Some(d.info.guid.clone()),
                                        text,
                                    )
                                    .on_hover_text(&d.info.guid);
                                }
                            });
                        ui.end_row();
                    }
                });
            let unmapped = self.rows.iter().any(|r| r.target.is_none());
            let any = self.rows.iter().any(|r| r.target.is_some());
            let buttons = [
                style::DialogButton::primary(format!(
                    "{}  {}",
                    icon::CHECK,
                    i18n.tr("subst-apply")
                ))
                .enabled(any),
                style::DialogButton::secondary(format!(
                    "{}  {}",
                    icon::MAGIC_WAND,
                    i18n.tr("subst-auto")
                ))
                .enabled(unmapped)
                .tip(i18n.tr("subst-auto-tip")),
            ];
            match style::dialog_buttons(ui, &buttons) {
                Some(0) => apply = true,
                Some(_) => {
                    let mapped = self.auto_map();
                    log::info!("map controls: auto-mapped {mapped} device(s) by name");
                }
                None => {}
            }
        });
        if !apply {
            return None;
        }
        *open = false;
        Some(
            self.rows
                .iter()
                .filter_map(|r| {
                    let guid = r.target.as_ref()?;
                    let device = physical.iter().find(|d| &d.info.guid == guid)?;
                    Some((r.old_id.clone(), guid.clone(), device.info.name.clone()))
                })
                .collect(),
        )
    }
}

/// Similarity of two device names: shared words, `None` when they clearly differ
/// (too few shared words, or one is a left and the other a right stick).
fn name_score(a: &str, b: &str) -> Option<u32> {
    if same_device_name(a, b) {
        return Some(u32::MAX);
    }
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(String::from)
            .collect()
    };
    let (a, b) = (words(a), words(b));
    let side = |w: &[String]| {
        let has = |names: [&str; 2]| w.iter().any(|x| names.contains(&x.as_str()));
        match (has(["l", "left"]), has(["r", "right"])) {
            (true, false) => Some('l'),
            (false, true) => Some('r'),
            _ => None,
        }
    };
    if let (Some(x), Some(y)) = (side(&a), side(&b))
        && x != y
    {
        return None;
    }
    let shared = a.iter().filter(|w| b.contains(w)).count();
    let named = a
        .iter()
        .filter(|w| b.contains(w) && !["l", "r", "left", "right"].contains(&w.as_str()))
        .count();
    (named > 0 && shared * 2 >= a.len().max(b.len())).then_some(shared as u32)
}

/// Profile device GUID (lowercase) -> attached device with the same name, for
/// devices whose GUID differs on this PC. Ambiguous names are left out.
pub fn auto_aliases(profile: &Profile, devices: &[DeviceState]) -> HashMap<String, String> {
    let free: Vec<&DeviceState> = devices
        .iter()
        .filter(|s| {
            !s.info.is_own()
                && !profile
                    .devices
                    .iter()
                    .any(|d| d.id.eq_ignore_ascii_case(&s.info.guid))
        })
        .collect();
    let mut aliases = HashMap::new();
    for d in missing_devices(profile, devices) {
        let same: Vec<&&DeviceState> = free
            .iter()
            .filter(|s| same_device_name(&s.info.name, &d.name))
            .collect();
        if let [one] = same.as_slice()
            && !aliases.values().any(|g: &String| g == &one.info.guid)
        {
            aliases.insert(d.id.to_ascii_lowercase(), one.info.guid.clone());
        }
    }
    aliases
}

#[cfg(test)]
mod tests;
