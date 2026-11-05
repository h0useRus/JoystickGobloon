//! Voice commands (Joystick Gobloon only): spoken phrases that act like buttons. Each command is
//! an input of the voice device [`VOICE_GUID`] (input type `voice`, input id = the command's id)
//! and is bound to actions like any button. The microphone listens only while an input with a
//! `voice-input` action is held.

use crate::action::ActionKind;
use crate::profile::Profile;

/// Device id of voice command bindings.
pub const VOICE_GUID: &str = "5f8e2c1a-7b3d-4e9a-9c6f-2d4b8a1e7c30";

/// Input type of voice command bindings.
pub const VOICE_INPUT_TYPE: &str = "voice";

/// How long a recognized command holds its button down when nothing else is set, in seconds.
pub const DEFAULT_HOLD: f64 = 0.1;

/// One voice command (`<command>` in `<voice-commands>`).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceCommand {
    /// Its input id, unique, from 1.
    pub id: u32,
    /// What can be said for it; any of them triggers it.
    pub phrases: Vec<String>,
    /// Seconds the command's button stays pressed after it was recognized.
    pub hold: f64,
}

/// A binding whose actions start listening ([`Profile::voice_inputs`]).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceInput {
    /// Index into [`Profile::inputs`].
    pub binding: usize,
    /// The `voice-input` action.
    pub action: String,
    /// Microphone id; empty = the system's default one.
    pub microphone: String,
}

impl Profile {
    pub fn voice_command(&self, id: u32) -> Option<&VoiceCommand> {
        self.voice_commands.iter().find(|c| c.id == id)
    }

    /// The id [`Profile::add_voice_command`] gives the next command: the lowest free one.
    pub fn next_voice_command_id(&self) -> u32 {
        (1..)
            .find(|n| self.voice_command(*n).is_none())
            .unwrap_or(1)
    }

    /// Adds a command with the lowest free id and no phrases yet; returns its id.
    pub fn add_voice_command(&mut self) -> u32 {
        let id = self.next_voice_command_id();
        self.voice_commands.push(VoiceCommand {
            id,
            phrases: Vec::new(),
            hold: DEFAULT_HOLD,
        });
        self.add_section(crate::profile::Section::VoiceCommands);
        id
    }

    /// Replaces a command's phrases: trimmed, inner spaces collapsed, empty and repeated ones
    /// left out. False when there is no such command.
    pub fn set_voice_phrases(&mut self, id: u32, phrases: &[String]) -> bool {
        let Some(command) = self.voice_commands.iter_mut().find(|c| c.id == id) else {
            return false;
        };
        command.phrases.clear();
        for p in phrases {
            let p = p.split_whitespace().collect::<Vec<_>>().join(" ");
            if !p.is_empty() && !command.phrases.contains(&p) {
                command.phrases.push(p);
            }
        }
        true
    }

    /// Seconds the command holds its button (0.01..=10); false when there is no such command.
    pub fn set_voice_hold(&mut self, id: u32, hold: f64) -> bool {
        match self.voice_commands.iter_mut().find(|c| c.id == id) {
            Some(c) if hold.is_finite() => {
                c.hold = hold.clamp(0.01, 10.0);
                true
            }
            _ => false,
        }
    }

    /// Where a card's voice command saying `phrase` goes: `Ok` with the command already saying
    /// it that runs nothing (its link was removed), else the next new one; `Err` with the
    /// command that already says it and runs other actions (the phrase is taken). Case and
    /// extra spaces are ignored.
    pub fn voice_command_for_phrase(&self, phrase: &str) -> Result<u32, u32> {
        let norm = |p: &str| {
            p.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        };
        let phrase = norm(phrase);
        match self
            .voice_commands
            .iter()
            .find(|c| c.phrases.iter().any(|p| norm(p) == phrase))
        {
            Some(c) if self.voice_roots(c.id).any(|r| !r.child_ids().is_empty()) => Err(c.id),
            Some(c) => Ok(c.id),
            None => Ok(self.next_voice_command_id()),
        }
    }

