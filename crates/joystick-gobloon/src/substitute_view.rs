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
            !d.id.eq_ignore_ascii_case(gob_core::LOGICAL_DEVICE_GUID)
                && !d.id.eq_ignore_ascii_case(gob_core::KEYBOARD_GUID)
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
            .default_pos(ctx.screen_rect().center());
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
mod tests {
    use super::*;
    use gob_input::DeviceInfo;

    fn live(guid: &str, name: &str) -> DeviceState {
        DeviceState::new(DeviceInfo {
            guid: guid.into(),
            name: name.into(),
            vendor_id: 0x231d,
            product_id: 0x0200,
            axes: vec![1],
            buttons: 1,
            hats: 0,
            ..Default::default()
        })
    }

    #[test]
    fn similar_names() {
        let nxt_l = "VKBsim Gladiator NXT EVO OT L";
        assert!(name_score(nxt_l, "VKBsim Gladiator EVO OT L").is_some());
        assert_eq!(
            name_score(nxt_l, "VKBsim Gladiator EVO R"),
            None,
            "other side"
        );
        assert_eq!(name_score("Thrustmaster T.16000M", "Saitek X52"), None);
        assert!(
            name_score(nxt_l, "VKBsim Gladiator EVO OT L")
                > name_score(nxt_l, "VKBsim Gladiator L")
        );
    }

    #[test]
    fn auto_map_skips_ambiguous_and_mapped_rows() {
        let profile = Profile {
            devices: vec![
                gob_core::Device {
                    id: "p1".into(),
                    name: "VKBsim Gladiator NXT L".into(),
                },
                gob_core::Device {
                    id: "p2".into(),
                    name: "Generic Throttle".into(),
                },
                gob_core::Device {
                    id: "p3".into(),
                    name: "MFG Crosswind".into(),
                },
            ],
            ..Default::default()
        };
        let devices = [
            live("d1", "VKBsim Gladiator EVO L"),
            live("d2", "Generic Throttle A"),
            live("d3", "Generic Throttle B"),
        ];
        let mut dialog = SubstituteDialog::new(&profile, &devices);
        assert!(dialog.rows.iter().all(|r| r.target.is_none()));
        assert_eq!(dialog.auto_map(), 1);
        assert_eq!(dialog.rows[0].target.as_deref(), Some("d1"));
        assert_eq!(dialog.rows[1].target, None, "two equally similar throttles");
        assert_eq!(dialog.rows[2].target, None, "no similar device");
    }

    #[test]
    fn maps_onto_devices_the_profile_already_knows() {
        // Gremlin on this PC already added the attached sticks; old entries remain.
        let dev = |id: &str, name: &str| gob_core::Device {
            id: id.into(),
            name: name.into(),
        };
        let profile = Profile {
            devices: vec![
                dev("old-l", " VKBsim Gladiator EVO OT L  "),
                dev("old-r", " VKBsim Gladiator EVO  R  "),
                dev("new-l", " VKBsim Gladiator EVO OT L  "),
                dev("new-r", " VKBsim Gladiator EVO R  "),
            ],
            ..Default::default()
        };
        let devices = [
            live("new-l", "VKBsim Gladiator EVO OT L"),
            live("new-r", "VKBsim Gladiator EVO R"),
        ];
        let mut dialog = SubstituteDialog::new(&profile, &devices);
        for r in &mut dialog.rows {
            r.target = None;
        }
        assert_eq!(dialog.auto_map(), 2);
        assert_eq!(dialog.rows[0].target.as_deref(), Some("new-l"));
        assert_eq!(dialog.rows[1].target.as_deref(), Some("new-r"));
    }

    #[test]
    fn aliases_profile_devices_by_name() {
        let profile = Profile {
            devices: vec![
                gob_core::Device {
                    id: "7D12D5C0-43EA-11F0-800A-444553540000".into(),
                    name: " VKBsim Gladiator EVO OT L  ".into(),
                },
                gob_core::Device {
                    id: "ec8bbeb0-4009-11f0-8002-444553540000".into(),
                    name: " VKBsim Gladiator EVO  R  ".into(),
                },
            ],
            ..Default::default()
        };
        let devices = [
            live(
                "a2196530-a616-11f0-8012-444553540000",
                "VKBsim Gladiator EVO OT L",
            ),
            live(
                "a21805a0-a616-11f0-8006-444553540000",
                "VKBsim Gladiator EVO R",
            ),
        ];
        let aliases = auto_aliases(&profile, &devices);
        assert_eq!(aliases.len(), 2);
        assert_eq!(
            aliases["7d12d5c0-43ea-11f0-800a-444553540000"],
            "a2196530-a616-11f0-8012-444553540000"
        );
        assert_eq!(
            aliases["ec8bbeb0-4009-11f0-8002-444553540000"],
            "a21805a0-a616-11f0-8006-444553540000"
        );
    }
}
