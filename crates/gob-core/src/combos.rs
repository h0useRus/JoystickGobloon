//! Key combinations (Joystick Gobloon only): several keyboard keys held together act as one
//! button. Each combination is an input of the keyboard device ([`crate::KEYBOARD_GUID`],
//! input type `combo`, input id = the combination's id) and is bound to actions like any key.
//! It goes down when the last of its keys goes down (in any order) and up when any of them
//! goes up; the keys' own bindings run as well.

use crate::profile::{Profile, Section};

/// Input type of key combination bindings.
pub const COMBO_INPUT_TYPE: &str = "combo";

/// Most keys a combination can have.
pub const MAX_COMBO_KEYS: usize = 5;

/// One key combination (`<combo>` in `<key-combos>`).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyCombo {
    /// Its input id, unique, from 1.
    pub id: u32,
    /// Its keys, each once, in the order they were recorded.
    pub keys: Vec<crate::Key>,
}

/// Whether a binding belongs to a key combination (padded hand-edited values too).
pub fn is_combo_binding(device_id: &str, input_type: &str) -> bool {
    device_id.trim().eq_ignore_ascii_case(crate::KEYBOARD_GUID)
        && input_type.trim() == COMBO_INPUT_TYPE
}

/// `keys` each once, at most [`MAX_COMBO_KEYS`].
fn tidy(keys: &[crate::Key]) -> Vec<crate::Key> {
    let mut out: Vec<crate::Key> = Vec::new();
    for k in keys {
        if !out.contains(k) && out.len() < MAX_COMBO_KEYS {
            out.push(*k);
        }
    }
    out
}

impl Profile {
    pub fn key_combo(&self, id: u32) -> Option<&KeyCombo> {
        self.key_combos.iter().find(|c| c.id == id)
    }

    /// The combination with exactly these keys (in any order).
    pub fn key_combo_with(&self, keys: &[crate::Key]) -> Option<&KeyCombo> {
        let keys = tidy(keys);
        self.key_combos
            .iter()
            .find(|c| c.keys.len() == keys.len() && keys.iter().all(|k| c.keys.contains(k)))
    }

    /// Adds a combination of `keys` (two or more) with the lowest free id, or finds the one
    /// that has them already; returns its id.
    pub fn add_key_combo(&mut self, keys: &[crate::Key]) -> Option<u32> {
        let keys = tidy(keys);
        if keys.len() < 2 {
            return None;
        }
        if let Some(c) = self.key_combo_with(&keys) {
            return Some(c.id);
        }
        let id = (1..).find(|n| self.key_combo(*n).is_none()).unwrap_or(1);
        self.key_combos.push(KeyCombo { id, keys });
        self.add_section(Section::KeyCombos);
        Some(id)
    }

    /// Replaces a combination's keys (two or more, not those of another combination); false
    /// otherwise.
    pub fn set_combo_keys(&mut self, id: u32, keys: &[crate::Key]) -> bool {
        let keys = tidy(keys);
        if keys.len() < 2 || self.key_combo_with(&keys).is_some_and(|c| c.id != id) {
            return false;
        }
        match self.key_combos.iter_mut().find(|c| c.id == id) {
            Some(c) => {
                c.keys = keys;
                true
            }
            None => false,
        }
    }

    /// Removes a combination and its bindings (with the actions only they used).
    pub fn remove_key_combo(&mut self, id: u32) {
        self.key_combos.retain(|c| c.id != id);
        self.inputs.retain(|b| {
            !(is_combo_binding(&b.device_id, &b.input_type) && b.input_id.trim().parse() == Ok(id))
        });
        self.collect_garbage();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Key;

    const CTRL: Key = Key {
        scan_code: 0x1d,
        extended: false,
    };
    const F1: Key = Key {
        scan_code: 0x3b,
        extended: false,
    };
    const SHIFT: Key = Key {
        scan_code: 0x2a,
        extended: false,
    };

    #[test]
    fn combos_are_found_by_their_keys_and_removed_with_their_bindings() {
        let mut p = Profile::default();
        assert_eq!(p.add_key_combo(&[CTRL]), None, "one key is a key");
        let id = p.add_key_combo(&[CTRL, F1, CTRL]).unwrap();
        assert_eq!(p.key_combo(id).unwrap().keys, [CTRL, F1]);
        assert_eq!(
            p.add_key_combo(&[F1, CTRL]),
            Some(id),
            "same keys, same combination"
        );
        let other = p.add_key_combo(&[SHIFT, F1]).unwrap();
        assert!(!p.set_combo_keys(other, &[CTRL, F1]), "taken by another");
        assert!(p.set_combo_keys(other, &[CTRL, SHIFT, F1]));
        p.bind(
            crate::KEYBOARD_GUID,
            COMBO_INPUT_TYPE,
            &id.to_string(),
            "Default",
        );
        let actions = p.library.len();
        p.remove_key_combo(id);
        assert!(p.key_combo(id).is_none());
        assert!(p.inputs.is_empty());
        assert!(p.library.len() < actions, "its actions go");
        assert!(p.order.contains(&Section::KeyCombos));
    }
}
