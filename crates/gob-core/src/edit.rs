//! Profile editing. New actions are written exactly like Gremlin R14 writes
//! them (property names, types and order), so edited profiles load in Gremlin.

use std::collections::HashSet;
use std::hash::{BuildHasher, Hasher};

use crate::action::{Action, InputType, Item, Property};
use crate::profile::{ActionConfig, InputBinding, Mode, Profile};

/// Action types the editor can create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewAction {
    MapToVjoy {
        input_type: InputType,
    },
    ResponseCurve,
    ChangeMode {
        target: String,
    },
    Tempo,
    Description,
    /// Button inputs map to a mouse button, axes and hats to cursor motion.
    MapToMouse {
        input_type: InputType,
    },
    SmartToggle,
    SplitAxis,
    PauseResume,
    RunCommand,
}

/// Child lists of container actions: `(group tag, is ordered list of actions)`.
pub fn child_groups(kind: &str) -> &'static [&'static str] {
    match kind {
        "root" => &["actions"],
        "tempo" => &["short-actions", "long-actions"],
        "smart-toggle" => &["actions"],
        "split-axis" => &["lower-actions", "upper-actions"],
        _ => &[],
    }
}

/// Random UUID v4 in Gremlin's text form.
pub fn new_uuid() -> String {
    let rand = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        h.finish()
    };
    let (a, b) = (rand(), rand());
    let b = (b & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000; // RFC 4122 variant
    let a = (a & 0xFFFF_FFFF_FFFF_0FFF) | 0x0000_0000_0000_4000; // version 4
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        a >> 32,
        (a >> 16) & 0xFFFF,
        a & 0xFFFF,
        b >> 48,
        b & 0xFFFF_FFFF_FFFF
    )
}

fn prop(ty: &str, name: &str, value: impl Into<String>) -> Item {
    Item::Property(Property {
        ty: ty.into(),
        name: name.into(),
        value: value.into(),
    })
}

fn group(tag: &str, items: Vec<Item>) -> Item {
    Item::Group {
        tag: tag.into(),
        attrs: Vec::new(),
        items,
    }
}

impl Action {
    fn build(kind: &str, mut items: Vec<Item>, activation: &str) -> Self {
        items.push(prop("string", "action-label", ""));
        items.push(prop("activation-mode", "activation-mode", activation));
        Self {
            id: new_uuid(),
            kind: kind.into(),
            items,
        }
    }

    /// Empty root container.
    pub fn new_root() -> Self {
        Self::build("root", vec![group("actions", Vec::new())], "disallowed")
    }

    pub fn new(kind: &NewAction) -> Self {
        match kind {
            NewAction::MapToVjoy { input_type } => {
                let ty = match input_type {
                    InputType::Axis => "axis",
                    InputType::Hat => "hat",
                    _ => "button",
                };
                let mut items = vec![
                    prop("int", "vjoy-device-id", "1"),
                    prop("int", "vjoy-input-id", "1"),
                    prop("input_type", "vjoy-input-type", ty),
                ];
                if ty == "axis" {
                    items.push(prop("axis_mode", "axis-mode", "absolute"));
                    items.push(prop("float", "axis-scaling", "1.0"));
                } else if ty == "button" {
                    items.push(prop("bool", "button-inverted", "False"));
                }
                Self::build("map-to-vjoy", items, "both")
            }
            NewAction::ResponseCurve => Self::build(
                "response-curve",
                vec![
                    group(
                        "deadzone",
                        vec![
                            prop("float", "low", "-1.0"),
                            prop("float", "center-low", "0.0"),
                            prop("float", "center-high", "0.0"),
                            prop("float", "high", "1.0"),
                        ],
                    ),
                    group(
                        "control-points",
                        vec![
                            prop("point2d", "point", "-1.0,-1.0"),
                            prop("point2d", "point", "1.0,1.0"),
                        ],
                    ),
                    prop("string", "curve-type", "PiecewiseLinear"),
                ],
                "disallowed",
            ),
            NewAction::ChangeMode { target } => Self::build(
                "change-mode",
                vec![
                    prop("string", "change-type", "Switch"),
                    group("target-mode", vec![prop("string", "name", target.clone())]),
                ],
                "press",
            ),
            NewAction::Tempo => Self::build(
                "tempo",
                vec![
                    group("short-actions", Vec::new()),
                    group("long-actions", Vec::new()),
                    prop("float", "threshold", "0.5"),
                    prop("string", "activate-on", "release"),
                ],
                "disallowed",
            ),
            NewAction::MapToMouse { input_type } => {
                let items = if *input_type == InputType::Button {
                    vec![
                        prop("string", "mode", "Button"),
                        prop("string", "button", "Left"),
                    ]
                } else {
                    vec![
                        prop("string", "mode", "Motion"),
                        prop("int", "direction", "0"),
                        prop("int", "min-speed", "50"),
                        prop("int", "max-speed", "250"),
                        prop("float", "time-to-max-speed", "1.0"),
                    ]
                };
                Self::build("map-to-mouse", items, "both")
            }
            NewAction::SmartToggle => Self::build(
                "smart-toggle",
                vec![group("actions", Vec::new()), prop("float", "delay", "0.5")],
                "disallowed",
            ),
            NewAction::SplitAxis => Self::build(
                "split-axis",
                vec![
                    prop("float", "split-value", "0.0"),
                    group("lower-actions", Vec::new()),
                    group("upper-actions", Vec::new()),
                ],
                "disallowed",
            ),
            NewAction::PauseResume => Self::build(
                "pause-resume",
                vec![prop("string", "operation", "Pause")],
                "press",
            ),
            NewAction::RunCommand => Self::build(
                "run-command",
                vec![
                    prop("string", "executable", ""),
                    prop("string", "arguments", ""),
                ],
                "press",
            ),
            NewAction::Description => Self::build(
                "description",
                vec![prop("string", "description", "")],
                "disallowed",
            ),
        }
    }

