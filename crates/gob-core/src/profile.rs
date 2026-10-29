use std::collections::{HashMap, HashSet};

use crate::action::{Action, ActionKind, InputType, Item};
use crate::modifier::{Layer, Modifier};
use crate::xml::Element;

/// A virtual device output an action writes to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VirtualOutput {
    Vjoy {
        device: u32,
        input_type: InputType,
        id: u32,
    },
    /// `control` is a map-to-gamepad control name (`a`, `left-x`, `dpad`, ...).
    Gamepad { gamepad: u32, control: String },
}

/// The virtual output `action` writes, if it is map-to-vjoy or map-to-gamepad.
pub fn virtual_output(action: &Action) -> Option<VirtualOutput> {
    match action.kind() {
        ActionKind::MapToVjoy {
            device,
            input_type,
            input_id,
            ..
        } => Some(VirtualOutput::Vjoy {
            device,
            input_type,
            id: input_id,
        }),
        ActionKind::MapToGamepad {
            gamepad, control, ..
        } => Some(VirtualOutput::Gamepad {
            gamepad,
            control: control.to_string(),
        }),
        _ => None,
    }
}

/// One binding writing a [`VirtualOutput`].
#[derive(Debug, Clone, PartialEq)]
pub struct OutputUse {
    /// Index into [`Profile::inputs`].
    pub binding: usize,
    /// The action that writes it.
    pub action: String,
}

/// A physical input bound to one or more action sequences in one mode.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBinding {
    /// DirectInput instance GUID of the device.
    pub device_id: String,
    pub input_type: String,
    pub mode: String,
    pub input_id: String,
    /// One entry per `<action-configuration>`; those of the layer the held modifiers choose run
    /// for every event (see [`crate::modifier`]).
    pub configs: Vec<ActionConfig>,
    /// Unknown children of `<input>`, kept verbatim.
    pub extra: Vec<Element>,
}

impl InputBinding {
    pub fn input_type(&self) -> InputType {
        InputType::parse(&self.input_type)
    }
}

/// One action sequence of an input (`<action-configuration>`).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionConfig {
    pub root_action: String,
    /// How the input is interpreted by its actions (`button`, `axis`, `hat`).
    pub behavior: String,
    /// `<virtual-button>` used when an axis or hat behaves as a button; kept verbatim.
    pub virtual_button: Option<Element>,
    /// Modifiers that must be held for it to run; the base layer runs without any.
    pub layer: Layer,
    /// Runs on top of the configurations of the held layer instead of only alone.
    pub additive: bool,
    /// Axis outputs keep their last value when this layer stops running instead of going back
    /// to rest.
    pub keep_value: bool,
    /// Unknown children, kept verbatim.
    pub extra: Vec<Element>,
}

/// Direction an axis has to move in for a virtual button to trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisDirection {
    Anywhere,
    Above,
    Below,
}

/// Turns an axis range or a set of hat directions into a button.
#[derive(Debug, Clone, PartialEq)]
pub enum VirtualButton {
    Axis {
        lower: f32,
        upper: f32,
        direction: AxisDirection,
    },
    /// Hat directions in hundredths of a degree; `None` is the center position.
    Hat { directions: Vec<Option<u16>> },
}

impl ActionConfig {
    pub fn behavior(&self) -> InputType {
        InputType::parse(&self.behavior)
    }

    /// Decodes the virtual button for `input_type` when this config needs one.
    pub fn virtual_button(&self, input_type: InputType) -> Option<VirtualButton> {
        if self.behavior() != InputType::Button {
            return None;
        }
        let vb = self.virtual_button.as_ref()?;
        match input_type {
            InputType::Axis => {
                let num = |n: &str, d: f32| {
                    vb.child_text(n)
                        .and_then(|t| t.trim().parse().ok())
                        .unwrap_or(d)
                };
                let direction = match vb
                    .child_text("axis-button-direction")
                    .as_deref()
                    .map(str::trim)
                {
                    Some("above") => AxisDirection::Above,
                    Some("below") => AxisDirection::Below,
                    _ => AxisDirection::Anywhere,
                };
                Some(VirtualButton::Axis {
                    lower: num("lower-limit", 0.0),
                    upper: num("upper-limit", 1.0),
                    direction,
                })
            }
            InputType::Hat => Some(VirtualButton::Hat {
                directions: vb
                    .elements()
                    .filter(|e| e.name == "hat-direction")
                    .filter_map(|e| hat_direction(e.text().trim()))
                    .collect(),
            }),
            _ => None,
        }
    }
}

/// Gremlin hat direction name of a direction (`center` for `None`).
pub(crate) fn hat_direction_name(d: Option<u16>) -> &'static str {
    const NAMES: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    match d {
        None => "center",
        Some(deg) => NAMES[((usize::from(deg) + 2250) / 4500) % 8],
    }
}

