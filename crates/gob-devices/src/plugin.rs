//! One device plugin: a folder with `device.gob` (identity, views, controls), `inputs.gob`
//! (the default numbering) and the views' SVG pictures.
//!
//! ```toml
//! id = "vkb-gladiator-nxt-evo-r"
//! name = "Gladiator NXT EVO (right)"
//! brand = "VKB"
//!
//! [[match]]
//! vid = 0x231D
//! name = "Gladiator EVO R"
//!
//! [[view]]
//! id = "main"
//! name = "Stick"
//! svg = "views/main.svg"
//!
//! [[control]]
//! id = "trigger"
//! name = "Trigger"
//! kind = "trigger"
//! at = [512, 330]
//! side = "left"
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::input::Input;
use crate::inputs::Inputs;

/// The device plugin format this host reads; a plugin asking for a newer one is not loaded.
pub const API: u32 = 1;
pub const MANIFEST: &str = "device.gob";
pub const INPUTS: &str = "inputs.gob";
/// The view a control is on when it names none.
pub const MAIN_VIEW: &str = "main";

/// Every part name a control may have; "" = a control with one part.
pub const PARTS: [&str; 16] = [
    "", "up", "right", "down", "left", "push", "cw", "ccw", "first", "second", "fwd", "back", "x",
    "y", "twist", "press",
];

/// What kind of control it is: sets its default parts and how its callout reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Button,
    /// Two-stage trigger: first, second.
    Trigger,
    /// Four-way hat: up, right, down, left (add `push` in `parts` when it clicks).
    Hat,
    /// Rotary encoder: cw, ccw.
    Encoder,
    /// Two-way toggle switch: up, down.
    Switch,
    /// Lever pushed forward or back: fwd, back.
    Lever,
    /// One axis (wheel, slider, throttle).
    Axis,
    /// Stick axes: x, y (add `twist` in `parts`).
    Stick,
}

impl Kind {
    pub fn default_parts(self) -> &'static [&'static str] {
        match self {
            Self::Button | Self::Axis => &[""],
            Self::Trigger => &["first", "second"],
            Self::Hat => &["up", "right", "down", "left"],
            Self::Encoder => &["cw", "ccw"],
            Self::Switch => &["up", "down"],
            Self::Lever => &["fwd", "back"],
            Self::Stick => &["x", "y"],
        }
    }

    /// Whether `part` is an axis (else a button or hat direction).
    pub fn is_axis_part(self, part: &str) -> bool {
        match self {
            Self::Axis => part.is_empty(),
            Self::Stick => matches!(part, "x" | "y" | "twist"),
            _ => false,
        }
    }
}

/// Which column a control's callout goes to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    #[default]
    Left,
    Right,
}

/// A connected device this plugin is for. VKB and Virpil users can change the product id in
/// their configurators, so a vendor id with a name part is enough.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Match {
    pub vid: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u16>,
    /// A part of the product name, ignoring case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Match {
    /// How well a device fits: `None` = not at all; more = closer.
    pub fn score(&self, vid: u16, pid: u16, name: &str) -> Option<u8> {
        if vid != self.vid {
            return None;
        }
        let pid_fits = match self.pid {
            Some(p) if p != pid => return None,
            Some(_) => true,
            None => false,
        };
        let name_fits = match &self.name {
            Some(part) if !name.to_lowercase().contains(&part.to_lowercase()) => return None,
            Some(_) => true,
            None => false,
        };
        if !pid_fits && !name_fits {
            return None;
        }
        Some(u8::from(pid_fits) * 2 + u8::from(name_fits))
    }
}

