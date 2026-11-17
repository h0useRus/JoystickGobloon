//! The bindings of every game action as one state: what the profile holds, set against what
//! the game's files have. Both views and the "is this control taken" check read it.

use super::*;

/// How a binding of the profile stands against the game's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    /// The profile's, and the game has it too (or no game files were read).
    Same,
    /// The profile's, not in the game yet: the next export writes it.
    New,
    /// In the game on what the profile holds, not in the profile: the next export removes it.
    Removed,
    /// A keyboard or mouse binding the game keeps: the player's own or the game's default.
    Game { default: bool },
}

impl Mark {
    /// The game has it after the next export.
    pub(super) fn in_effect(self) -> bool {
        self != Self::Removed
    }

    /// The profile holds it.
    pub(super) fn profile(self) -> bool {
        matches!(self, Self::Same | Self::New)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Binding<'a> {
    pub(super) output: &'a GameOutput,
    pub(super) mark: Mark,
}

/// A binding the game's files have: (action, output, the game's own default).
pub(super) type GameBind = (String, GameOutput, bool);

/// The bindings last read from the game.
pub(super) struct InGame {
    pub(super) plugin: usize,
    pub(super) binds: Vec<GameBind>,
}

impl InGame {
    pub(super) fn new(plugin: usize, read: &ReadResult) -> Self {
        let binds = read
            .binds
            .iter()
            .filter_map(|b| Some((b.action.clone(), GameOutput::parse(&b.output)?, b.default)))
            .collect();
        Self { plugin, binds }
    }

    /// Bindings on the virtual devices: what an import takes.
    pub(super) fn on_devices(&self) -> usize {
        self.binds
            .iter()
            .filter(|(_, o, _)| o.combo().is_none())
            .count()
    }
}

/// Every action's bindings.
pub(super) struct State<'a> {
    by_action: HashMap<&'a str, Vec<Binding<'a>>>,
    /// Per control in effect: how many actions use it, and whether one of them is the profile's.
    users: HashMap<&'a GameOutput, (usize, bool)>,
}

impl<'a> State<'a> {
    /// `game`: `None` when no game files were read; every binding of the profile is then
    /// shown without a mark.
    pub(super) fn build(
        profile: Option<&'a gob_core::GameBindings>,
        game: Option<&'a [GameBind]>,
    ) -> Self {
        let mut in_game: HashMap<&str, Vec<(&GameOutput, bool)>> = HashMap::new();
        for (action, output, default) in game.into_iter().flatten() {
            in_game
                .entry(action.as_str())
                .or_default()
                .push((output, *default));
        }
        let mut by_action: HashMap<&str, Vec<Binding>> = HashMap::new();
        // Actions whose keyboard and mouse bindings the profile holds.
        let mut keyed: std::collections::HashSet<&str> = std::collections::HashSet::new();
        if let Some(p) = profile {
            keyed.extend(p.cleared.iter().map(String::as_str));
            for b in &p.binds {
                if b.output.combo().is_some() {
                    keyed.insert(b.action.as_str());
                }
                let there = in_game
                    .get(b.action.as_str())
                    .is_some_and(|v| v.iter().any(|(o, _)| *o == &b.output));
                let mark = if game.is_none() || there {
                    Mark::Same
                } else {
                    Mark::New
                };
                by_action
                    .entry(b.action.as_str())
                    .or_default()
                    .push(Binding {
                        output: &b.output,
                        mark,
                    });
            }
        }
        for (action, output, default) in game.into_iter().flatten() {
            let list = by_action.entry(action.as_str()).or_default();
            if list.iter().any(|b| b.output == output) {
                continue;
            }
            let mark = if output.combo().is_some() && !keyed.contains(action.as_str()) {
                Mark::Game { default: *default }
            } else {
                Mark::Removed
            };
            list.push(Binding { output, mark });
        }
        let mut users: HashMap<&GameOutput, (usize, bool)> = HashMap::new();
        for list in by_action.values() {
            for b in list.iter().filter(|b| b.mark.in_effect()) {
                let u = users.entry(b.output).or_default();
                u.0 += 1;
                u.1 |= b.mark.profile();
            }
        }
        Self { by_action, users }
    }

