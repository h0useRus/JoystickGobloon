//! Technical log `joygob.log` next to the settings (`settings::data_dir`; processing results, warnings,
//! errors). The previous run is kept as `joygob.old.log`. Messages are English, not translated.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

struct FileLog(Mutex<File>);

impl Log for FileLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // Our crates in detail, libraries (eframe, winit, wgpu...) only when something is wrong.
        let target = metadata.target();
        let ours = target.starts_with("gob_") || target.starts_with("JoystickGobloon");
        metadata.level() <= if ours { Level::Debug } else { Level::Warn }
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {}: {}\r\n",
            timestamp(),
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut f) = self.0.lock() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.0.lock() {
            let _ = f.flush();
        }
    }
}

pub fn path() -> Option<PathBuf> {
    crate::settings::data_dir().map(|dir| dir.join("joygob.log"))
}

/// Starts logging to the file; without a writable file nothing is logged.
pub fn init() {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::rename(&path, path.with_extension("old.log"));
    let Ok(file) = File::create(&path) else {
        return;
    };
    if log::set_boxed_logger(Box::new(FileLog(Mutex::new(file)))).is_ok() {
        log::set_max_level(LevelFilter::Debug);
    }
    log::info!("{} {} started", crate::APP_NAME, crate::VERSION);
}

/// Local time like `2026-10-16 14:03:27.512`.
fn timestamp() -> String {
    #[cfg(windows)]
    {
        // SAFETY: GetLocalTime only writes the returned struct.
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
        )
    }
    #[cfg(not(windows))]
    {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}.{:03}", d.as_secs(), d.subsec_millis())
    }
}
