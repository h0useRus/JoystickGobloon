//! Platforms without their own module yet (Linux: XDG paths, a socket for the single instance,
//! fontconfig, docs/LINUX_PLAN.md phase 5).

use std::path::{Path, PathBuf};

use gob_core::Key;

pub(crate) struct Desktop;

impl super::Desktop for Desktop {
    type InstanceEvent = ();

    fn default_data_dir() -> Option<PathBuf> {
        None
    }

    fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
        Vec::new()
    }

    fn open_text_file(_path: &Path) {}

    fn program_extensions() -> &'static [&'static str] {
        &[]
    }

    fn local_time() -> String {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}.{:03}", d.as_secs(), d.subsec_millis())
    }

    fn claim_instance() -> Option<()> {
        Some(())
    }

    fn listen_instance(_event: (), _show: Box<dyn Fn() + Send>) {}

    fn held_keys() -> Vec<Key> {
        Vec::new()
    }

    fn key_name(key: Key) -> String {
        super::key_code(key)
    }
}