/// Gremlin hat direction name -> direction (`Some(None)` is center).
pub(crate) fn hat_direction(name: &str) -> Option<Option<u16>> {
    const NAMES: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    if name == "center" {
        return Some(None);
    }
    NAMES
        .iter()
        .position(|&n| n == name)
        .map(|i| Some(i as u16 * 4500))
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

/// Virtual devices the profile writes to, as they were set up when it was last saved, so
/// another system can set them up the same way (Joystick Gobloon only).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VirtualDevices {
    pub joysticks: Vec<VirtualJoystick>,
    /// Gamepad ids (their layout is fixed).
    pub gamepads: Vec<u32>,
}

impl VirtualDevices {
    pub fn is_empty(&self) -> bool {
        self.joysticks.is_empty() && self.gamepads.is_empty()
    }
}

/// A virtual joystick's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualJoystick {
    pub id: u32,
    /// Axis slots 1..=8 (X, Y, Z, Rx, Ry, Rz, slider, dial).
    pub axes: Vec<u8>,
    pub buttons: u32,
    pub hats: u32,
    /// 4-direction hats instead of continuous ones.
    pub discrete_hats: bool,
}

/// Top-level profile sections, in the order they appear in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Inputs,
    Settings,
    LogicalDevice,
    Library,
    Modes,
    Modifiers,
    Scripts,
    Devices,
    VirtualDevices,
    /// Index into [`Profile::extra`].
    Extra(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Format the profile was read from, for the import report (e.g. `Joystick Gremlin 14`);
    /// profiles are always written in Joystick Gobloon's own format.
    pub source: String,
    pub inputs: Vec<InputBinding>,
    /// `<settings>` kept as a tree; see [`Profile::startup_mode`].
    pub settings: Element,
    pub logical_device: Element,
    pub library: Vec<Action>,
    pub modes: Vec<Mode>,
    pub modifiers: Vec<Modifier>,
    /// User Python scripts (plugins). Preserved, not executed.
    pub scripts: Element,
    pub devices: Vec<Device>,
    pub virtual_devices: VirtualDevices,
    pub extra: Vec<Element>,
    pub order: Vec<Section>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            source: "Joystick Gobloon 1".into(),
            inputs: Vec::new(),
            settings: Element::new("settings"),
            logical_device: Element::new("logical-device"),
            library: Vec::new(),
            modes: vec![Mode {
                name: "Default".into(),
                parent: None,
            }],
            modifiers: Vec::new(),
            scripts: Element::new("scripts"),
            devices: Vec::new(),
            virtual_devices: VirtualDevices::default(),
            extra: Vec::new(),
            order: vec![
                Section::Inputs,
                Section::Settings,
                Section::LogicalDevice,
                Section::Library,
                Section::Modes,
                Section::Modifiers,
                Section::Scripts,
                Section::Devices,
            ],
        }
    }
}

impl Profile {
    /// Replaces the stored virtual devices, adding their section to a profile that had none.
    pub fn set_virtual_devices(&mut self, devices: VirtualDevices) {
        if !self.order.contains(&Section::VirtualDevices) {
            let at = self
                .order
                .iter()
                .position(|s| *s == Section::Devices)
                .map_or(self.order.len(), |i| i + 1);
            self.order.insert(at, Section::VirtualDevices);
        }
        self.virtual_devices = devices;
    }

    /// Whether the profile uses something only Joystick Gobloon has (modifiers and layers,
    /// light-led and map-to-gamepad actions, monotone cubic curves, stored virtual devices): it is then written in Joystick Gobloon's own format, which
    /// Joystick Gremlin cannot open.
    pub fn needs_own_format(&self) -> bool {
        !self.modifiers.is_empty()
            || !self.virtual_devices.is_empty()
            || self
                .inputs
                .iter()
                .flat_map(|b| &b.configs)
                .any(|c| c.additive || c.keep_value || !c.layer.is_base())
            || self.library.iter().any(|a| {
                a.kind == "light-led"
                    || a.kind == "map-to-gamepad"
                    || (a.kind == "response-curve"
                        && a.str("curve-type").map(crate::curve::CurveType::parse)
                            == Some(crate::curve::CurveType::MonotoneCubic))
            })
    }

    pub fn action(&self, id: &str) -> Option<&Action> {
        self.library.iter().find(|a| a.id == id)
    }

    /// Action lookup table; build once when resolving many references.
    pub fn action_index(&self) -> HashMap<&str, &Action> {
        self.library.iter().map(|a| (a.id.as_str(), a)).collect()
    }

    fn mode_parent(&self, mode: &str) -> Option<&str> {
        self.modes
            .iter()
            .find(|m| m.name == mode)
            .and_then(|m| m.parent.as_deref())
    }

    /// Whether `m` is `target` or inherits from it (bounded, in case of a parent loop).
    fn descends(&self, m: &str, target: &str) -> bool {
        let mut m = Some(m);
        for _ in 0..=self.modes.len() {
            match m {
                Some(x) if x == target => return true,
                Some(x) => m = self.mode_parent(x),
                None => return false,
            }
        }
        false
    }

    /// Whether modes `a` and `b` can run together: one is the other or inherits from it.
    pub fn modes_overlap(&self, a: &str, b: &str) -> bool {
        self.descends(a, b) || self.descends(b, a)
    }

