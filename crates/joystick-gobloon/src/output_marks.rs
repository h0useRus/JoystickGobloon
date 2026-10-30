//! Which virtual outputs the loaded profile writes to, for the Virtual joysticks page, Quickmap
//! and the output lists of map-to-vjoy and map-to-gamepad: free, used, or used by several
//! inputs that run together, with the inputs in the tooltip.

use std::collections::HashMap;

use gob_core::{InputType, OutputUse, Profile, VirtualOutput};
use gob_i18n::I18n;

use crate::style::OutputMark;

/// At most this many inputs are listed in a tooltip.
const TOOLTIP_USES: usize = 8;

pub struct OutputMarks<'a> {
    profile: &'a Profile,
    /// File name of the profile, shown first in the tooltips.
    name: Option<&'a str>,
    usage: HashMap<VirtualOutput, Vec<OutputUse>>,
    /// Uses by this action are left out: the edited action does not mark its own output.
    skip: Option<&'a str>,
    /// Outputs with a mark the viewer switched off are hidden (used, conflict).
    shown: [bool; 2],
}

impl<'a> OutputMarks<'a> {
    /// `mode`: only bindings that run together with it; `None` takes every mode.
    pub fn new(profile: &'a Profile, name: Option<&'a str>, mode: Option<&str>) -> Self {
        Self {
            profile,
            name,
            usage: profile.output_usage(mode),
            skip: None,
            shown: [true; 2],
        }
    }

    /// Which marked outputs are shown: used, conflict ([`Self::visible`]).
    pub fn set_shown(&mut self, shown: [bool; 2]) {
        self.shown = shown;
    }

    pub fn name(&self) -> Option<&str> {
        self.name
    }

    pub fn profile(&self) -> &'a Profile {
        self.profile
    }

    pub fn skip(mut self, action: &'a str) -> Self {
        self.skip = Some(action);
        self
    }

    pub fn vjoy(device: u32, input_type: InputType, id: u32) -> VirtualOutput {
        VirtualOutput::Vjoy {
            device,
            input_type,
            id,
        }
    }

    pub fn gamepad(gamepad: u32, control: &str) -> VirtualOutput {
        VirtualOutput::Gamepad {
            gamepad,
            control: control.to_string(),
        }
    }

    fn uses(&self, output: &VirtualOutput) -> Vec<OutputUse> {
        self.usage
            .get(output)
            .into_iter()
            .flatten()
            .filter(|u| Some(u.action.as_str()) != self.skip)
            .cloned()
            .collect()
    }

    /// The mark and the number of bindings writing `output`.
    pub fn mark(&self, output: &VirtualOutput) -> (OutputMark, usize) {
        let uses = self.uses(output);
        let mark = if uses.is_empty() {
            OutputMark::Free
        } else if self.profile.uses_conflict(&uses) {
            OutputMark::Conflict
        } else {
            OutputMark::Used
        };
        (mark, uses.len())
    }

    /// Whether `output` is shown: free outputs always, marked ones while their tick is on.
    pub fn visible(&self, output: &VirtualOutput) -> bool {
        match self.mark(output).0 {
            OutputMark::Free => true,
            OutputMark::Used => self.shown[0],
            OutputMark::Conflict => self.shown[1],
        }
    }

    /// `title` (the output's name), whether it is free or used how often, then device · input
    /// · mode of each binding; the profile's name first when known.
    pub fn tooltip(&self, i18n: &I18n, output: &VirtualOutput, title: &str) -> String {
        let uses = self.uses(output);
        let mut lines = Vec::new();
        if let Some(name) = self.name {
            lines.push(i18n.trf("output-profile", &[("name", name.to_string().into())]));
        }
        lines.push(if uses.is_empty() {
            i18n.trf("output-free", &[("output", title.to_string().into())])
        } else {
            i18n.trf(
                "output-used",
                &[
                    ("output", title.to_string().into()),
                    ("n", uses.len().into()),
                ],
            )
        });
        if self.profile.uses_conflict(&uses) {
            lines.push(i18n.tr("output-conflict"));
        }
        for u in uses.iter().take(TOOLTIP_USES) {
            let Some(b) = self.profile.inputs.get(u.binding) else {
                continue;
            };
            let device = if b.device_id.eq_ignore_ascii_case(gob_core::KEYBOARD_GUID) {
                i18n.tr("keyboard-device")
            } else {
                self.profile
                    .device_name(&b.device_id)
                    .unwrap_or(&b.device_id)
                    .to_string()
            };
            let input = input_name(i18n, &b.input_type, &b.input_id);
            lines.push(format!("  {device} · {input} · {}", b.mode));
        }
        if uses.len() > TOOLTIP_USES {
            let more = uses.len() - TOOLTIP_USES;
            lines.push(format!(
                "  {}",
                i18n.trf("output-more", &[("n", more.into())])
            ));
        }
        lines.join("\n")
    }
}

/// A physical input's name as the Devices page shows it.
fn input_name(i18n: &I18n, input_type: &str, id: &str) -> String {
    let Ok(n) = id.trim().parse::<u32>() else {
        return id.to_string();
    };
    match input_type {
        "axis" => crate::devices_view::axis_name(i18n, n),
        "button" => i18n.trf("vjoy-button-n", &[("n", n.into())]),
        "hat" => i18n.trf("vjoy-hat-n", &[("n", n.into())]),
        "key" => crate::key_capture::name(gob_core::key_from_input_id(n)),
        _ => id.to_string(),
    }
}
