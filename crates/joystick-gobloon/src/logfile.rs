//! Technical log `JoystickGobloon.log` next to the settings (`settings::data_dir`; processing results, warnings,
//! errors). The previous run is kept as `JoystickGobloon.old.log`. Messages are English, not translated.
//! How much is written follows the `log_level` setting ([`set_level`]).

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use log::{LevelFilter, Log, Metadata, Record};

use crate::settings::LogLevel;

/// The current [`LogLevel`] as its index.
static LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Critical as u8);

struct FileLog(Mutex<File>);

/// The most detailed level written for our crates and for libraries (eframe, winit, glow...):
/// libraries stay quieter, their details are of no use in a report. Nothing goes below debug, so
/// the trace calls of libraries never reach the logger.
fn filters(level: LogLevel) -> (LevelFilter, LevelFilter) {
    match level {
        LogLevel::All => (LevelFilter::Debug, LevelFilter::Info),
        LogLevel::Important => (LevelFilter::Info, LevelFilter::Warn),
        LogLevel::Critical => (LevelFilter::Warn, LevelFilter::Warn),
    }
}

fn level() -> LogLevel {
    match LEVEL.load(Ordering::Relaxed) {
        0 => LogLevel::All,
        1 => LogLevel::Important,
        _ => LogLevel::Critical,
    }
}

/// Writes from now on what `level` asks for.
pub fn set_level(level: LogLevel) {
    LEVEL.store(level as u8, Ordering::Relaxed);
    log::set_max_level(filters(level).0);
}

fn enabled(level: LogLevel, metadata: &Metadata) -> bool {
    let target = metadata.target();
    let ours = target.starts_with("gob_")
        || target.starts_with("joystick_gobloon")
        || target.starts_with("JoystickGobloon");
    let (own, libraries) = filters(level);
    metadata.level() <= if ours { own } else { libraries }
}

impl Log for FileLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        enabled(level(), metadata)
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
    crate::settings::data_dir().map(|dir| dir.join("JoystickGobloon.log"))
}

/// Starts logging to the file at `level`; without a writable file nothing is logged.
pub fn init(level: LogLevel) {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::rename(&path, path.with_extension("old.log"));
    let Ok(mut file) = File::create(&path) else {
        return;
    };
    // Whatever the level: which version wrote the log.
    let _ = write!(
        file,
        "{} INFO  JoystickGobloon: {} {} started, log level {level:?}\r\n",
        timestamp(),
        crate::APP_NAME,
        crate::VERSION
    );
    if log::set_boxed_logger(Box::new(FileLog(Mutex::new(file)))).is_ok() {
        set_level(level);
    }
}

/// Local time like `2026-10-16 14:03:27.512`.
fn timestamp() -> String {
    crate::platform::local_time()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn writes(level: LogLevel, target: &str, l: log::Level) -> bool {
        enabled(level, &Metadata::builder().target(target).level(l).build())
    }

    #[test]
    fn levels() {
        use log::Level::*;
        assert!(writes(LogLevel::All, "gob_voice::service", Debug));
        assert!(writes(LogLevel::All, "winit", Info));
        assert!(!writes(LogLevel::All, "winit", Debug));
        assert!(writes(LogLevel::Important, "joystick_gobloon::app", Info));
        assert!(!writes(LogLevel::Important, "gob_engine", Debug));
        assert!(!writes(LogLevel::Important, "eframe", Info));
        assert!(writes(LogLevel::Critical, "gob_engine", Warn));
        assert!(!writes(LogLevel::Critical, "gob_engine", Info));
        assert!(writes(LogLevel::Critical, "eframe", Error));
    }
}
