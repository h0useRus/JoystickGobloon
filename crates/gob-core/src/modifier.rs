//! Modifiers: inputs that, while held, choose which layer of an input's actions runs (issue #39).
//!
//! A profile names a few modifiers (`M1` = button 2 of one stick, ...). Every action configuration of an
//! input belongs to a [`Layer`]: the set of modifiers that must be held for it. The base layer
//! (no modifiers) is what runs with no modifier held. When an input is used, the engine picks
//! (see [`Selection`]):
//! - the configurations whose layer is exactly the held set; other layers never join, M1 does not
//!   run while M1+M2 is held;
//! - plus the base when that layer is *additive* ("add to the base"), e.g. an axis that always
//!   drives one output and, while M1 is held, a second one too;
//! - for a held set without a layer: nothing (an undefined combination does not fall back to the
//!   base), unless the input has no exclusive (non-additive) layers. Such an input keeps its base
//!   (an axis with only a base, or a base plus additive M1, keeps working while M2 is held).
//!
//! A button chooses its layer when pressed and its release goes to the same layer, whatever the
//! modifiers do meanwhile. Modifier inputs only switch layers; bindings on them are not run.
//!
//! In the profile (`<modifiers>` and attributes of `<action-configuration>`):
//! ```xml
//! <modifiers>
//!   <modifier name="M1" device-id="{...}" input-type="button" input-id="2"/>
//! </modifiers>
//! <action-configuration layer="M1+M2" additive="true">...</action-configuration>
//! ```
//!
//! When the held modifiers change, the axis outputs of a layer that stops go back to rest (an
//! additive brake axis is released with its modifier), unless the layer keeps its value
//! (`keep-value="true"`, e.g. a trim axis on M1, or the base flight axis while M1 trims).

use std::fmt;

use crate::profile::{ActionConfig, Profile, Section};

/// Separator of modifier names in a layer (`M1+M2`); not allowed in names.
pub const SEPARATOR: char = '+';

/// A named input that selects layers while held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modifier {
    pub name: String,
    /// DirectInput instance GUID of the device (keyboard: `KEYBOARD_GUID`).
    pub device_id: String,
    /// `button` or `key`.
    pub input_type: String,
    pub input_id: String,
}

impl Modifier {
    /// Whether this modifier is the given input.
    pub fn is_input(&self, device_id: &str, input_type: &str, input_id: &str) -> bool {
        self.device_id.eq_ignore_ascii_case(device_id)
            && self.input_type == input_type
            && self.input_id == input_id
    }
}

/// The modifiers a configuration needs, sorted and without duplicates; empty is the base layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Layer(Vec<String>);

impl Layer {
    pub fn new<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Self {
        let mut names: Vec<String> = names
            .into_iter()
            .map(Into::into)
            .map(|n: String| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect();
        names.sort();
        names.dedup();
        Self(names)
    }

    /// The base layer.
    pub fn base() -> Self {
        Self::default()
    }

    /// `M1+M2`; empty for the base layer.
    pub fn parse(text: &str) -> Self {
        Self::new(text.split(SEPARATOR))
    }

    pub fn is_base(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|n| n == name)
    }

    fn rename(&mut self, old: &str, new: &str) {
        if self.contains(old) {
            *self = Self::new(self.0.iter().map(|n| if n == old { new } else { n }));
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join(&SEPARATOR.to_string()))
    }
}

/// Which configurations of one input run while some modifiers are held (rules in the module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The held layer, when the input has it.
    layer: Option<Layer>,
    /// The base runs too.
    base: bool,
}

impl Selection {
    /// `configs`: the layer of each configuration of the input and whether it is additive.
    pub fn new<'a>(configs: impl IntoIterator<Item = (&'a Layer, bool)>, held: &Layer) -> Self {
        let (mut found, mut adds, mut exclusive) = (false, false, false);
        for (layer, additive) in configs {
            if layer == held {
                found = true;
                adds |= additive;
            }
            exclusive |= !additive && !layer.is_base();
        }
        if found {
            Self {
                layer: Some(held.clone()),
                base: adds,
            }
        } else {
            Self {
                layer: None,
                base: !exclusive,
            }
        }
    }

    /// Whether the configurations on `layer` run.
    pub fn runs(&self, layer: &Layer) -> bool {
        self.layer.as_ref() == Some(layer) || (self.base && layer.is_base())
    }
}

/// Whether `name` can name a modifier: not empty, no separator, no surrounding spaces.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.trim() == name && !name.contains(SEPARATOR)
}

impl Profile {
    pub fn modifier(&self, name: &str) -> Option<&Modifier> {
        self.modifiers.iter().find(|m| m.name == name)
    }

    /// The modifier that is this input, if any.
    pub fn modifier_at(
        &self,
        device_id: &str,
        input_type: &str,
        input_id: &str,
    ) -> Option<&Modifier> {
        self.modifiers
            .iter()
            .find(|m| m.is_input(device_id, input_type, input_id))
    }

