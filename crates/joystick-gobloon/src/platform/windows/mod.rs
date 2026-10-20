//! Windows desktop integration (Win32).

mod instance;
mod keys;

use std::path::{Path, PathBuf};

use gob_core::Key;

pub(crate) struct Desktop;

impl super::Desktop for Desktop {
    type InstanceEvent = instance::Event;

    fn default_data_dir() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("JoystickGobloon"))
    }

    /// Fonts every Windows 10/11 has: Microsoft YaHei, Malgun Gothic.
    fn cjk_fonts() -> Vec<(&'static str, PathBuf)> {
        let windows = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
        let dir = Path::new(&windows).join("Fonts");
        vec![
            ("cjk-zh", dir.join("msyh.ttc")),
            ("cjk-ko", dir.join("malgun.ttf")),
        ]
    }

    fn open_text_file(path: &Path) {
        let _ = std::process::Command::new("notepad.exe").arg(path).spawn();
    }

    fn program_extensions() -> &'static [&'static str] {
        &["exe"]
    }

    fn local_time() -> String {
        // SAFETY: GetLocalTime only writes the returned struct.
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
        )
    }

    fn claim_instance() -> Option<instance::Event> {
        instance::claim()
    }

    fn listen_instance(event: instance::Event, show: Box<dyn Fn() + Send>) {
        instance::listen(event, show);
    }

    fn held_keys() -> Vec<Key> {
        keys::held()
    }

    fn key_name(key: Key) -> String {
        keys::name(key)
    }
}