    /// Makes a voice command saying `phrase` run the library action `action` itself (linked,
    /// not copied) in `mode` and `layer`, like the card it comes from: the command that already
    /// says it and runs nothing, else a new one. Returns its id, or `None` when another command
    /// already says `phrase` and runs other actions.
    pub fn add_voice_command_for(
        &mut self,
        action: &str,
        phrase: &str,
        mode: &str,
        layer: &crate::Layer,
    ) -> Option<u32> {
        let id = self.voice_command_for_phrase(phrase).ok()?;
        if self.voice_command(id).is_none() {
            self.add_voice_command();
            self.set_voice_phrases(id, &[phrase.to_string()]);
        }
        let root = self.bind_layer(VOICE_GUID, VOICE_INPUT_TYPE, &id.to_string(), mode, layer);
        self.add_ref(&root, "actions", action);
        Some(id)
    }

    /// Root actions of command `id`'s bindings (every mode and layer).
    fn voice_roots(&self, id: u32) -> impl Iterator<Item = &crate::Action> {
        self.inputs
            .iter()
            .filter(move |b| {
                is_voice_binding(&b.device_id, &b.input_type) && b.input_id.trim().parse() == Ok(id)
            })
            .flat_map(|b| &b.configs)
            .filter_map(|c| self.action(&c.root_action))
    }

    /// Every action a voice command runs, anywhere among its actions, with the command's id
    /// (the first command when several run it). Bindings of removed commands do not count.
    pub fn voice_links(&self) -> std::collections::HashMap<String, u32> {
        let index = self.action_index();
        let mut links = std::collections::HashMap::new();
        for b in &self.inputs {
            if !is_voice_binding(&b.device_id, &b.input_type) {
                continue;
            }
            let Some(id) = b
                .input_id
                .trim()
                .parse()
                .ok()
                .filter(|id| self.voice_command(*id).is_some())
            else {
                continue;
            };
            for c in &b.configs {
                let mut seen = std::collections::HashSet::new();
                let mut stack: Vec<&str> = index
                    .get(c.root_action.as_str())
                    .map(|r| r.child_ids())
                    .unwrap_or_default();
                while let Some(a) = stack.pop() {
                    if !seen.insert(a) {
                        continue;
                    }
                    links.entry(a.to_string()).or_insert(id);
                    if let Some(action) = index.get(a) {
                        stack.extend(action.child_ids());
                    }
                }
            }
        }
        links
    }

    /// The command that runs `action` (see [`Profile::voice_links`]), if any.
    pub fn voice_command_running(&self, action: &str) -> Option<u32> {
        self.voice_links().get(action).copied()
    }

    /// Removes the command and its bindings (in every mode).
    pub fn remove_voice_command(&mut self, id: u32) {
        self.voice_commands.retain(|c| c.id != id);
        let id = id.to_string();
        self.inputs.retain(|b| {
            !(is_voice_binding(&b.device_id, &b.input_type) && b.input_id.trim() == id)
        });
        self.collect_garbage();
    }

    /// Every phrase with the id of its command, in command order: what the recognizer listens
    /// for while the profile runs.
    pub fn voice_phrases(&self) -> Vec<(u32, &str)> {
        self.voice_commands
            .iter()
            .flat_map(|c| c.phrases.iter().map(move |p| (c.id, p.as_str())))
            .collect()
    }

    /// The bindings with a `voice-input` action anywhere in their action trees, in binding order.
    pub fn voice_inputs(&self) -> Vec<VoiceInput> {
        let index = self.action_index();
        let mut found = Vec::new();
        for (binding, b) in self.inputs.iter().enumerate() {
            let mut stack: Vec<&str> = b.configs.iter().map(|c| c.root_action.as_str()).collect();
            let mut seen = std::collections::HashSet::new();
            while let Some(id) = stack.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(action) = index.get(id) else {
                    continue;
                };
                if let ActionKind::VoiceInput { microphone } = action.kind() {
                    found.push(VoiceInput {
                        binding,
                        action: action.id.clone(),
                        microphone: microphone.to_string(),
                    });
                }
                stack.extend(action.child_ids());
            }
        }
        found
    }
}

/// Whether a binding with this device and input type is a voice command's.
pub fn is_voice_binding(device_id: &str, input_type: &str) -> bool {
    device_id.trim().eq_ignore_ascii_case(VOICE_GUID) && input_type.trim() == VOICE_INPUT_TYPE
}
