//! `inputs.gob`: which input each control part sends. A plugin ships the device's default
//! numbering; a user's taught numbering (`<data dir>/devices/<id>.gob`, same format) replaces it
//! control by control.
//!
//! ```toml
//! red_button = "button:3"
//! left_hat = "hat:1"            # a hat control on a POV hat: its four directions
//! [trigger]
//! first = "button:1"
//! second = "button:2"
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::input::{Direction, Input};

/// One control's inputs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Entry {
    /// A control with one part, or a hat control on a whole POV hat.
    One(Input),
    /// Part name -> input.
    Parts(BTreeMap<String, Input>),
}

/// Control id -> its inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Inputs(pub BTreeMap<String, Entry>);

impl Inputs {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.message().to_string())
    }

    /// Reads `path`; a missing file is no inputs.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn to_text(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// Writes `path` whole (temp file + rename), creating its folder.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        gob_core::file::write_whole(path, self.to_text().as_bytes())
    }

    /// The input of `part` of `control` ("" = a control with one part).
    pub fn get(&self, control: &str, part: &str) -> Option<Input> {
        match self.0.get(control)? {
            Entry::One(input) if part.is_empty() => Some(*input),
            Entry::One(Input::Hat(n)) => part
                .parse::<Direction>()
                .ok()
                .map(|d| Input::HatDirection(*n, d)),
            Entry::One(_) => None,
            Entry::Parts(parts) => parts.get(part).copied(),
        }
    }

    /// Sets one part, keeping the control's other parts.
    pub fn set(&mut self, control: &str, part: &str, input: Input) {
        if part.is_empty() {
            self.0.insert(control.to_string(), Entry::One(input));
            return;
        }
        let mut parts = match self.0.remove(control) {
            Some(Entry::Parts(parts)) => parts,
            Some(Entry::One(Input::Hat(n))) => Direction::ALL
                .into_iter()
                .map(|d| (d.name().to_string(), Input::HatDirection(n, d)))
                .collect(),
            _ => BTreeMap::new(),
        };
        parts.insert(part.to_string(), input);
        self.0.insert(control.to_string(), Entry::Parts(parts));
    }

    /// Takes the input off `part` of `control` ("" = the whole control).
    pub fn clear(&mut self, control: &str, part: &str) {
        if part.is_empty() {
            self.0.remove(control);
            return;
        }
        let mut parts = match self.0.remove(control) {
            Some(Entry::Parts(parts)) => parts,
            Some(Entry::One(Input::Hat(n))) => Direction::ALL
                .into_iter()
                .map(|d| (d.name().to_string(), Input::HatDirection(n, d)))
                .collect(),
            Some(one) => {
                self.0.insert(control.to_string(), one);
                return;
            }
            None => return,
        };
        parts.remove(part);
        if !parts.is_empty() {
            self.0.insert(control.to_string(), Entry::Parts(parts));
        }
    }

    /// Every control part `input` is on (a whole hat counts for each of its directions).
    pub fn parts_with(&self, input: Input) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (control, entry) in &self.0 {
            match entry {
                Entry::One(i) if *i == input => out.push((control.clone(), String::new())),
                Entry::One(Input::Hat(n)) => {
                    if let Input::HatDirection(m, d) = input
                        && m == *n
                    {
                        out.push((control.clone(), d.name().to_string()));
                    }
                }
                Entry::One(_) => {}
                Entry::Parts(parts) => out.extend(
                    parts
                        .iter()
                        .filter(|(_, i)| **i == input)
                        .map(|(p, _)| (control.clone(), p.clone())),
                ),
            }
        }
        out
    }

    /// Takes `input` off every control part it is on.
    pub fn remove_input(&mut self, input: Input) {
        for (control, part) in self.parts_with(input) {
            self.clear(&control, &part);
        }
    }

    /// Writes a hat control whose four directions are one POV hat's as `hat:N`.
    pub fn compact_hat(&mut self, control: &str) {
        let Some(Entry::Parts(parts)) = self.0.get(control) else {
            return;
        };
        let Some(Input::HatDirection(hat, Direction::Up)) = parts.get("up") else {
            return;
        };
        let hat = *hat;
        let pov = parts.len() == 4
            && Direction::ALL
                .iter()
                .all(|d| parts.get(d.name()) == Some(&Input::HatDirection(hat, *d)));
        if pov {
            self.0
                .insert(control.to_string(), Entry::One(Input::Hat(hat)));
        }
    }

    /// Renames a control's entry (its control was renamed).
    pub fn rename(&mut self, from: &str, to: &str) {
        if let Some(entry) = self.0.remove(from) {
            self.0.insert(to.to_string(), entry);
        }
    }

    /// `other`'s controls replace these, control by control.
    pub fn overlay(&mut self, other: &Inputs) {
        for (id, entry) in &other.0 {
            self.0.insert(id.clone(), entry.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"
red_button = "button:3"
left_hat = "hat:1"
[trigger]
first = "button:1"
second = "button:2"
"#;

    #[test]
    fn parses_and_resolves() {
        let inputs = Inputs::parse(TEXT).unwrap();
        assert_eq!(inputs.get("red_button", ""), Some(Input::Button(3)));
        assert_eq!(inputs.get("trigger", "second"), Some(Input::Button(2)));
        assert_eq!(inputs.get("trigger", ""), None);
        assert_eq!(
            inputs.get("left_hat", "left"),
            Some(Input::HatDirection(1, Direction::Left))
        );
        assert_eq!(inputs.get("left_hat", "push"), None);
        assert_eq!(Inputs::parse(&inputs.to_text()).unwrap(), inputs);
    }

    #[test]
    fn set_and_overlay() {
        let mut inputs = Inputs::parse(TEXT).unwrap();
        inputs.set("left_hat", "push", Input::Button(9));
        assert_eq!(inputs.get("left_hat", "push"), Some(Input::Button(9)));
        assert_eq!(
            inputs.get("left_hat", "up"),
            Some(Input::HatDirection(1, Direction::Up))
        );
        let mut taught = Inputs::default();
        taught.set("trigger", "first", Input::Button(5));
        inputs.overlay(&taught);
        assert_eq!(inputs.get("trigger", "first"), Some(Input::Button(5)));
        assert_eq!(inputs.get("trigger", "second"), None);
    }

    #[test]
    fn clear_remove_and_compact() {
        let mut inputs = Inputs::parse(TEXT).unwrap();
        inputs.remove_input(Input::HatDirection(1, Direction::Left));
        assert_eq!(inputs.get("left_hat", "left"), None);
        assert_eq!(
            inputs.get("left_hat", "up"),
            Some(Input::HatDirection(1, Direction::Up))
        );
        inputs.set("left_hat", "left", Input::HatDirection(1, Direction::Left));
        inputs.compact_hat("left_hat");
        assert_eq!(inputs.0.get("left_hat"), Some(&Entry::One(Input::Hat(1))));
        inputs.clear("trigger", "first");
        inputs.clear("trigger", "second");
        assert!(!inputs.0.contains_key("trigger"));
        assert_eq!(
            inputs.parts_with(Input::Button(3)),
            vec![("red_button".to_string(), String::new())]
        );
        inputs.rename("red_button", "fire");
        assert_eq!(inputs.get("fire", ""), Some(Input::Button(3)));
    }
}