    /// Sets a top-level property, adding it before the common label/activation properties if missing.
    pub fn set(&mut self, name: &str, ty: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(p) = self.items.iter_mut().find_map(|i| match i {
            Item::Property(p) if p.name == name => Some(p),
            _ => None,
        }) {
            p.value = value;
            return;
        }
        let at = self
            .items
            .iter()
            .position(|i| matches!(i, Item::Property(p) if p.name == "action-label" || p.name == "activation-mode"))
            .unwrap_or(self.items.len());
        self.items.insert(at, prop(ty, name, value));
    }

    /// Removes a top-level property.
    pub fn unset(&mut self, name: &str) {
        self.items
            .retain(|i| !matches!(i, Item::Property(p) if p.name == name));
    }

    /// Items of a group, created (before the properties) when missing.
    pub fn group_mut(&mut self, tag: &str) -> &mut Vec<Item> {
        let pos = match self
            .items
            .iter()
            .position(|i| matches!(i, Item::Group { tag: t, .. } if t == tag))
        {
            Some(p) => p,
            None => {
                let at = self
                    .items
                    .iter()
                    .position(|i| matches!(i, Item::Property(_)))
                    .unwrap_or(self.items.len());
                self.items.insert(at, group(tag, Vec::new()));
                at
            }
        };
        match &mut self.items[pos] {
            Item::Group { items, .. } => items,
            _ => unreachable!("position matched a group"),
        }
    }

    /// Child action ids of a group, in order.
    pub fn children(&self, tag: &str) -> Vec<String> {
        self.group(tag)
            .unwrap_or_default()
            .iter()
            .filter_map(|i| match i {
                Item::ActionRef(id) => Some(id.clone()),
                _ => None,
            })
            .collect()
    }
}

impl Profile {
    pub fn action_mut(&mut self, id: &str) -> Option<&mut Action> {
        self.library.iter_mut().find(|a| a.id == id)
    }

    /// Root action of an input in a mode, creating the binding if needed.
    pub fn bind(
        &mut self,
        device_id: &str,
        input_type: &str,
        input_id: &str,
        mode: &str,
    ) -> String {
        let found = self.inputs.iter().position(|b| {
            b.device_id.eq_ignore_ascii_case(device_id)
                && b.input_type == input_type
                && b.input_id == input_id
                && b.mode == mode
        });
        if let Some(cfg) = found.and_then(|i| self.inputs[i].configs.first()) {
            return cfg.root_action.clone();
        }
        let root = Action::new_root();
        let id = root.id.clone();
        self.library.push(root);
        let config = ActionConfig {
            root_action: id.clone(),
            behavior: input_type.into(),
            virtual_button: None,
            extra: Vec::new(),
        };
        match found {
            Some(i) => self.inputs[i].configs.push(config),
            None => self.inputs.push(InputBinding {
                device_id: device_id.into(),
                input_type: input_type.into(),
                mode: mode.into(),
                input_id: input_id.into(),
                configs: vec![config],
                extra: Vec::new(),
            }),
        }
        id
    }

    /// Removes an input's binding in a mode and every action only it used.
    pub fn unbind(&mut self, device_id: &str, input_type: &str, input_id: &str, mode: &str) {
        self.inputs.retain(|b| {
            !(b.device_id.eq_ignore_ascii_case(device_id)
                && b.input_type == input_type
                && b.input_id == input_id
                && b.mode == mode)
        });
        self.collect_garbage();
    }

    /// Adds `action` to the library and to `parent`'s child list `group`; returns its id.
    pub fn add_child(&mut self, parent: &str, group: &str, action: Action) -> Option<String> {
        let id = action.id.clone();
        self.action_mut(parent)?
            .group_mut(group)
            .push(Item::ActionRef(id.clone()));
        self.library.push(action);
        Some(id)
    }

