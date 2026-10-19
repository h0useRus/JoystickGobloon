//! Profile editing. New actions are written exactly like Gremlin R14 writes
//! them (property names, types and order), so edited profiles load in Gremlin -
//! except Joystick Gobloon's own actions (light-led), which Gremlin does not know.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};

use crate::action::{Action, InputType, Item, Property, HAT_BUTTONS};
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
    DoubleTap,
    /// Four directions plus center.
    HatButtons,
    AxisDelta,
    /// No conditions yet: like Gremlin, true until some are added.
    Condition,
    LoadProfile,
    PlaySound,
    /// LED `led` of `device` (DirectInput instance GUID, empty = not chosen yet) lit green
    /// on press.
    LightLed {
        device: String,
        led: u8,
    },
    TextToSpeech,
    /// No keys yet; they are recorded in the editor.
    MapToKeyboard,
    /// One empty step, no timeout (Gremlin's defaults).
    Chain,
    /// No steps, runs once.
    Macro,
    /// Writes to a logical device input of the same type as the edited input.
    MapToLogicalDevice {
        input_type: InputType,
        id: u32,
    },
    /// Starts with `axis` of `device` in both slots, like [`NewAction::MergeAxis`].
    DualAxisDeadzone {
        device: String,
        axis: u32,
    },
    /// Averages `axis` of `device` with itself until the second axis is picked.
    MergeAxis {
        device: String,
        axis: u32,
    },
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
        "double-tap" => &["single-actions", "double-actions"],
        "smart-toggle" => &["actions"],
        "split-axis" => &["lower-actions", "upper-actions"],
        "axis-delta" => &["positive-actions", "negative-actions"],
        "condition" => &["true-actions", "false-actions"],
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

fn macro_positions(items: &[Item]) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, i)| matches!(i, Item::Group { tag, .. } if tag == "macro-action"))
        .map(|(n, _)| n)
        .collect()
}

/// Gremlin's `<macro-action type=...>` content for a step.
fn macro_step_xml(step: &crate::MacroStep) -> Option<(&'static str, Vec<Item>)> {
    use crate::MacroStep as S;
    let b = |v: bool| if v { "True" } else { "False" };
    let vjoy = |device: u32, ty: &str, id: u32| {
        vec![
            prop("int", "vjoy-id", device.to_string()),
            prop("input_type", "input-type", ty),
            prop("int", "input-id", id.to_string()),
        ]
    };
    Some(match *step {
        S::Pause(d) => ("pause", vec![prop("float", "duration", format!("{d:?}"))]),
        S::Key {
            scan_code,
            extended,
            pressed,
        } => (
            "key",
            vec![
                prop("int", "scan-code", scan_code.to_string()),
                prop("bool", "is-extended", b(extended)),
                prop("bool", "is-pressed", b(pressed)),
            ],
        ),
        S::MouseButton { button, pressed } => (
            "mouse-button",
            vec![
                prop("string", "button", button.name()),
                prop("bool", "is-pressed", b(pressed)),
            ],
        ),
        S::MouseMotion { dx, dy } => (
            "mouse-motion",
            vec![
                prop("int", "dx", dx.to_string()),
                prop("int", "dy", dy.to_string()),
            ],
        ),
        S::VjoyButton {
            device,
            id,
            pressed,
        } => {
            let mut items = vjoy(device, "button", id);
            items.push(prop("bool", "value", b(pressed)));
            ("vjoy", items)
        }
        S::VjoyAxis {
            device,
            id,
            value,
            relative,
        } => {
            let mut items = vjoy(device, "axis", id);
            items.push(prop("float", "value", format!("{value:?}")));
            let mode = if relative { "relative" } else { "absolute" };
            items.push(prop("axis_mode", "axis-mode", mode));
            ("vjoy", items)
        }
        S::VjoyHat {
            device,
            id,
            direction,
        } => {
            let mut items = vjoy(device, "hat", id);
            items.push(prop(
                "hat_direction",
                "value",
                crate::profile::hat_direction_name(direction),
            ));
            ("vjoy", items)
        }
        S::Other => return None,
    })
}

/// (step number, item position) of `chain-N` groups, in step order.
fn chain_positions(items: &[Item]) -> Vec<(u32, usize)> {
    let mut steps: Vec<(u32, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(pos, i)| match i {
            Item::Group { tag, .. } => Some((tag.strip_prefix("chain-")?.parse().ok()?, pos)),
            _ => None,
        })
        .collect();
    steps.sort();
    steps
}

