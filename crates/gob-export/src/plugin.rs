//! One plugin: its manifest, catalogs, renames and the Luau state running `plugin.luau`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gob_script::{ScriptError, script_error};
use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaSerdeExt, Table, Value, VmState};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::catalog::{Catalogs, Renames};
use crate::types::{
    Env, Files, ReadContext, ReadResult, Target, TargetsContext, VersionContext, WriteContext,
    WriteResult,
};
use crate::version::Range;

const PRELUDE: &str = include_str!("prelude.luau");
/// The plugin API version this host offers; a plugin asking for a newer one is not loaded.
pub const API: u32 = 1;
/// Longest a hook may run: export reads and writes whole binding files.
pub const CALL_BUDGET: Duration = Duration::from_secs(2);
/// Longest `plugin.luau`'s top level may run when it is loaded.
pub const LOAD_BUDGET: Duration = Duration::from_secs(1);

pub const MANIFEST: &str = "plugin.gob";
pub const CODE: &str = "plugin.luau";
pub const CATALOG_DIR: &str = "catalog";
pub const RENAMES: &str = "renames.gob";

/// A plugin's error, with the line of `plugin.luau` when there is one.
pub type PluginError = ScriptError;

/// `plugin.gob`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Manifest {
    /// Unique among plugins; profiles keep their bindings under it.
    pub id: String,
    /// The game's name as shown.
    pub name: String,
    /// The plugin's own version.
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// The plugin API it needs.
    #[serde(default = "first_api")]
    pub api: u32,
    /// An image next to `plugin.gob`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The game versions it can write.
    #[serde(default)]
    pub game: Range,
}

fn first_api() -> u32 {
    1
}

/// The hooks a plugin may register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    Targets,
    Version,
    Read,
    Write,
}