    /// Removes the `index`-th child of `parent`'s `group` and actions left unreferenced.
    pub fn remove_child(&mut self, parent: &str, group: &str, index: usize) {
        if let Some(a) = self.action_mut(parent) {
            let refs = a.group_mut(group);
            if let Some(pos) = nth_ref(refs, index) {
                refs.remove(pos);
            }
        }
        self.collect_garbage();
    }

    /// Moves a child within its list.
    pub fn move_child(&mut self, parent: &str, group: &str, from: usize, to: usize) {
        let Some(a) = self.action_mut(parent) else {
            return;
        };
        let refs = a.group_mut(group);
        let (Some(f), Some(t)) = (nth_ref(refs, from), nth_ref(refs, to)) else {
            return;
        };
        let item = refs.remove(f);
        refs.insert(t, item);
    }

    /// Drops library actions that no input reaches any more.
    pub fn collect_garbage(&mut self) {
        let index = self.action_index();
        let mut reachable: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = self
            .inputs
            .iter()
            .flat_map(|b| b.configs.iter().map(|c| c.root_action.clone()))
            .collect();
        while let Some(id) = stack.pop() {
            if reachable.insert(id.clone()) {
                if let Some(a) = index.get(id.as_str()) {
                    stack.extend(a.child_ids().into_iter().map(String::from));
                }
            }
        }
        self.library.retain(|a| reachable.contains(&a.id));
    }

    /// Mode active when the profile starts.
    pub fn set_startup_mode(&mut self, name: &str) {
        let mut found = false;
        for c in self.settings.children.iter_mut() {
            if let crate::xml::Node::Element(e) = c {
                if e.name == "startup-mode" {
                    e.children = vec![crate::xml::Node::Text(name.into())];
                    found = true;
                }
            }
        }
        if !found {
            self.settings
                .push(crate::xml::Element::with_text("startup-mode", name));
        }
    }

    pub fn add_mode(&mut self, name: &str, parent: Option<&str>) -> bool {
        if name.trim().is_empty() || self.modes.iter().any(|m| m.name == name) {
            return false;
        }
        self.modes.push(Mode {
            name: name.into(),
            parent: parent.map(String::from),
        });
        true
    }

    /// Renames a mode and every reference to it (bindings, parents, change-mode targets, startup mode).
    pub fn rename_mode(&mut self, old: &str, new: &str) -> bool {
        if new.trim().is_empty()
            || self.modes.iter().any(|m| m.name == new)
            || !self.modes.iter().any(|m| m.name == old)
        {
            return false;
        }
        for m in &mut self.modes {
            if m.name == old {
                m.name = new.into();
            }
            if m.parent.as_deref() == Some(old) {
                m.parent = Some(new.into());
            }
        }
        for b in &mut self.inputs {
            if b.mode == old {
                b.mode = new.into();
            }
        }
        for a in self.library.iter_mut().filter(|a| a.kind == "change-mode") {
            for item in &mut a.items {
                if let Item::Group { tag, items, .. } = item {
                    if tag == "target-mode" {
                        for i in items {
                            if let Item::Property(p) = i {
                                if p.name == "name" && p.value == old {
                                    p.value = new.into();
                                }
                            }
                        }
                    }
                }
            }
        }
        if self.startup_mode().as_deref() == Some(old) {
            self.settings.replace_text(old, new);
        }
        true
    }

    /// Removes a mode and its bindings; its children move to its parent. The last mode stays.
    pub fn remove_mode(&mut self, name: &str) -> bool {
        let Some(pos) = self.modes.iter().position(|m| m.name == name) else {
            return false;
        };
        if self.modes.len() == 1 {
            return false;
        }
        let parent = self.modes.remove(pos).parent;
        for m in &mut self.modes {
            if m.parent.as_deref() == Some(name) {
                m.parent = parent.clone();
            }
        }
        self.inputs.retain(|b| b.mode != name);
        if self.startup_mode().as_deref() == Some(name) {
            let first = self.modes[0].name.clone();
            self.settings.replace_text(name, &first);
        }
        self.collect_garbage();
        true
    }

    /// Changes a mode's parent; refuses cycles.
    pub fn set_mode_parent(&mut self, name: &str, parent: Option<&str>) -> bool {
        let mut cursor = parent.map(String::from);
        while let Some(p) = cursor {
            if p == name {
                return false;
            }
            cursor = self
                .modes
                .iter()
                .find(|m| m.name == p)
                .and_then(|m| m.parent.clone());
        }
        match self.modes.iter_mut().find(|m| m.name == name) {
            Some(m) => {
                m.parent = parent.map(String::from);
                true
            }
            None => false,
        }
    }
}

/// Position of the `n`-th action reference within a group's items.
fn nth_ref(items: &[Item], n: usize) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, i)| matches!(i, Item::ActionRef(_)))
        .nth(n)
        .map(|(pos, _)| pos)
}
