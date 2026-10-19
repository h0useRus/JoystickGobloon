//! Smallest plugin DLL: only tells Joystick Gobloon its name and version.
//! Build: `cargo build -p gob-plugin --example stub_plugin`
//! (target\debug\examples\stub_plugin.dll), then copy it to `<Joystick Gobloon folder>\plugins`.

use gob_plugin::{ABI_VERSION, PluginInfo};

struct Info(PluginInfo);

// SAFETY: the info only points to static strings.
unsafe impl Sync for Info {}

static INFO: Info = Info(PluginInfo {
    abi_version: ABI_VERSION,
    name: c"Stub plugin".as_ptr(),
    version: c"0.1.0".as_ptr(),
});

#[unsafe(no_mangle)]
pub extern "C" fn joystick_gobloon_plugin_info() -> *const PluginInfo {
    &INFO.0
}