    /// vJoy outputs (device, type, id) that map-to-vjoy actions write to while `mode` runs:
    /// those bound in the mode, the modes it inherits from and the modes inheriting from it.
    pub fn used_vjoy_outputs(&self, mode: &str) -> HashSet<(u32, InputType, u32)> {
        self.output_usage(Some(mode))
            .into_keys()
            .filter_map(|o| match o {
                VirtualOutput::Vjoy {
                    device,
                    input_type,
                    id,
                } => Some((device, input_type, id)),
                VirtualOutput::Gamepad { .. } => None,
            })
            .collect()
    }

    /// Virtual outputs that map-to-vjoy and map-to-gamepad actions write to, each with the
    /// bindings writing it (one entry per binding). `mode` limits them to the bindings that can
    /// run together with it ([`Self::modes_overlap`]); `None` takes every mode.
    pub fn output_usage(&self, mode: Option<&str>) -> HashMap<VirtualOutput, Vec<OutputUse>> {
        let mut usage: HashMap<VirtualOutput, Vec<OutputUse>> = HashMap::new();
        for (binding, b) in self.inputs.iter().enumerate() {
            if mode.is_some_and(|m| !self.modes_overlap(&b.mode, m)) {
                continue;
            }
            let roots: Vec<&str> = b.configs.iter().map(|c| c.root_action.as_str()).collect();
            for (action, output) in self.tree_outputs(&roots) {
                let uses = usage.entry(output).or_default();
                if uses.last().is_none_or(|u| u.binding != binding) {
                    uses.push(OutputUse { binding, action });
                }
            }
        }
        usage
    }

    /// Outputs written in the action trees under `roots`, with the action writing each (an
    /// action reached twice counts once).
    pub fn tree_outputs(&self, roots: &[&str]) -> Vec<(String, VirtualOutput)> {
        let index = self.action_index();
        let mut outputs = Vec::new();
        let mut seen = HashSet::new();
        let mut stack: Vec<&str> = roots.to_vec();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(action) = index.get(id) else {
                continue;
            };
            if let Some(output) = virtual_output(action) {
                outputs.push((id.to_string(), output));
            }
            stack.extend(action.child_ids());
        }
        outputs
    }

    /// After `moved` (a copy) was added to a binding: its map-to-vjoy actions that write an
    /// output another action of the same action tree already writes move to the first output of
    /// that type nothing in the tree writes, preferring one no input uses in the binding's modes.
    pub fn separate_outputs(&mut self, moved: &str) {
        let Some((root, mode)) = self.inputs.iter().find_map(|b| {
            b.configs
                .iter()
                .find(|c| self.reaches(&c.root_action, moved))
                .map(|c| (c.root_action.clone(), b.mode.clone()))
        }) else {
            return;
        };
        let mine: Vec<(String, VirtualOutput)> = self.tree_outputs(&[moved]);
        let ids: HashSet<&str> = mine.iter().map(|(id, _)| id.as_str()).collect();
        let mut taken: HashSet<VirtualOutput> = self
            .tree_outputs(&[&root])
            .into_iter()
            .filter(|(id, _)| !ids.contains(id.as_str()))
            .map(|(_, o)| o)
            .collect();
        let busy: HashSet<VirtualOutput> = self.output_usage(Some(&mode)).into_keys().collect();
        let mut changes = Vec::new();
        for (id, output) in mine {
            let VirtualOutput::Vjoy {
                device, input_type, ..
            } = output
            else {
                continue;
            };
            if !taken.contains(&output) {
                taken.insert(output);
                continue;
            }
            let max = match input_type {
                InputType::Axis => 8,
                InputType::Hat => 4,
                _ => 128,
            };
            let at = |n| VirtualOutput::Vjoy {
                device,
                input_type,
                id: n,
            };
            let free = (1..=max)
                .find(|&n| !taken.contains(&at(n)) && !busy.contains(&at(n)))
                .or_else(|| (1..=max).find(|&n| !taken.contains(&at(n))));
            if let Some(n) = free {
                taken.insert(at(n));
                changes.push((id, n));
            }
        }
        for (id, n) in changes {
            if let Some(a) = self.action_mut(&id) {
                a.set("vjoy-input-id", "int", n.to_string());
            }
        }
    }

    /// Whether two of `uses` (from [`Self::output_usage`]) are different physical inputs in modes
    /// that run together, so they fight over the output.
    pub fn uses_conflict(&self, uses: &[OutputUse]) -> bool {
        let bindings: Vec<&InputBinding> = uses
            .iter()
            .filter_map(|u| self.inputs.get(u.binding))
            .collect();
        bindings.iter().enumerate().any(|(i, a)| {
            bindings[i + 1..].iter().any(|b| {
                (&a.device_id, &a.input_type, &a.input_id)
                    != (&b.device_id, &b.input_type, &b.input_id)
                    && self.modes_overlap(&a.mode, &b.mode)
            })
        })
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
        for m in &mut self.modifiers {
            if m.device_id.eq_ignore_ascii_case(old) {
                m.device_id = new.to_string();
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