    /// Every action with bindings.
    pub(super) fn all(&self) -> impl Iterator<Item = (&'a str, &[Binding<'a>])> {
        self.by_action.iter().map(|(a, v)| (*a, v.as_slice()))
    }

    pub(super) fn of(&self, action: &str) -> &[Binding<'a>] {
        self.by_action.get(action).map_or(&[], Vec::as_slice)
    }

    /// `output` drives `action` (after the next export too).
    pub(super) fn has(&self, action: &str, output: &GameOutput) -> bool {
        self.of(action)
            .iter()
            .any(|b| b.output == output && b.mark.in_effect())
    }

    /// The other actions `output` drives, with how each holds it; sorted by id.
    pub(super) fn others(&self, output: &GameOutput, action: &str) -> Vec<(&'a str, Mark)> {
        let mut list: Vec<(&str, Mark)> = self
            .by_action
            .iter()
            .filter(|(a, _)| **a != action)
            .filter_map(|(a, v)| {
                v.iter()
                    .find(|b| b.output == output && b.mark.in_effect())
                    .map(|b| (*a, b.mark))
            })
            .collect();
        list.sort_by_key(|(a, _)| *a);
        list
    }

    /// Several actions use `output`, at least one of them through the profile.
    pub(super) fn shared(&self, output: &GameOutput) -> bool {
        self.users
            .get(output)
            .is_some_and(|(n, profile)| *n > 1 && *profile)
    }

    /// The keyboard and mouse bindings `action` has in effect.
    pub(super) fn keys(&self, action: &str) -> Vec<gob_core::Combo> {
        self.of(action)
            .iter()
            .filter(|b| b.mark.in_effect())
            .filter_map(|b| b.output.combo().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(s: &str) -> GameOutput {
        GameOutput::parse(s).unwrap()
    }

    #[test]
    fn marks_follow_the_game() {
        let mut p = Profile::default();
        p.bind_game_action("g", "gear", out("vjoy:1:button:5"));
        p.bind_game_action("g", "lights", out("vjoy:1:button:6"));
        p.bind_game_action("g", "eject", out("key:0x26"));
        let game = vec![
            ("gear".to_string(), out("vjoy:1:button:5"), false),
            ("brake".to_string(), out("vjoy:1:button:9"), false),
            // The profile holds eject's keys: the game's default goes at the next export.
            ("eject".to_string(), out("key:0x138+key:0x15"), true),
            // Nobody holds the mining keys: the game keeps them.
            ("mining".to_string(), out("key:0x32"), true),
            ("salvage".to_string(), out("key:0x32"), true),
        ];
        let s = State::build(p.game_bindings("g"), Some(&game));
        let mark = |action: &str, o: &str| {
            let o = out(o);
            s.of(action).iter().find(|b| *b.output == o).unwrap().mark
        };
        assert_eq!(mark("gear", "vjoy:1:button:5"), Mark::Same);
        assert_eq!(mark("lights", "vjoy:1:button:6"), Mark::New);
        assert_eq!(mark("brake", "vjoy:1:button:9"), Mark::Removed);
        assert_eq!(mark("eject", "key:0x26"), Mark::New);
        assert_eq!(mark("eject", "key:0x138+key:0x15"), Mark::Removed);
        assert_eq!(mark("mining", "key:0x32"), Mark::Game { default: true });
        // Where a control is used: only what stays in effect, the asking action left out.
        assert_eq!(
            s.others(&out("key:0x32"), "mining"),
            [("salvage", Mark::Game { default: true })]
        );
        assert!(s.others(&out("vjoy:1:button:9"), "gear").is_empty());
        // Shared controls count only with the profile in them.
        assert!(!s.shared(&out("key:0x32")));
        assert_eq!(
            s.keys("eject"),
            [gob_core::Combo::parse("key:0x26").unwrap()]
        );
        // Without game files nothing is marked.
        let alone = State::build(p.game_bindings("g"), None);
        assert_eq!(alone.of("lights")[0].mark, Mark::Same);
    }
}
