//! Gremlin's logical device: virtual axes, buttons and hats defined in the profile.
//! `map-to-logical-device` actions write to them and they can be bound like a real
//! stick, under the fixed device GUID [`LOGICAL_DEVICE_GUID`].

use crate::action::InputType;
use crate::profile::Profile;
use crate::xml::Element;

/// Device id Gremlin uses for logical device bindings (`dill.UUID_LogicalDevice`).
pub const LOGICAL_DEVICE_GUID: &str = "f0af472f-8e17-493b-a1eb-7333ee8543f2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalInput {
    pub input_type: InputType,
    /// Unique per input type, from 1.
    pub id: u32,
    pub label: String,
}

fn type_name(t: InputType) -> &'static str {
    match t {
        InputType::Axis => "axis",
        InputType::Hat => "hat",
        _ => "button",
    }
}

impl Profile {
    /// Inputs of the logical device in file order.
    pub fn logical_inputs(&self) -> Vec<LogicalInput> {
        self.logical_device
            .elements()
            .filter(|e| e.name == "input")
            .filter_map(|e| {
                let input_type = InputType::parse(e.child_text("input-type")?.trim());
                let id = e.child_text("input-id")?.trim().parse().ok()?;
                let label = e.child_text("label").unwrap_or_default();
                (input_type != InputType::Other).then_some(LogicalInput {
                    input_type,
                    id,
                    label,
                })
            })
            .collect()
    }

    fn set_logical_inputs(&mut self, inputs: &[LogicalInput]) {
        let mut node = Element::new("logical-device");
        for i in inputs {
            let mut e = Element::new("input");
            e.push(Element::with_text("input-type", type_name(i.input_type)));
            e.push(Element::with_text("input-id", i.id.to_string()));
            e.push(Element::with_text("label", i.label.clone()));
            node.push(e);
        }
        self.logical_device = node;
    }

    /// Adds an input with the lowest free id and a label like Gremlin's ("Axis 2"); returns its id.
    pub fn add_logical_input(&mut self, input_type: InputType) -> u32 {
        let mut inputs = self.logical_inputs();
        let id = (1..)
            .find(|n| {
                !inputs
                    .iter()
                    .any(|i| i.input_type == input_type && i.id == *n)
            })
            .unwrap_or(1);
        let base = match input_type {
            InputType::Axis => "Axis",
            InputType::Hat => "Hat",
            _ => "Button",
        };
        let mut label = format!("{base} {id}");
        let mut n = 2;
        while inputs.iter().any(|i| i.label == label) {
            label = format!("{base} {id} ({n})");
            n += 1;
        }
        inputs.push(LogicalInput {
            input_type,
            id,
            label,
        });
        self.set_logical_inputs(&inputs);
        id
    }

    /// Labels are unique like in Gremlin; returns false when the label is taken or empty.
    pub fn rename_logical_input(&mut self, input_type: InputType, id: u32, label: &str) -> bool {
        let label = label.trim();
        let mut inputs = self.logical_inputs();
        let taken = inputs
            .iter()
            .any(|i| i.label == label && !(i.input_type == input_type && i.id == id));
        if label.is_empty() || taken {
            return false;
        }
        match inputs
            .iter_mut()
            .find(|i| i.input_type == input_type && i.id == id)
        {
            Some(i) => i.label = label.to_string(),
            None => return false,
        }
        self.set_logical_inputs(&inputs);
        true
    }

    /// Removes the input and its bindings (in every mode).
    pub fn remove_logical_input(&mut self, input_type: InputType, id: u32) {
        let mut inputs = self.logical_inputs();
        inputs.retain(|i| !(i.input_type == input_type && i.id == id));
        self.set_logical_inputs(&inputs);
        let (ty, id) = (type_name(input_type), id.to_string());
        self.inputs.retain(|b| {
            !(b.device_id.eq_ignore_ascii_case(LOGICAL_DEVICE_GUID)
                && b.input_type == ty
                && b.input_id.trim() == id)
        });
        self.collect_garbage();
    }
}
