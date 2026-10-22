//! Modifiers: inputs that, while held, choose which layer of an input's actions runs (issue #39).
//!
//! A profile names a few modifiers (`M1` = TWCS button 2, ...). Every action configuration of an
//! input belongs to a [`Layer`]: the set of modifiers that must be held for it. The base layer
//! (no modifiers) is what runs with no modifier held. The engine picks, when an input is used:
//! - the configurations whose layer is exactly the held set (nothing when there is none: an
//!   undefined combination does nothing, it does not fall back to the base layer);
//! - plus the *additive* ones whose layer is held (a subset of the held set): they run on top,
//!   e.g. an axis that always drives pitch and with M1 also the wheel brakes.
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

use std::fmt;

use crate::profile::{Profile, Section};

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

    pub fn names(&self) -> &[String] {
        &self.0
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|n| n == name)
    }

    /// Every modifier of this layer is in `held`.
    pub fn is_held(&self, held: &Layer) -> bool {
        self.0.iter().all(|n| held.contains(n))
    }

    /// Whether a configuration on this layer runs while `held` is held.
    pub fn runs(&self, additive: bool, held: &Layer) -> bool {
        if additive {
            self.is_held(held)
        } else {
            self == held
        }
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
mod tests {
    use super::*;

    fn modifier(name: &str, button: &str) -> Modifier {
        Modifier {
            name: name.into(),
            device_id: "{TWCS}".into(),
            input_type: "button".into(),
            input_id: button.into(),
        }
    }

    #[test]
    fn layers_are_sets() {
        let l = Layer::parse(" M2 + M1+M1 ");
        assert_eq!(l.to_string(), "M1+M2");
        assert_eq!(l, Layer::new(["M1", "M2"]));
        assert!(Layer::parse("").is_base());
        assert_eq!(Layer::base().to_string(), "");
    }

    #[test]
    fn exact_and_additive_layers() {
        let (none, m1, both) = (Layer::base(), Layer::parse("M1"), Layer::parse("M1+M2"));
        // Exclusive: only the exact combination.
        assert!(none.runs(false, &none));
        assert!(!none.runs(false, &m1));
        assert!(m1.runs(false, &m1));
        assert!(!m1.runs(false, &both));
        // Additive: whenever its modifiers are held.
        assert!(none.runs(true, &both));
        assert!(m1.runs(true, &both));
        assert!(!both.runs(true, &m1));
    }

    #[test]
    fn add_rename_remove() {
        let mut p = Profile::default();
        assert!(p.add_modifier(modifier("M1", "2")));
        assert!(!p.add_modifier(modifier("M1", "3")), "name taken");
        assert!(!p.add_modifier(modifier("M2", "2")), "input taken");
        assert!(!p.add_modifier(modifier("A+B", "3")), "separator");
        assert!(p.add_modifier(modifier("M2", "3")));
        assert!(p.order.contains(&Section::Modifiers));
        assert_eq!(p.modifier_at("{twcs}", "button", "3").unwrap().name, "M2");

        let base = p.bind("{T16000}", "button", "4", "Default");
        let layered = p.bind_layer("{T16000}", "button", "4", "Default", &Layer::parse("M1+M2"));
        assert_ne!(base, layered);
        assert_eq!(p.inputs[0].configs.len(), 2);

        assert!(p.rename_modifier("M2", "Shift"));
        assert_eq!(p.inputs[0].configs[1].layer.to_string(), "M1+Shift");
        assert!(!p.rename_modifier("M1", "Shift"), "name taken");

        assert!(!p.set_modifier_input("M1", "{TWCS}", "button", "3"));
        assert!(p.set_modifier_input("M1", "{TWCS}", "button", "5"));

        assert!(p.remove_modifier("Shift"));
        assert_eq!(p.inputs[0].configs.len(), 1);
        assert!(p.action(&layered).is_none(), "its actions are gone");
        assert!(p.action(&base).is_some());
    }
}
