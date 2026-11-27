//! A game's actions per game version (`catalog/<version>.gob`) and the actions renamed between
//! versions (`renames.gob`). Both are TOML text.

use std::cmp::Ordering;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::version;

/// What kind of input a game action takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    #[default]
    Button,
    Axis,
    Hat,
}

/// One action of the game.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GameAction {
    /// The plugin's id of the action, unique in the catalog.
    pub id: String,
    /// The group the game shows it in.
    #[serde(default)]
    pub category: String,
    /// The name to show.
    pub name: String,
    #[serde(default)]
    pub kind: ActionKind,
    /// The game's own default bindings, as the plugin writes them (a key like "kb1_g").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default: Vec<String>,
    /// The game's setting that inverts this axis, as the plugin writes it (Star Citizen:
    /// the option group, "flight_move_pitch"); none = the game inverts it no other way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<String>,
}

/// The actions of one game version.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Catalog {
    /// The game version, from the file name.
    pub version: String,
    pub actions: Vec<GameAction>,
}

#[derive(Deserialize)]
struct CatalogFile {
    #[serde(default)]
    action: Vec<GameAction>,
}

impl Catalog {
    pub fn parse(version: &str, text: &str) -> Result<Self, String> {
        let file: CatalogFile = toml::from_str(text).map_err(|e| e.message().to_string())?;
        Ok(Self {
            version: version.to_string(),
            actions: file.action,
        })
    }

    pub fn action(&self, id: &str) -> Option<&GameAction> {
        self.actions.iter().find(|a| a.id == id)
    }
}

/// How the picked catalog fits the game's version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// A catalog of exactly this version.
    Exact,
    /// The game is newer than the catalog: actions added since are missing.
    GameNewer,
    /// The game is older than every catalog: some actions may not exist in it yet.
    GameOlder,
    /// The game's version is not known; the latest catalog is used.
    Unknown,
}

/// Every catalog of a plugin, oldest first.
#[derive(Debug, Clone, Default)]
pub struct Catalogs(Vec<Catalog>);

impl Catalogs {
    pub fn new(mut list: Vec<Catalog>) -> Self {
        list.sort_by(|a, b| version::compare(&a.version, &b.version));
        Self(list)
    }

    /// Reads `catalog/*.gob` in a plugin's folder; a missing folder is no catalogs.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let mut list = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "gob") {
                continue;
            }
            let Some(version) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            list.push(
                Catalog::parse(version, &text)
                    .map_err(|e| format!("catalog/{version}.gob: {e}"))?,
            );
        }
        Ok(Self::new(list))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn versions(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|c| c.version.as_str())
    }

    /// The catalog for a game version: the same version, else the newest older one, else the
    /// oldest. `None` only when there are no catalogs.
    pub fn pick(&self, game: Option<&str>) -> Option<(&Catalog, Fit)> {
        let latest = self.0.last()?;
        let Some(game) = game else {
            return Some((latest, Fit::Unknown));
        };
        let mut best = None;
        for c in &self.0 {
            match version::compare(&c.version, game) {
                Ordering::Equal => return Some((c, Fit::Exact)),
                Ordering::Less => best = Some(c),
                Ordering::Greater => break,
            }
        }
        Some(match best {
            Some(c) => (c, Fit::GameNewer),
            None => (&self.0[0], Fit::GameOlder),
        })
    }
}

/// An action id changed by the game in a version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Rename {
    pub from: String,
    pub to: String,
    /// The first game version with the new id.
    pub since: String,
}

/// The renames of a plugin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Renames(Vec<Rename>);

#[derive(Deserialize)]
struct RenamesFile {
    #[serde(default)]
    rename: Vec<Rename>,
}

/// Renames followed at most for one id, so a loop in the file cannot hang.
const MAX_HOPS: usize = 32;

impl Renames {
    pub fn new(list: Vec<Rename>) -> Self {
        Self(list)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let file: RenamesFile = toml::from_str(text).map_err(|e| e.message().to_string())?;
        Ok(Self(file.rename))
    }

    /// The id an action has in `version` (every rename up to that version applied); with no
    /// version, every rename is applied.
    pub fn resolve<'a>(&'a self, id: &'a str, version: Option<&str>) -> &'a str {
        let mut id = id;
        for _ in 0..MAX_HOPS {
            let next = self.0.iter().find(|r| {
                r.from == id
                    && version.is_none_or(|v| version::compare(&r.since, v) != Ordering::Greater)
            });
            match next {
                Some(r) => id = &r.to,
                None => break,
            }
        }
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(v: &str) -> Catalog {
        Catalog {
            version: v.into(),
            actions: Vec::new(),
        }
    }

    #[test]
    fn parses_actions() {
        let c = Catalog::parse(
            "4.10.197",
            r#"
                [[action]]
                id = "seat_general/v_eject"
                category = "Seat"
                name = "Eject"
                default = ["kb1_ralt+l"]

                [[action]]
                id = "flight/v_roll"
                name = "Roll"
                kind = "axis"
            "#,
        )
        .unwrap();
        assert_eq!(c.actions.len(), 2);
        assert_eq!(c.action("flight/v_roll").unwrap().kind, ActionKind::Axis);
        assert_eq!(c.actions[0].kind, ActionKind::Button);
        assert_eq!(c.actions[0].default, ["kb1_ralt+l"]);
    }

    #[test]
    fn picks_the_nearest_catalog() {
        let set = Catalogs::new(vec![
            catalog("4.10.200"),
            catalog("4.9"),
            catalog("4.10.197"),
        ]);
        let pick = |v| set.pick(v).map(|(c, f)| (c.version.as_str(), f));
        assert_eq!(pick(Some("4.10.197")), Some(("4.10.197", Fit::Exact)));
        assert_eq!(pick(Some("4.10.199.5")), Some(("4.10.197", Fit::GameNewer)));
        assert_eq!(pick(Some("4.11")), Some(("4.10.200", Fit::GameNewer)));
        assert_eq!(pick(Some("4.8")), Some(("4.9", Fit::GameOlder)));
        assert_eq!(pick(None), Some(("4.10.200", Fit::Unknown)));
        assert!(Catalogs::default().pick(Some("1")).is_none());
    }

    #[test]
    fn renames_follow_versions() {
        let r = Renames::parse(
            r#"
                [[rename]]
                from = "a"
                to = "b"
                since = "4.10"

                [[rename]]
                from = "b"
                to = "c"
                since = "4.11"

                [[rename]]
                from = "x"
                to = "x"
                since = "1"
            "#,
        )
        .unwrap();
        assert_eq!(r.resolve("a", Some("4.9")), "a");
        assert_eq!(r.resolve("a", Some("4.10.197")), "b");
        assert_eq!(r.resolve("a", Some("4.11")), "c");
        assert_eq!(r.resolve("a", None), "c");
        assert_eq!(r.resolve("x", None), "x");
    }
}
