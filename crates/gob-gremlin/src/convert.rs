//! [`Element`] tree <-> [`Profile`] for Gremlin R14+ profiles.

use gob_core::{
    Action, ActionConfig, Device, Element, InputBinding, Item, Mode, Node, Profile, Property,
    Section,
};

use crate::Error;

/// First profile format version written by Gremlin R14.
pub const MIN_VERSION: u32 = 14;

pub fn profile_from_element(root: &Element) -> Result<Profile, Error> {
    if root.name != "profile" {
        return Err(Error::NotAProfile);
    }
    let version = root.attr("version").unwrap_or_default().to_string();
    if version.parse::<u32>().map_or(true, |v| v < MIN_VERSION) {
        return Err(Error::UnsupportedVersion(version));
    }
    let mut p = Profile {
        version,
        order: Vec::new(),
        modes: Vec::new(),
        ..Default::default()
    };
    for section in root.elements() {
        let kind = match section.name.as_str() {
            "inputs" => {
                p.inputs = named(section, "input").map(input_from).collect();
                Section::Inputs
            }
            "settings" => {
                p.settings = section.clone();
                Section::Settings
            }
            "logical-device" => {
                p.logical_device = section.clone();
                Section::LogicalDevice
            }
            "library" => {
                p.library = named(section, "action").map(action_from).collect();
                Section::Library
            }
            "modes" => {
                p.modes = named(section, "mode")
                    .map(|m| Mode {
                        name: m.text(),
                        parent: m.attr("parent").map(String::from),
                    })
                    .collect();
                Section::Modes
            }
            "scripts" => {
                p.scripts = section.clone();
                Section::Scripts
            }
            "devices" => {
                p.devices = named(section, "device")
                    .map(|d| Device {
                        id: d.child_text("device-id").unwrap_or_default(),
                        name: d.child_text("device-name").unwrap_or_default(),
                    })
                    .collect();
                Section::Devices
            }
            _ => {
                p.extra.push(section.clone());
                Section::Extra(p.extra.len() - 1)
            }
        };
        p.order.push(kind);
    }
    Ok(p)
}

pub fn profile_to_element(p: &Profile) -> Element {
    let mut root = Element::new("profile").with_attr("version", &p.version);
    for section in &p.order {
        root.push(match *section {
            Section::Inputs => container("inputs", p.inputs.iter().map(input_to)),
            Section::Settings => p.settings.clone(),
            Section::LogicalDevice => p.logical_device.clone(),
            Section::Library => container("library", p.library.iter().map(action_to)),
            Section::Modes => container(
                "modes",
                p.modes.iter().map(|m| {
                    let el = Element::with_text("mode", &m.name);
                    match &m.parent {
                        Some(parent) => el.with_attr("parent", parent),
                        None => el,
                    }
                }),
            ),
            Section::Scripts => p.scripts.clone(),
            Section::Devices => container(
                "devices",
                p.devices.iter().map(|d| {
                    container(
                        "device",
                        [leaf("device-id", &d.id), leaf("device-name", &d.name)],
                    )
                }),
            ),
            Section::Extra(i) => p.extra[i].clone(),
        });
    }
    root
}

fn named<'a>(parent: &'a Element, name: &'a str) -> impl Iterator<Item = &'a Element> {
    parent.elements().filter(move |e| e.name == name)
}

fn container(name: &str, children: impl IntoIterator<Item = Element>) -> Element {
    let mut el = Element::new(name);
    children.into_iter().for_each(|c| el.push(c));
    el
}

/// Text element; empty text becomes `<name/>`.
fn leaf(name: &str, text: &str) -> Element {
    if text.is_empty() {
        Element::new(name)
    } else {
        Element::with_text(name, text)
    }
}

