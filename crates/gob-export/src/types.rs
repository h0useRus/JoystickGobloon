//! What plugin hooks take and return. Every hook gets one table (a context) and returns plain
//! tables, converted with serde.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::catalog::Catalog;

/// Where to look for the game.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Env {
    /// "windows" or "linux".
    pub platform: String,
    /// Folders to search: drives on Windows, Wine/Proton prefixes on Linux.
    pub roots: Vec<String>,
}

/// One install or channel of the game.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Target {
    pub id: String,
    pub name: String,
    /// The game's files the plugin reads and writes, by its own keys.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// The key in `files` of the file that tells the game version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_file: Option<String>,
}

/// A game file as the host read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameFile {
    pub path: String,
    /// `None` when the file does not exist.
    pub text: Option<String>,
}

/// The target's files by key.
pub type Files = BTreeMap<String, GameFile>;

/// One of our virtual devices, as the plugin may find it in the game's files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OurDevice {
    /// The output id in the profile, like "vjoy:1".
    pub output: String,
    /// The device name games see.
    pub name: String,
    /// The product GUID games see, "{XXXXXXXX-...}".
    pub guid: String,
}

/// How the game numbers one of our devices ("vjoy:1" -> "js1").
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DeviceLink {
    pub output: String,
    pub game_device: String,
}

/// A game action driven by a profile output.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Bind {
    /// The catalog's action id.
    pub action: String,
    /// The output id, like "vjoy:1:button:5", "vjoy:2:axis:z" or "key:0x2e".
    pub output: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invert: bool,
}

/// What a plugin found in the game's files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ReadResult {
    #[serde(default)]
    pub devices: Vec<DeviceLink>,
    #[serde(default)]
    pub binds: Vec<Bind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

/// One change a write makes, for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Change {
    pub kind: ChangeKind,
    pub action: String,
    /// The game's binding before, as the game writes it.
    #[serde(default)]
    pub from: Option<String>,
    /// The binding after.
    #[serde(default)]
    pub to: Option<String>,
}

/// New contents of the game's files, by the keys of `Target::files`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct WriteResult {
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub changes: Vec<Change>,
}

#[derive(Serialize)]
pub(crate) struct TargetsContext<'a> {
    pub env: &'a Env,
}

#[derive(Serialize)]
pub(crate) struct VersionContext<'a> {
    pub target: &'a Target,
    pub files: &'a Files,
}

#[derive(Serialize)]
pub struct ReadContext<'a> {
    pub target: &'a Target,
    pub files: &'a Files,
    pub catalog: &'a Catalog,
    pub ours: &'a [OurDevice],
}

#[derive(Serialize)]
pub struct WriteContext<'a> {
    pub target: &'a Target,
    pub files: &'a Files,
    pub catalog: &'a Catalog,
    pub ours: &'a [OurDevice],
    pub devices: &'a [DeviceLink],
    pub binds: &'a [Bind],
    /// The profile's name, for names the plugin gives new files.
    pub profile: &'a str,
}
