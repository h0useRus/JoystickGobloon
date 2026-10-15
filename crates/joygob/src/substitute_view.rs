//! Device substitution: re-target profile devices that are not connected onto
//! attached ones (DirectInput GUIDs differ between PCs).

use std::collections::HashMap;

use eframe::egui::{self, RichText};
use gob_core::{same_device_name, Profile};
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
}

/// One accepted substitution: old GUID, new GUID, new device name.
pub type Substitution = (String, String, String);

/// Profile devices that no attached controller matches.
pub fn missing_devices<'a>(
    profile: &'a Profile,
    devices: &[DeviceState],
) -> Vec<&'a gob_core::Device> {
    profile
        .devices
        .iter()
        .filter(|d| {
            !devices
                .iter()
                .any(|s| s.info.guid.eq_ignore_ascii_case(&d.id))
        })
        .collect()
}

impl SubstituteDialog {
    /// Pre-selects an attached device with the same name when it is unambiguous.
    pub fn new(profile: &Profile, devices: &[DeviceState]) -> Self {
        let free: Vec<&DeviceState> = devices
            .iter()
            .filter(|s| {
                !s.info.is_vjoy()
                    && !profile
                        .devices
                        .iter()
                        .any(|d| d.id.eq_ignore_ascii_case(&s.info.guid))
            })
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
        Self { rows }
    }

    /// Returns the chosen substitutions when the user applies; `open` is cleared on apply or close.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &I18n,
        devices: &[DeviceState],
        open: &mut bool,
    ) -> Option<Vec<Substitution>> {
        let physical: Vec<&DeviceState> = devices.iter().filter(|d| !d.info.is_vjoy()).collect();
        let keep = i18n.tr("subst-keep");
        let mut apply = false;
        egui::Window::new(i18n.tr("subst-title"))
            .open(open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
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
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    let any = self.rows.iter().any(|r| r.target.is_some());
                    apply = ui
                        .add_enabled(
                            any,
                            egui::Button::new(format!(
                                "{}  {}",
                                icon::CHECK,
                                i18n.tr("subst-apply")
                            )),
                        )
                        .clicked();
                });
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

/// Profile device GUID (lowercase) -> attached device with the same name, for
/// devices whose GUID differs on this PC. Ambiguous names are left out.
pub fn auto_aliases(profile: &Profile, devices: &[DeviceState]) -> HashMap<String, String> {
    let free: Vec<&DeviceState> = devices
        .iter()
        .filter(|s| {
            !s.info.is_vjoy()
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
        if let [one] = same.as_slice() {
            if !aliases.values().any(|g: &String| g == &one.info.guid) {
                aliases.insert(d.id.to_ascii_lowercase(), one.info.guid.clone());
            }
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
        })
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