fn input_from(e: &Element) -> InputBinding {
    let mut b = InputBinding {
        device_id: String::new(),
        input_type: String::new(),
        mode: String::new(),
        input_id: String::new(),
        configs: Vec::new(),
        extra: Vec::new(),
    };
    for c in e.elements() {
        match c.name.as_str() {
            "device-id" => b.device_id = c.text(),
            "input-type" => b.input_type = c.text(),
            "mode" => b.mode = c.text(),
            "input-id" => b.input_id = c.text(),
            "action-configuration" => b.configs.push(config_from(c)),
            _ => b.extra.push(c.clone()),
        }
    }
    b
}

fn config_from(e: &Element) -> ActionConfig {
    let mut cfg = ActionConfig {
        root_action: String::new(),
        behavior: String::new(),
        virtual_button: None,
        extra: Vec::new(),
    };
    for c in e.elements() {
        match c.name.as_str() {
            "root-action" => cfg.root_action = c.text(),
            "behavior" => cfg.behavior = c.text(),
            "virtual-button" => cfg.virtual_button = Some(c.clone()),
            _ => cfg.extra.push(c.clone()),
        }
    }
    cfg
}

fn input_to(b: &InputBinding) -> Element {
    let configs = b.configs.iter().map(|cfg| {
        container(
            "action-configuration",
            [
                leaf("root-action", &cfg.root_action),
                leaf("behavior", &cfg.behavior),
            ]
            .into_iter()
            .chain(cfg.virtual_button.iter().cloned())
            .chain(cfg.extra.iter().cloned()),
        )
    });
    container(
        "input",
        [
            leaf("device-id", &b.device_id),
            leaf("input-type", &b.input_type),
            leaf("mode", &b.mode),
            leaf("input-id", &b.input_id),
        ]
        .into_iter()
        .chain(configs)
        .chain(b.extra.iter().cloned()),
    )
}

fn action_from(e: &Element) -> Action {
    Action {
        id: e.attr("id").unwrap_or_default().into(),
        kind: e.attr("type").unwrap_or_default().into(),
        items: e.elements().map(item_from).collect(),
    }
}

fn action_to(a: &Action) -> Element {
    let el = Element::new("action")
        .with_attr("id", &a.id)
        .with_attr("type", &a.kind);
    container_items(el, &a.items)
}

fn is_leaf(e: &Element) -> bool {
    e.elements().next().is_none()
}

fn item_from(e: &Element) -> Item {
    if e.name == "property" {
        if let Some(p) = property_from(e) {
            return Item::Property(p);
        }
    } else if e.name == "action-id" && is_leaf(e) {
        return Item::ActionRef(e.text());
    } else if !is_leaf(e) || e.children.is_empty() {
        let has_text = e.children.iter().any(|n| matches!(n, Node::Text(_)));
        if !has_text {
            return Item::Group {
                tag: e.name.clone(),
                attrs: e.attrs.clone(),
                items: e.elements().map(item_from).collect(),
            };
        }
    }
    Item::Raw(e.clone())
}

/// Accepts only the exact `<property type><name/><value/></property>` shape.
fn property_from(e: &Element) -> Option<Property> {
    let [(key, ty)] = e.attrs.as_slice() else {
        return None;
    };
    let kids: Vec<&Element> = e.elements().collect();
    let [name, value] = kids.as_slice() else {
        return None;
    };
    (key == "type"
        && name.name == "name"
        && value.name == "value"
        && is_leaf(name)
        && is_leaf(value))
    .then(|| Property {
        ty: ty.clone(),
        name: name.text(),
        value: value.text(),
    })
}

fn item_to(item: &Item) -> Element {
    match item {
        Item::Property(p) => {
            container("property", [leaf("name", &p.name), leaf("value", &p.value)])
                .with_attr("type", &p.ty)
        }
        Item::ActionRef(id) => leaf("action-id", id),
        Item::Group { tag, attrs, items } => {
            let mut el = Element::new(tag);
            el.attrs = attrs.clone();
            container_items(el, items)
        }
        Item::Raw(e) => e.clone(),
    }
}

fn container_items(mut el: Element, items: &[Item]) -> Element {
    items.iter().for_each(|i| el.push(item_to(i)));
    el
}