impl Hook {
    fn name(self) -> &'static str {
        match self {
            Self::Targets => "targets",
            Self::Version => "version",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

pub struct Plugin {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub catalogs: Catalogs,
    pub renames: Renames,
    lua: Lua,
    hooks: Table,
    /// Nanoseconds after `clock` at which the running call is stopped.
    deadline: Arc<AtomicU64>,
    clock: Instant,
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugin")
            .field("dir", &self.dir)
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}

fn error(plugin: &str, message: impl Into<String>) -> PluginError {
    PluginError {
        script: plugin.to_string(),
        line: None,
        message: message.into(),
    }
}

impl Plugin {
    /// Loads the plugin in `dir`: `plugin.gob`, `catalog/*.gob`, `renames.gob` and `plugin.luau`.
    pub fn load(dir: &Path) -> Result<Self, PluginError> {
        let folder = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name))
                .map_err(|e| error(&folder, format!("{name}: {e}")))
        };
        let manifest: Manifest = toml::from_str(&read(MANIFEST)?)
            .map_err(|e| error(&folder, format!("{MANIFEST}: {}", e.message())))?;
        let id = manifest.id.clone();
        if manifest.api > API {
            return Err(error(
                &id,
                format!(
                    "needs plugin API {}, this version of the program has {API}",
                    manifest.api
                ),
            ));
        }
        let catalogs = Catalogs::load(&dir.join(CATALOG_DIR)).map_err(|e| error(&id, e))?;
        let renames = match std::fs::read_to_string(dir.join(RENAMES)) {
            Ok(text) => Renames::parse(&text).map_err(|e| error(&id, format!("{RENAMES}: {e}")))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Renames::default(),
            Err(e) => return Err(error(&id, format!("{RENAMES}: {e}"))),
        };
        let code = read(CODE).map_err(|e| PluginError {
            script: id.clone(),
            ..e
        })?;
        let mut plugin =
            Self::with_code(&id, &code, manifest).map_err(|e| PluginError { script: id, ..e })?;
        plugin.dir = dir.to_path_buf();
        plugin.catalogs = catalogs;
        plugin.renames = renames;
        Ok(plugin)
    }

    /// A plugin from its code alone (no folder, catalogs or renames), for tests and checks.
    pub fn with_code(name: &str, code: &str, manifest: Manifest) -> Result<Self, PluginError> {
        let lua_error = |e: mlua::Error| PluginError {
            script: name.to_string(),
            ..script_error(CODE, &e)
        };
        let lua = Lua::new();
        let clock = Instant::now();
        let deadline = Arc::new(AtomicU64::new(u64::MAX));
        {
            let deadline = deadline.clone();
            lua.set_interrupt(move |_| {
                if clock.elapsed().as_nanos() as u64 > deadline.load(Ordering::Relaxed) {
                    Err(mlua::Error::runtime("ran too long and was stopped"))
                } else {
                    Ok(VmState::Continue)
                }
            });
        }
        let host = lua.create_table().map_err(lua_error)?;
        {
            let name = name.to_string();
            let log = lua
                .create_function(move |_, text: String| {
                    log::info!("export plugin {name}: {text}");
                    Ok(())
                })
                .map_err(lua_error)?;
            host.set("log", log).map_err(lua_error)?;
        }
        lua.load(PRELUDE)
            .set_name("=prelude")
            .call::<()>(host.clone())
            .map_err(lua_error)?;
        lua.sandbox(true).map_err(lua_error)?;
        let hooks: Table = host.get("hooks").map_err(lua_error)?;
        let plugin = Self {
            dir: PathBuf::new(),
            manifest,
            catalogs: Catalogs::default(),
            renames: Renames::default(),
            lua,
            hooks,
            deadline,
            clock,
        };
        plugin.arm(LOAD_BUDGET);
        let result = plugin.lua.load(code).set_name(format!("={CODE}")).exec();
        plugin.disarm();
        result.map_err(lua_error)?;
        Ok(plugin)
    }

    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    pub fn has(&self, hook: Hook) -> bool {
        matches!(
            self.hooks.raw_get::<Value>(hook.name()),
            Ok(Value::Function(_))
        )
    }

    fn arm(&self, budget: Duration) {
        let at = (self.clock.elapsed() + budget).as_nanos() as u64;
        self.deadline.store(at, Ordering::Relaxed);
    }

    fn disarm(&self) {
        self.deadline.store(u64::MAX, Ordering::Relaxed);
    }

    fn lua_error(&self, e: &mlua::Error) -> PluginError {
        PluginError {
            script: self.id().to_string(),
            ..script_error(CODE, e)
        }
    }

    /// Calls a hook with one context table; a missing hook or a `nil` result gives `R::default()`.
    fn call<C: Serialize, R: DeserializeOwned + Default>(
        &self,
        hook: Hook,
        ctx: &C,
    ) -> Result<R, PluginError> {
        let f: Option<Function> = self
            .hooks
            .raw_get(hook.name())
            .map_err(|e| self.lua_error(&e))?;
        let Some(f) = f else {
            return Ok(R::default());
        };
        let options = SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false);
        let arg = self
            .lua
            .to_value_with(ctx, options)
            .map_err(|e| self.lua_error(&e))?;
        self.arm(CALL_BUDGET);
        let result = f.call::<Value>(arg);
        self.disarm();
        let value = result.map_err(|e| self.lua_error(&e))?;
        if value.is_nil() {
            return Ok(R::default());
        }
        self.lua.from_value(value).map_err(|e| {
            error(
                self.id(),
                format!("plugin.{} returned something unexpected: {e}", hook.name()),
            )
        })
    }

    /// The game's installs or channels the plugin finds.
    pub fn targets(&self, env: &Env) -> Result<Vec<Target>, PluginError> {
        self.call(Hook::Targets, &TargetsContext { env })
    }

    /// The game version of a target, read from its files.
    pub fn version(&self, target: &Target, files: &Files) -> Result<Option<String>, PluginError> {
        self.call(Hook::Version, &VersionContext { target, files })
    }

    /// The bindings already in the game's files.
    pub fn read(&self, ctx: &ReadContext) -> Result<ReadResult, PluginError> {
        self.call(Hook::Read, ctx)
    }

    /// The game's files with the profile's bindings, and what changes.
    pub fn write(&self, ctx: &WriteContext) -> Result<WriteResult, PluginError> {
        self.call(Hook::Write, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        toml::from_str("id = \"test\"\nname = \"Test\"").unwrap()
    }

    #[test]
    fn errors_name_their_line() {
        let e = Plugin::with_code("test", "local x = 1\nerror('broken')", manifest()).unwrap_err();
        assert_eq!(e.script, "test");
        assert_eq!(e.line, Some(2));
        assert_eq!(e.message, "broken");
    }

    #[test]
    fn a_hook_that_runs_forever_is_stopped() {
        let p = Plugin::with_code(
            "test",
            "plugin.targets(function() while true do end end)",
            manifest(),
        )
        .unwrap();
        let start = Instant::now();
        let e = p.targets(&Env::default()).unwrap_err();
        assert!(e.message.contains("ran too long"), "{}", e.message);
        assert!(start.elapsed() < CALL_BUDGET * 3);
    }

    #[test]
    fn missing_hooks_give_nothing() {
        let p = Plugin::with_code("test", "", manifest()).unwrap();
        assert!(!p.has(Hook::Targets));
        assert!(p.targets(&Env::default()).unwrap().is_empty());
    }

    #[test]
    fn plugins_cannot_reach_files_or_programs() {
        for code in [
            "io.open('x')",
            "os.execute('x')",
            "require('x')",
            "loadfile('x')",
        ] {
            assert!(
                Plugin::with_code("test", code, manifest()).is_err(),
                "{code}"
            );
        }
        // Hooks cannot be replaced from outside the prelude's API.
        assert!(Plugin::with_code("test", "plugin.targets(1)", manifest()).is_err());
    }
}
