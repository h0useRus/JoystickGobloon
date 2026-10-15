use std::collections::HashMap;

use crate::action::{Action, InputType, Item};
use crate::xml::Element;

/// A physical input bound to a tree of actions in one mode.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBinding {
    /// DirectInput instance GUID of the device.
    pub device_id: String,
    pub input_type: String,
    pub mode: String,
    pub input_id: String,
    pub root_action: String,
    /// How the input is interpreted by its actions (`button`, `axis`, `hat`).
    pub behavior: String,
    /// Unknown children of `<input>`, kept verbatim.
    pub extra: Vec<Element>,
}

impl InputBinding {
    pub fn input_type(&self) -> InputType {
        InputType::parse(&self.input_type)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    pub name: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: String,
    /// Name as Gremlin stored it, including any surrounding whitespace.
    pub name: String,
}

/// Top-level profile sections, in the order they appear in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Inputs,
    Settings,
    LogicalDevice,
    Library,
    Modes,
    Scripts,
    Devices,
    /// Index into [`Profile::extra`].
    Extra(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub version: String,
    pub inputs: Vec<InputBinding>,
    /// `<settings>` kept as a tree; see [`Profile::startup_mode`].
    pub settings: Element,
    pub logical_device: Element,
    pub library: Vec<Action>,
    pub modes: Vec<Mode>,
    /// User Python scripts (plugins). Preserved, not executed.
    pub scripts: Element,
    pub devices: Vec<Device>,
    pub extra: Vec<Element>,
    pub order: Vec<Section>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: "14".into(),
            inputs: Vec::new(),
            settings: Element::new("settings"),
            logical_device: Element::new("logical-device"),
            library: Vec::new(),
            modes: vec![Mode {
                name: "Default".into(),
                parent: None,
            }],
            scripts: Element::new("scripts"),
            devices: Vec::new(),
            extra: Vec::new(),
            order: vec![
                Section::Inputs,
                Section::Settings,
                Section::LogicalDevice,
                Section::Library,
                Section::Modes,
                Section::Scripts,
                Section::Devices,
            ],
        }
    }
}

impl Profile {
    pub fn action(&self, id: &str) -> Option<&Action> {
        self.library.iter().find(|a| a.id == id)
    }

    /// Action lookup table; build once when resolving many references.
    pub fn action_index(&self) -> HashMap<&str, &Action> {
        self.library.iter().map(|a| (a.id.as_str(), a)).collect()
    }

    pub fn startup_mode(&self) -> Option<String> {
        self.settings.child_text("startup-mode")
    }

    pub fn device_name(&self, id: &str) -> Option<&str> {
        self.devices
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.name.trim())
    }

    pub fn has_scripts(&self) -> bool {
        self.scripts.elements().next().is_some()
    }

    /// Moves everything bound to device `old` onto device `new`, e.g. the same
    /// stick on another PC where DirectInput assigned a different GUID.
    /// Returns the number of updated references.
    pub fn substitute_device(&mut self, old: &str, new: &str, new_name: &str) -> usize {
        let mut count = 0;
        for b in &mut self.inputs {
            if b.device_id.eq_ignore_ascii_case(old) {
                b.device_id = new.to_string();
                count += 1;
            }
        }
        for a in &mut self.library {
            count += replace_in_items(&mut a.items, old, new);
        }
        count += self.logical_device.replace_text(old, new);
        for e in &mut self.extra {
            count += e.replace_text(old, new);
        }
        let new_known = self.devices.iter().any(|d| d.id.eq_ignore_ascii_case(new));
        if new_known {
            self.devices.retain(|d| !d.id.eq_ignore_ascii_case(old));
        } else if let Some(d) = self
            .devices
            .iter_mut()
            .find(|d| d.id.eq_ignore_ascii_case(old))
        {
            d.id = new.to_string();
            d.name = new_name.to_string();
        }
        count
    }
}

/// Device names compare equal ignoring case and extra whitespace
/// (Gremlin keeps the padding some drivers report, e.g. `" VKBsim Gladiator EVO  R  "`).
pub fn same_device_name(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

fn replace_in_items(items: &mut [Item], old: &str, new: &str) -> usize {
    let mut count = 0;
    for item in items {
        match item {
            Item::Property(p) if p.value.eq_ignore_ascii_case(old) => {
                p.value = new.to_string();
                count += 1;
            }
            Item::Group { items, .. } => count += replace_in_items(items, old, new),
            Item::Raw(e) => count += e.replace_text(old, new),
            _ => {}
        }
    }
    count
}
