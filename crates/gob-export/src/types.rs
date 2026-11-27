//! What plugin hooks take and return. Every hook gets one table (a context) and returns plain
//! tables, converted with serde.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::catalog::{Catalog, GameAction};

/// Where to look for the game.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Env {
    /// "windows" or "linux".
    pub platform: String,
    /// Folders to search: drives on Windows, Wine/Proton prefixes on Linux.
    pub roots: Vec<String>,
    /// The user's Documents folder, with `/` and a `/` at its end (wherever the user moved it);
    /// empty when not known.
    pub documents: String,
    /// The user's local application data folder (`%LOCALAPPDATA%` on Windows), written like
    /// `documents`; empty when not known (on Linux games keep theirs in the Wine prefixes).
    pub local_app_data: String,
}

/// One install or channel of the game.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Target {
    pub id: String,
    pub name: String,
    /// The game's files the plugin reads and writes, by its own keys.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// Folders the plugin may create new files in (`WriteResult::create`), by its own keys.
    #[serde(default)]
    pub dirs: BTreeMap<String, String>,
    /// Files whose names the game makes up, by a key and a pattern where `*` stands for any
    /// part of one folder or file name ("C:/Steam/userdata/*/remote/profile_*"). The host
    /// finds them when the plugin returns its targets and adds each to `files` as
    /// "<key>:<path>"; the plugin then reads and writes them like its other files.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub globs: BTreeMap<String, String>,
    /// Text the files of a glob (by the same key) must have near their start, in the first
    /// [`crate::host::GLOB_PEEK`] bytes; others are left out without being read whole.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub glob_text: BTreeMap<String, String>,
    /// The key in `files` of the file that tells the game version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_file: Option<String>,
    /// Folders the target stands for one by one, as a pattern like a glob's
    /// ("C:/Users/x/Saved Games/*/Config/Input/*"): the host makes a copy of the target for
    /// every folder found, with `{1}`, `{2}` ... in its id, name, files, dirs, globs and
    /// glob_text replaced by the folder names the first, second ... `*` matched. A target
    /// without it stands for itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub each: Option<String>,
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OurDevice {
    /// The output id in the profile, like "vjoy:1".
    pub output: String,
    /// The device name games see.
    pub name: String,
    /// The product GUID games see, "{XXXXXXXX-...}".
    pub guid: String,
    /// The DirectInput instance GUID of the device on this computer, "{XXXXXXXX-...}"; empty
    /// when not known (the device is not attached, or the system has no such GUIDs).
    pub instance: String,
    /// Its axes in DirectInput order, by our names ("x", "y", "z", "rx", "ry", "rz", "slider",
    /// "dial"): for games that number a device's controls one after another.
    pub axes: Vec<String>,
    pub buttons: u32,
    pub hats: u32,
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
    /// The output id, like "vjoy:1:button:5", "vjoy:2:axis:z", "key:0x2e", "mouse:left" or
    /// keys and mouse buttons pressed together, "key:0x138+key:0x15".
    pub output: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invert: bool,
    /// Read from the game: its own default binding, not one the player set.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
}

/// What a plugin found in the game's files.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ReadResult {
    #[serde(default)]
    pub devices: Vec<DeviceLink>,
    #[serde(default)]
    pub binds: Vec<Bind>,
    /// Actions the game's own files list, for games whose actions differ per install (add-ons,
    /// aircraft); the program shows them with the catalog's.
    #[serde(default)]
    pub actions: Vec<GameAction>,
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

/// A file a write creates (or replaces) in one of the target's `dirs`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NewFile {
    /// The key in `Target::dirs`.
    pub dir: String,
    /// A file name, without folders.
    pub name: String,
    pub text: String,
}

/// New contents of the game's files, by the keys of `Target::files`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct WriteResult {
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub create: Vec<NewFile>,
    #[serde(default)]
    pub changes: Vec<Change>,
}

/// A value of a plugin option.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OptionValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionKind {
    Bool,
    Number,
    Text,
}

/// An option a plugin declares with `plugin.options`; the program shows it as a form.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct OptionDecl {
    #[serde(default)]
    pub key: String,
    pub kind: OptionKind,
    pub label: String,
    pub default: OptionValue,
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
    /// The device links the profile keeps (typed by the user or read before); a plugin that
    /// finds links itself lets these win.
    pub devices: &'a [DeviceLink],
}

#[derive(Serialize)]
pub struct WriteContext<'a> {
    pub target: &'a Target,
    pub files: &'a Files,
    pub catalog: &'a Catalog,
    pub ours: &'a [OurDevice],
    pub devices: &'a [DeviceLink],
    /// Bindings of the virtual devices, and the keyboard and mouse bindings of the actions the
    /// profile holds them for; the other actions keep the game's keyboard and mouse bindings.
    pub binds: &'a [Bind],
    /// Actions the profile leaves without any keyboard or mouse binding.
    pub cleared: &'a [String],
    /// The profile's name, for names the plugin gives new files.
    pub profile: &'a str,
    /// Values of the plugin's options, by key; missing ones take their default.
    pub options: &'a BTreeMap<String, OptionValue>,
}