/// Names steps `chain-0`, `chain-1`, ... in their current order (as Gremlin writes them).
fn renumber_chain(items: &mut [Item]) {
    for (n, (_, pos)) in chain_positions(items).into_iter().enumerate() {
        if let Item::Group { tag, .. } = &mut items[pos] {
            *tag = format!("chain-{n}");
        }
    }
}

fn condition_positions(items: &[Item]) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, i)| matches!(i, Item::Group { tag, .. } if tag == "condition"))
        .map(|(n, _)| n)
        .collect()
}

fn remap_refs(items: &mut [Item], ids: &HashMap<String, String>) {
    for item in items {
        match item {
            Item::ActionRef(id) => {
                if let Some(new) = ids.get(id.as_str()) {
                    *id = new.clone();
                }
            }
            Item::Group { items, .. } => remap_refs(items, ids),
            _ => {}
        }
    }
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
            NewAction::DoubleTap => Self::build(
                "double-tap",
                vec![
                    group("single-actions", Vec::new()),
                    group("double-actions", Vec::new()),
                    prop("float", "threshold", "0.5"),
                    prop("string", "activate-on", "exclusive"),
                ],
                "disallowed",
            ),
            NewAction::DualAxisDeadzone { device, axis } => Self::build(
                "dual-axis-deadzone",
                vec![
                    prop("string", "label", ""),
                    prop("float", "inner-deadzone", "0.0"),
                    prop("float", "outer-deadzone", "1.0"),
                    prop("uuid", "axis1-guid", device.clone()),
                    prop("int", "axis1-axis", axis.to_string()),
                    prop("uuid", "axis2-guid", device.clone()),
                    prop("int", "axis2-axis", axis.to_string()),
                    group("output1-actions", Vec::new()),
                    group("output2-actions", Vec::new()),
                ],
                "disallowed",
            ),
            NewAction::MapToKeyboard => Self::build("map-to-keyboard", Vec::new(), "both"),
            NewAction::MapToLogicalDevice { input_type, id } => {
                let ty = match input_type {
                    InputType::Axis => "axis",
                    InputType::Hat => "hat",
                    _ => "button",
                };
                let mut items = vec![
                    prop("int", "logical-input-id", id.to_string()),
                    prop("input_type", "logical-input-type", ty),
                ];
                match input_type {
                    InputType::Axis => {
                        items.push(prop("axis_mode", "axis-mode", "absolute"));
                        items.push(prop("float", "axis-scaling", "1.0"));
                    }
                    InputType::Button => items.push(prop("bool", "button-inverted", "False")),
                    _ => {}
                }
                Self::build("map-to-logical-device", items, "both")
            }
            NewAction::Macro => Self::build(
                "macro",
                vec![
                    prop("bool", "is-exclusive", "False"),
                    prop("bool", "is-preemptive", "False"),
                    prop("string", "repeat-mode", "Single"),
                    prop("int", "repeat-count", "1"),
                    prop("float", "repeat-delay", "0.1"),
                ],
                "press",
            ),
            NewAction::Chain => Self::build(
                "chain",
                vec![
                    group("chain-0", Vec::new()),
                    prop("float", "timeout", "0.0"),
                ],
                "disallowed",
            ),
            NewAction::LoadProfile => Self::build(
                "load-profile",
                vec![prop("string", "load-profile", "")],
                "press",
            ),
            NewAction::PlaySound => Self::build(
                "play-sound",
                vec![prop("string", "filename", ""), prop("int", "volume", "50")],
                "press",
            ),
            NewAction::LightLed { device, led } => Self::build(
                "light-led",
                vec![
                    prop("uuid", "device-guid", device.clone()),
                    prop("int", "led-id", led.to_string()),
                    prop("int", "led-element", "0"),
                    prop("string", "color", "#00FF00"),
                    prop("string", "color2", "#000000"),
                    prop("string", "color-mode", "color1"),
                    prop("string", "led-mode", "on"),
                    prop("bool", "on-activate", "False"),
                ],
                "press",
            ),
            NewAction::TextToSpeech => Self::build(
                "text-to-speech",
                vec![
                    prop("string", "text", ""),
                    prop("string", "queue-mode", "queue-back"),
                    prop("float", "playback-rate", "0.0"),
                    prop("float", "playback-volume", "1.0"),
                    prop("float", "playback-pitch", "0.0"),
                ],
                "press",
            ),
            NewAction::Condition => Self::build(
                "condition",
                vec![
                    prop("string", "logical-operator", "all"),
                    group("true-actions", Vec::new()),
                    group("false-actions", Vec::new()),
                ],
                "both",
            ),
            NewAction::AxisDelta => Self::build(
                "axis-delta",
                vec![
                    prop("float", "threshold", "0.1"),
                    group("positive-actions", Vec::new()),
                    group("negative-actions", Vec::new()),
                ],
                "disallowed",
            ),
            NewAction::HatButtons => {
                let mut items = vec![prop("int", "button-count", "4")];
                items.extend(
                    ["North", "East", "South", "West", "Center"].map(|d| group(d, Vec::new())),
                );
                Self::build("hat-buttons", items, "disallowed")
            }
            NewAction::MergeAxis { device, axis } => Self::build(
                "merge-axis",
                vec![
                    prop("string", "label", ""),
                    prop("uuid", "axis1-guid", device.clone()),
                    prop("int", "axis1-axis", axis.to_string()),
                    prop("uuid", "axis2-guid", device.clone()),
                    prop("int", "axis2-axis", axis.to_string()),
                    prop("string", "operation", "average"),
                    group("actions", Vec::new()),
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

    /// Appends an existing library action to `parent`'s `group` (Gremlin's action reuse).
    /// Refused when it would create a loop (`parent` is `id` or lies under it).
    pub fn add_ref(&mut self, parent: &str, group: &str, id: &str) -> bool {
        if self.action(id).is_none() || self.reaches(id, parent) {
            return false;
        }
        let Some(a) = self.action_mut(parent) else {
            return false;
        };
        a.group_mut(group).push(Item::ActionRef(id.into()));
        true
    }

    /// Replaces the `index`-th step of a macro (`index` = step count appends). Steps Joystick Gobloon
    /// cannot represent ([`MacroStep::Other`]) are never written.
    pub fn set_macro_step(&mut self, id: &str, index: usize, step: &crate::MacroStep) {
        let Some((ty, items)) = macro_step_xml(step) else {
            return;
        };
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let steps = macro_positions(&a.items);
        let node = Item::Group {
            tag: "macro-action".into(),
            attrs: vec![("type".into(), ty.into())],
            items,
        };
        match steps.get(index) {
            Some(&pos) => a.items[pos] = node,
            None => {
                // After the last step, else after the repeat settings (Gremlin's order).
                let at = steps.last().map(|p| p + 1).unwrap_or_else(|| {
                    a.items
                        .iter()
                        .rposition(|i| matches!(i, Item::Property(p) if p.name.starts_with("repeat-") || p.name.starts_with("is-")))
                        .map_or(0, |p| p + 1)
                });
                a.items.insert(at, node);
            }
        }
    }

    pub fn remove_macro_step(&mut self, id: &str, index: usize) {
        if let Some(a) = self.action_mut(id) {
            if let Some(&pos) = macro_positions(&a.items).get(index) {
                a.items.remove(pos);
            }
        }
    }

    /// Swaps step `index` with the next one.
    pub fn swap_macro_steps(&mut self, id: &str, index: usize) {
        if let Some(a) = self.action_mut(id) {
            let steps = macro_positions(&a.items);
            if let (Some(&x), Some(&y)) = (steps.get(index), steps.get(index + 1)) {
                a.items.swap(x, y);
            }
        }
    }

    /// Appends an empty step to a chain action.
    pub fn add_chain_step(&mut self, id: &str) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let steps = chain_positions(&a.items);
        // After the last step, numbered after it even if the numbers have gaps.
        let (next, at) = steps.last().map_or((0, 0), |&(n, pos)| (n + 1, pos + 1));
        a.items
            .insert(at, group(&format!("chain-{next}"), Vec::new()));
        renumber_chain(&mut a.items);
    }

    /// Removes the `index`-th step of a chain action and renumbers the rest from 0.
    pub fn remove_chain_step(&mut self, id: &str, index: usize) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        if let Some(&(_, pos)) = chain_positions(&a.items).get(index) {
            a.items.remove(pos);
        }
        renumber_chain(&mut a.items);
        self.collect_garbage();
    }

    /// Replaces the keys of a map-to-keyboard action (`<input>` groups, written first like Gremlin).
    pub fn set_keys(&mut self, id: &str, keys: &[crate::Key]) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        a.items
            .retain(|i| !matches!(i, Item::Group { tag, .. } if tag == "input"));
        let groups = keys.iter().map(|k| {
            group(
                "input",
                vec![
                    prop("int", "scan-code", k.scan_code.to_string()),
                    prop(
                        "bool",
                        "is-extended",
                        if k.extended { "True" } else { "False" },
                    ),
                ],
            )
        });
        a.items.splice(0..0, groups);
    }

    /// Replaces the `index`-th `<condition>` of a condition action; `index` = count appends.
    pub fn set_condition(&mut self, id: &str, index: usize, condition: &crate::Condition) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let items = condition.to_items();
        let positions: Vec<usize> = condition_positions(&a.items);
        match positions.get(index) {
            Some(&pos) => {
                if let Item::Group { items: old, .. } = &mut a.items[pos] {
                    *old = items;
                }
            }
            None => {
                // After the last condition, else after `logical-operator` (Gremlin's order).
                let at = positions.last().map(|p| p + 1).unwrap_or_else(|| {
                    a.items
                        .iter()
                        .position(
                            |i| matches!(i, Item::Property(p) if p.name == "logical-operator"),
                        )
                        .map_or(0, |p| p + 1)
                });
                a.items.insert(at, group("condition", items));
            }
        }
    }

    pub fn remove_condition(&mut self, id: &str, index: usize) {
        if let Some(a) = self.action_mut(id) {
            if let Some(&pos) = condition_positions(&a.items).get(index) {
                a.items.remove(pos);
            }
        }
    }

    /// `id` is `target` or contains it somewhere below.
    pub fn reaches(&self, id: &str, target: &str) -> bool {
        let index = self.action_index();
        let mut seen: HashSet<&str> = HashSet::new();
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            if cur == target {
                return true;
            }
            if seen.insert(cur) {
                if let Some(a) = index.get(cur) {
                    stack.extend(a.child_ids());
                }
            }
        }
        false
    }

    /// Deep copy of `id` and everything below it under new ids (Gremlin's "duplicate"
    /// in the reference action). Returns the copy's id.
    pub fn duplicate(&mut self, id: &str) -> Option<String> {
        let index = self.action_index();
        let mut order: Vec<String> = Vec::new();
        let mut stack = vec![id.to_string()];
        while let Some(cur) = stack.pop() {
            if order.contains(&cur) {
                continue;
            }
            let a = index.get(cur.as_str())?;
            stack.extend(a.child_ids().into_iter().map(String::from));
            order.push(cur);
        }
        let ids: HashMap<String, String> =
            order.iter().map(|old| (old.clone(), new_uuid())).collect();
        let copies: Vec<Action> = order
            .iter()
            .filter_map(|old| {
                let mut a = (*index.get(old.as_str())?).clone();
                a.id = ids[old].clone();
                remap_refs(&mut a.items, &ids);
                Some(a)
            })
            .collect();
        self.library.extend(copies);
        ids.get(id).cloned()
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

    /// Switches a hat-buttons action between 4 and 8 directions. Shared directions keep
    /// their actions, dropped diagonals lose theirs (Gremlin's `set_button_count`).
    pub fn set_hat_button_count(&mut self, id: &str, count: u8) {
        if count != 4 && count != 8 {
            return;
        }
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let mut lists: Vec<(String, Vec<Item>)> = Vec::new();
        a.items.retain_mut(|i| match i {
            Item::Group { tag, items, .. }
                if HAT_BUTTONS.contains(&tag.as_str()) || tag == "Center" =>
            {
                lists.push((tag.clone(), std::mem::take(items)));
                false
            }
            _ => true,
        });
        a.set("button-count", "int", count.to_string());
        let at = a
            .items
            .iter()
            .position(|i| matches!(i, Item::Property(p) if p.name == "button-count"))
            .map_or(0, |p| p + 1);
        let tags = HAT_BUTTONS
            .iter()
            .enumerate()
            .filter(|(i, _)| count == 8 || i % 2 == 0)
            .map(|(_, t)| *t)
            .chain(["Center"]);
        let groups: Vec<Item> = tags
            .map(|t| {
                let items = lists
                    .iter_mut()
                    .find(|(tag, _)| tag == t)
                    .map(|(_, items)| std::mem::take(items))
                    .unwrap_or_default();
                group(t, items)
            })
            .collect();
        a.items.splice(at..at, groups);
        self.collect_garbage();
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