    /// Adds a modifier; refuses an invalid or taken name and an input that is a modifier already.
    pub fn add_modifier(&mut self, modifier: Modifier) -> bool {
        if !valid_name(&modifier.name)
            || self.modifier(&modifier.name).is_some()
            || self
                .modifier_at(
                    &modifier.device_id,
                    &modifier.input_type,
                    &modifier.input_id,
                )
                .is_some()
        {
            return false;
        }
        if !self.order.contains(&Section::Modifiers) {
            let at = self
                .order
                .iter()
                .position(|s| *s == Section::Modes)
                .map_or(self.order.len(), |i| i + 1);
            self.order.insert(at, Section::Modifiers);
        }
        self.modifiers.push(modifier);
        true
    }

    /// Moves a modifier to another input; refuses one that is another modifier already.
    pub fn set_modifier_input(
        &mut self,
        name: &str,
        device_id: &str,
        input_type: &str,
        input_id: &str,
    ) -> bool {
        if self
            .modifier_at(device_id, input_type, input_id)
            .is_some_and(|m| m.name != name)
        {
            return false;
        }
        match self.modifiers.iter_mut().find(|m| m.name == name) {
            Some(m) => {
                m.device_id = device_id.into();
                m.input_type = input_type.into();
                m.input_id = input_id.into();
                true
            }
            None => false,
        }
    }

    /// Renames a modifier and the layers that use it.
    pub fn rename_modifier(&mut self, old: &str, new: &str) -> bool {
        if !valid_name(new) || self.modifier(new).is_some() {
            return false;
        }
        let Some(m) = self.modifiers.iter_mut().find(|m| m.name == old) else {
            return false;
        };
        m.name = new.into();
        for c in self.inputs.iter_mut().flat_map(|b| b.configs.iter_mut()) {
            c.layer.rename(old, new);
        }
        true
    }

    /// Removes the configurations of one layer of an input in a mode (the whole binding when no
    /// layer is left) and every action only they used.
    pub fn unbind_layer(
        &mut self,
        device_id: &str,
        input_type: &str,
        input_id: &str,
        mode: &str,
        layer: &Layer,
    ) {
        for b in self.inputs.iter_mut().filter(|b| {
            b.device_id.eq_ignore_ascii_case(device_id)
                && b.input_type == input_type
                && b.input_id == input_id
                && b.mode == mode
        }) {
            b.configs.retain(|c| c.layer != *layer);
        }
        self.inputs.retain(|b| !b.configs.is_empty());
        self.collect_garbage();
    }

    /// The configurations of one layer of an input in a mode.
    fn layer_configs_mut<'a>(
        &'a mut self,
        device_id: &'a str,
        input_type: &'a str,
        input_id: &'a str,
        mode: &'a str,
        layer: &'a Layer,
    ) -> impl Iterator<Item = &'a mut ActionConfig> {
        self.inputs
            .iter_mut()
            .filter(move |b| {
                b.device_id.eq_ignore_ascii_case(device_id)
                    && b.input_type == input_type
                    && b.input_id == input_id
                    && b.mode == mode
            })
            .flat_map(|b| b.configs.iter_mut())
            .filter(move |c| c.layer == *layer)
    }

    /// Makes one layer of an input in a mode additive (runs on top) or exclusive.
    pub fn set_layer_additive(
        &mut self,
        device_id: &str,
        input_type: &str,
        input_id: &str,
        mode: &str,
        layer: &Layer,
        additive: bool,
    ) {
        for c in self.layer_configs_mut(device_id, input_type, input_id, mode, layer) {
            c.additive = additive;
        }
    }

    /// Makes the axis outputs of one layer of an input in a mode keep their last value when the
    /// layer stops, or go back to rest.
    pub fn set_layer_keep_value(
        &mut self,
        device_id: &str,
        input_type: &str,
        input_id: &str,
        mode: &str,
        layer: &Layer,
        keep: bool,
    ) {
        for c in self.layer_configs_mut(device_id, input_type, input_id, mode, layer) {
            c.keep_value = keep;
        }
    }

    /// Makes a button held when the profile starts run the press of one layer of an input in a
    /// mode then (or not).
    pub fn set_layer_send_state(
        &mut self,
        device_id: &str,
        input_type: &str,
        input_id: &str,
        mode: &str,
        layer: &Layer,
        send: bool,
    ) {
        for c in self.layer_configs_mut(device_id, input_type, input_id, mode, layer) {
            c.send_state = send;
        }
    }

    /// Removes a modifier and every configuration of a layer that needs it.
    pub fn remove_modifier(&mut self, name: &str) -> bool {
        let before = self.modifiers.len();
        self.modifiers.retain(|m| m.name != name);
        if self.modifiers.len() == before {
            return false;
        }
        for b in &mut self.inputs {
            b.configs.retain(|c| !c.layer.contains(name));
        }
        self.inputs.retain(|b| !b.configs.is_empty());
        self.collect_garbage();
        true
    }
}

#[cfg(test)]
mod tests;
