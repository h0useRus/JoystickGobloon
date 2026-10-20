//! Extension point for action types Joystick Gobloon does not implement itself.
//!
//! An [`ActionPlugin`] handles one Gremlin action `type`: for every such action in a
//! profile it creates a [`Functor`], which receives the input values and answers with
//! [`Command`]s that the engine executes. Plugins are registered in-process with
//! [`Registry::register`].
//!
//! DLL plugins are a stub for now: [`Registry::load_dir`] loads `*.dll` files that export
//! [`INFO_SYMBOL`] (a C function returning [`PluginInfo`]), checks [`ABI_VERSION`] and
//! keeps them loaded, but they cannot provide actions yet.

use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};

use gob_core::Action;

/// Version of the DLL interface; a plugin built for another version is rejected.
pub const ABI_VERSION: u32 = 1;

/// Exported by plugin DLLs: `extern "C" fn joystick_gobloon_plugin_info() -> *const PluginInfo`.
pub const INFO_SYMBOL: &[u8] = b"joystick_gobloon_plugin_info\0";

/// What a plugin DLL tells about itself. Strings are NUL-terminated UTF-8 owned by the DLL.
#[repr(C)]
pub struct PluginInfo {
    pub abi_version: u32,
    pub name: *const c_char,
    pub version: *const c_char,
}

/// The value an action receives, like the engine's own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Button(bool),
    /// -1.0..=1.0
    Axis(f32),
    /// Hundredths of a degree, `None` when centered.
    Hat(Option<u16>),
}

/// Output a functor asks the engine to perform.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    VjoyAxis {
        device: u32,
        axis: u8,
        value: f32,
    },
    VjoyButton {
        device: u32,
        button: u8,
        pressed: bool,
    },
    VjoyHat {
        device: u32,
        hat: u8,
        direction: Option<u16>,
    },
    /// Set 1 scan code; `extended` adds the E0 prefix.
    Key {
        scan_code: u16,
        extended: bool,
        pressed: bool,
    },
    /// A line for the log file.
    Log(String),
}

/// Runtime state of one action instance.
pub trait Functor: Send {
    /// Called for every event the action receives; push the outputs to `out`.
    fn process(&mut self, value: Value, out: &mut Vec<Command>);
}

/// Handles one action type (the `type` attribute of `<action>` in Gremlin XML).
pub trait ActionPlugin: Send + Sync {
    fn kind(&self) -> &str;
    /// Human-readable name for the UI.
    fn name(&self) -> &str {
        self.kind()
    }
    /// Creates the functor for one action of this type (properties via [`Action::str`]).
    fn create(&self, action: &Action) -> Box<dyn Functor>;
}

/// A loaded plugin DLL.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin {
    pub path: PathBuf,
    pub name: String,
    pub version: String,
}

#[derive(Default)]
pub struct Registry {
    actions: Vec<Box<dyn ActionPlugin>>,
    loaded: Vec<LoadedPlugin>,
    /// Kept loaded for as long as the registry lives.
    libraries: Vec<libloading::Library>,
}

impl Registry {
    /// Adds an in-process plugin; a later one for the same kind replaces the earlier.
    pub fn register(&mut self, plugin: Box<dyn ActionPlugin>) {
        self.actions.retain(|p| p.kind() != plugin.kind());
        self.actions.push(plugin);
    }

    pub fn get(&self, kind: &str) -> Option<&dyn ActionPlugin> {
        self.actions
            .iter()
            .find(|p| p.kind() == kind)
            .map(|p| p.as_ref())
    }

    pub fn loaded(&self) -> &[LoadedPlugin] {
        &self.loaded
    }

    /// Loads every `*.dll` in `dir`. A missing folder is not an error; each failing DLL
    /// gives one error message and the others still load.
    pub fn load_dir(&mut self, dir: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut errors = Vec::new();
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dll")))
            .collect();
        paths.sort();
        for path in paths {
            if let Err(e) = self.load(&path) {
                errors.push(format!("{}: {e}", path.display()));
            }
        }
        errors
    }

    fn load(&mut self, path: &Path) -> Result<(), String> {
        // SAFETY: loading a DLL runs its initialisation code; plugins are trusted by being
        // placed in the plugins folder. The info function and its strings are only used
        // while the library stays loaded (it is kept in `self.libraries`).
        unsafe {
            let lib = libloading::Library::new(path).map_err(|e| e.to_string())?;
            let info_fn: libloading::Symbol<unsafe extern "C" fn() -> *const PluginInfo> = lib
                .get(INFO_SYMBOL)
                .map_err(|_| "not a Joystick Gobloon plugin".to_string())?;
            let info = info_fn();
            if info.is_null() {
                return Err("no plugin information".into());
            }
            let info = &*info;
            if info.abi_version != ABI_VERSION {
                return Err(format!(
                    "built for plugin interface {}, Joystick Gobloon has {ABI_VERSION}",
                    info.abi_version
                ));
            }
            let text = |p: *const c_char| {
                if p.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(p).to_string_lossy().into_owned()
                }
            };
            self.loaded.push(LoadedPlugin {
                path: path.to_path_buf(),
                name: text(info.name),
                version: text(info.version),
            });
            self.libraries.push(lib);
        }
        Ok(())
    }
}
