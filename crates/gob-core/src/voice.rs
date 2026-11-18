//! Voice commands (Joystick Gobloon only): spoken phrases that act like buttons. Each command is
//! an input of the voice device [`VOICE_GUID`] (input type `voice`, input id = the command's id)
//! and is bound to actions like any button. The microphone listens only while an input with a
//! `voice-input` action is held.
//!
//! Commands can sit in groups, and groups in groups: a command is said with its groups' phrases
//! before its own ("radio wing a attack"), in one go or a group at a time. Saying a group opens it
//! for a moment: then only what is in it is listened for. A group is an input too; its actions run
//! when it opens (a spoken answer, a sound).

use crate::action::ActionKind;
use crate::profile::Profile;

/// Device id of voice command bindings.
pub const VOICE_GUID: &str = "5f8e2c1a-7b3d-4e9a-9c6f-2d4b8a1e7c30";

/// Input type of voice command bindings.
pub const VOICE_INPUT_TYPE: &str = "voice";

/// How long a recognized command holds its button down when nothing else is set, in seconds.
pub const DEFAULT_HOLD: f64 = 0.1;

/// One voice command (`<command>` in `<voice-commands>`) or group of them (`<group>`).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceCommand {
    /// Its input id, unique, from 1 (groups and commands share them).
    pub id: u32,
    /// What can be said for it; any of them triggers it.
    pub phrases: Vec<String>,
    /// Seconds the command's button stays pressed after it was recognized.
    pub hold: f64,
    /// The group it is in; `None` = the top level.
    pub parent: Option<u32>,
    /// A group: holds commands and groups, and opens when said.
    pub group: bool,
}