/// One picture of the device (a stick of a pair, the base, ...).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct View {
    pub id: String,
    pub name: String,
    /// Path of the SVG, relative to the plugin folder.
    pub svg: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Control {
    pub id: String,
    /// The product's own wording (not translated, like product names).
    pub name: String,
    pub kind: Kind,
    /// The parts when not the kind's default ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<String>>,
    #[serde(default = "main_view")]
    pub view: String,
    /// Hotspot, in the view's SVG units.
    pub at: [f32; 2],
    #[serde(default)]
    pub side: Side,
    /// Callout heading when several controls share one (e.g. "Base buttons").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

fn main_view() -> String {
    MAIN_VIEW.to_string()
}

impl Control {
    pub fn parts(&self) -> Vec<&str> {
        match &self.parts {
            Some(parts) => parts.iter().map(String::as_str).collect(),
            None => self.kind.default_parts().to_vec(),
        }
    }
}

/// `device.gob`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub brand: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default = "first_api")]
    pub api: u32,
    #[serde(default, rename = "match")]
    pub matches: Vec<Match>,
    #[serde(default, rename = "view")]
    pub views: Vec<View>,
    #[serde(default, rename = "control")]
    pub controls: Vec<Control>,
}

fn first_api() -> u32 {
    1
}

#[derive(Debug, Clone)]
pub struct DevicePlugin {
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// The plugin's default numbering (`inputs.gob`).
    pub defaults: Inputs,
    /// The numbering in use: the defaults with the user's taught controls over them.
    pub inputs: Inputs,
    /// View id -> its SVG text.
    pub svgs: BTreeMap<String, String>,
}

impl DevicePlugin {
    /// Loads the plugin in `dir`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"))
        };
        let manifest: Manifest =
            toml::from_str(&read(MANIFEST)?).map_err(|e| format!("{MANIFEST}: {}", e.message()))?;
        if manifest.api > API {
            return Err(format!(
                "needs device plugin API {}, this version of the program has {API}",
                manifest.api
            ));
        }
        let defaults = Inputs::load(&dir.join(INPUTS)).map_err(|e| format!("{INPUTS}: {e}"))?;
        let mut svgs = BTreeMap::new();
        for view in &manifest.views {
            svgs.insert(view.id.clone(), read(&view.svg)?);
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            manifest,
            inputs: defaults.clone(),
            defaults,
            svgs,
        })
    }

    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    pub fn control(&self, id: &str) -> Option<&Control> {
        self.manifest.controls.iter().find(|c| c.id == id)
    }

    /// Puts the user's taught numbering over the defaults (empty = the defaults again).
    pub fn set_taught(&mut self, taught: &Inputs) {
        self.inputs = self.defaults.clone();
        self.inputs.overlay(taught);
    }

    /// The input `part` of `control` sends.
    pub fn input_of(&self, control: &str, part: &str) -> Option<Input> {
        self.inputs.get(control, part)
    }

    /// The control and part sending `input`. A whole hat's reading is looked up by direction
    /// ([`Input::HatDirection`]).
    pub fn control_of(&self, input: Input) -> Option<(&Control, &str)> {
        self.manifest.controls.iter().find_map(|control| {
            control
                .parts()
                .into_iter()
                .find(|part| self.inputs.get(&control.id, part) == Some(input))
                .map(|part| (control, part))
        })
    }

    /// How well the device fits this plugin (see [`Match::score`]).
    pub fn score(&self, vid: u16, pid: u16, name: &str) -> Option<u8> {
        self.manifest
            .matches
            .iter()
            .filter_map(|m| m.score(vid, pid, name))
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching() {
        let by_name = Match {
            vid: 0x231D,
            pid: None,
            name: Some("Gladiator EVO R".into()),
        };
        assert_eq!(
            by_name.score(0x231D, 0x0200, " VKBsim Gladiator EVO R "),
            Some(1)
        );
        assert_eq!(
            by_name.score(0x231D, 0x3201, "VKBsim Gladiator EVO OT L"),
            None
        );
        assert_eq!(by_name.score(0x044F, 0x0200, "Gladiator EVO R"), None);
        let exact = Match {
            vid: 0x231D,
            pid: Some(0x0200),
            name: Some("gladiator".into()),
        };
        assert_eq!(
            exact.score(0x231D, 0x0200, "VKBsim Gladiator EVO R"),
            Some(3)
        );
        assert_eq!(exact.score(0x231D, 0x0201, "VKBsim Gladiator EVO R"), None);
        let vid_only = Match {
            vid: 0x231D,
            pid: None,
            name: None,
        };
        assert_eq!(vid_only.score(0x231D, 1, "x"), None);
    }
}
