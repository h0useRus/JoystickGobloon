use std::collections::HashMap;

use crate::action::{Action, InputType};
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
}