impl VoiceCommand {
    /// A command at the top level with no phrases yet.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            phrases: Vec::new(),
            hold: DEFAULT_HOLD,
            parent: None,
            group: false,
        }
    }
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

    /// Adds a command with the lowest free id and no phrases yet at the top level; returns its
    /// id.
    pub fn add_voice_command(&mut self) -> u32 {
        self.add_voice_node(None, false)
    }

    /// Adds a command or a group (`group`) with no phrases into group `parent` (`None` or not a
    /// group = the top level); returns its id.
    pub fn add_voice_node(&mut self, parent: Option<u32>, group: bool) -> u32 {
        let id = self.next_voice_command_id();
        let parent = parent.filter(|p| self.voice_command(*p).is_some_and(|c| c.group));
        self.voice_commands.push(VoiceCommand {
            parent,
            group,
            ..VoiceCommand::new(id)
        });
        self.add_section(crate::profile::Section::VoiceCommands);
        id
    }

    /// The commands and groups right in group `parent` (`None` = the top level), in order.
    pub fn voice_children(&self, parent: Option<u32>) -> impl Iterator<Item = &VoiceCommand> {
        self.voice_commands
            .iter()
            .filter(move |c| c.parent == parent)
    }

    /// Every command and group in group `id`, at any depth, in tree order.
    pub fn voice_descendants(&self, id: u32) -> Vec<u32> {
        let tree = self.voice_tree();
        let Some(start) = tree.iter().position(|(_, c)| c.id == id) else {
            return Vec::new();
        };
        let depth = tree[start].0;
        tree[start + 1..]
            .iter()
            .take_while(|(d, _)| *d > depth)
            .map(|(_, c)| c.id)
            .collect()
    }

    /// Every command and group in tree order (a group, then what is in it), with its depth from
    /// 0. One whose group is missing counts as top level.
    pub fn voice_tree(&self) -> Vec<(usize, &VoiceCommand)> {
        fn walk<'a>(
            p: &'a Profile,
            parent: Option<u32>,
            depth: usize,
            out: &mut Vec<(usize, &'a VoiceCommand)>,
        ) {
            for c in p.voice_children(parent) {
                if out.iter().any(|(_, o)| o.id == c.id) {
                    continue;
                }
                out.push((depth, c));
                if c.group {
                    walk(p, Some(c.id), depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, None, 0, &mut out);
        for c in &self.voice_commands {
            if !out.iter().any(|(_, o)| o.id == c.id) {
                out.push((0, c));
                if c.group {
                    walk(self, Some(c.id), 1, &mut out);
                }
            }
        }
        out
    }

    /// The groups `id` is in, outermost first.
    pub fn voice_groups_of(&self, id: u32) -> Vec<&VoiceCommand> {
        let mut groups: Vec<&VoiceCommand> = Vec::new();
        let mut at = self.voice_command(id).and_then(|c| c.parent);
        while let Some(g) = at.and_then(|g| self.voice_command(g)) {
            if g.id == id || groups.iter().any(|o| o.id == g.id) {
                break;
            }
            groups.push(g);
            at = g.parent;
        }
        groups.reverse();
        groups
    }

    /// What is said for `id` in one go: the first phrase of each of its groups and its own.
    pub fn voice_full_phrase(&self, id: u32) -> String {
        self.voice_groups_of(id)
            .into_iter()
            .chain(self.voice_command(id))
            .filter_map(|c| c.phrases.first().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Moves `id` into group `parent` (`None` = the top level), at its end. False when `parent`
    /// is no group, or is `id` or inside it.
    pub fn set_voice_parent(&mut self, id: u32, parent: Option<u32>) -> bool {
        if let Some(g) = parent
            && (g == id
                || !self.voice_command(g).is_some_and(|c| c.group)
                || self.voice_descendants(id).contains(&g))
        {
            return false;
        }
        let Some(pos) = self.voice_commands.iter().position(|c| c.id == id) else {
            return false;
        };
        let mut c = self.voice_commands.remove(pos);
        c.parent = parent;
        self.voice_commands.push(c);
        true
    }

    /// Copies group `id` with everything in it (phrases and hold times, not their actions:
    /// those usually differ) into the same group. Returns the copy's id.
    pub fn duplicate_voice_group(&mut self, id: u32) -> Option<u32> {
        let group = self.voice_command(id).filter(|c| c.group)?.clone();
        let nodes: Vec<VoiceCommand> = std::iter::once(group)
            .chain(
                self.voice_descendants(id)
                    .into_iter()
                    .filter_map(|d| self.voice_command(d).cloned()),
            )
            .collect();
        let mut new_ids = std::collections::HashMap::new();
        for (i, mut node) in nodes.into_iter().enumerate() {
            let new = self.next_voice_command_id();
            new_ids.insert(node.id, new);
            node.id = new;
            if i > 0 {
                node.parent = node.parent.and_then(|p| new_ids.get(&p).copied());
            }
            self.voice_commands.push(node);
        }
        new_ids.get(&id).copied()
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
    /// not copied) in `mode`, like the card it comes from, on the base layer: a modifier held
    /// while speaking makes little sense. The command that already says it and runs nothing,
    /// else a new one. Returns its id, or `None` when another command already says `phrase`
    /// and runs other actions.
    pub fn add_voice_command_for(&mut self, action: &str, phrase: &str, mode: &str) -> Option<u32> {
        let id = self.voice_command_for_phrase(phrase).ok()?;
        if self.voice_command(id).is_none() {
            self.add_voice_command();
            self.set_voice_phrases(id, &[phrase.to_string()]);
        }
        let root = self.bind_layer(
            VOICE_GUID,
            VOICE_INPUT_TYPE,
            &id.to_string(),
            mode,
            &crate::Layer::base(),
        );
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

    /// Removes the command, or the group with everything in it, and their bindings (in every
    /// mode).
    pub fn remove_voice_command(&mut self, id: u32) {
        let mut gone = self.voice_descendants(id);
        gone.push(id);
        self.voice_commands.retain(|c| !gone.contains(&c.id));
        let gone: Vec<String> = gone.iter().map(u32::to_string).collect();
        self.inputs.retain(|b| {
            !(is_voice_binding(&b.device_id, &b.input_type)
                && gone.iter().any(|g| b.input_id.trim() == g))
        });
        self.collect_garbage();
    }

    /// What the recognizer listens for while the profile runs, with the id of the command or
    /// group each phrase sets off: every command and group with its groups' phrases before its
    /// own.
    pub fn voice_phrases(&self) -> Vec<(u32, String)> {
        self.voice_phrases_in(None)
    }

    /// What is listened for while group `scope` is open (`None` = none is): everything in it,
    /// said from there on ("wing a attack" in "radio").
    pub fn voice_phrases_in(&self, scope: Option<u32>) -> Vec<(u32, String)> {
        let tree = self.voice_tree();
        let (start, base) = match scope {
            None => (0, 0),
            Some(g) => match tree.iter().position(|(_, c)| c.id == g) {
                Some(i) => (i + 1, tree[i].0 + 1),
                None => return Vec::new(),
            },
        };
        let mut out = Vec::new();
        // The phrases said so far for each depth below `scope`: every combination of them.
        let mut said: Vec<Vec<String>> = Vec::new();
        for &(depth, c) in tree[start..].iter().take_while(|(d, _)| *d >= base) {
            let level = depth - base;
            said.truncate(level);
            let before: Vec<String> = match said.last() {
                Some(prefixes) => prefixes.clone(),
                None if level == 0 => vec![String::new()],
                // Its group has no phrases: it cannot be said.
                None => Vec::new(),
            };
            let mut mine = Vec::new();
            for prefix in &before {
                for phrase in &c.phrases {
                    let full = if prefix.is_empty() {
                        phrase.clone()
                    } else {
                        format!("{prefix} {phrase}")
                    };
                    out.push((c.id, full.clone()));
                    mine.push(full);
                }
            }
            if c.group {
                said.push(mine);
            }
        }
        out
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
