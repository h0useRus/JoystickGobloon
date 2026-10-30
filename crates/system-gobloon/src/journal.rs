//! Log lines for the systemd journal: stderr with the syslog priority prefix (`<3>` error ...
//! `<7>` debug), which journald turns into the entry's priority.

use log::{Level, LevelFilter, Log, Metadata, Record};

struct Journal;

impl Log for Journal {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("<{}>{}", priority(record.level()), record.args());
        }
    }

    fn flush(&self) {}
}

fn priority(level: Level) -> u8 {
    match level {
        Level::Error => 3,
        Level::Warn => 4,
        Level::Info => 6,
        Level::Debug | Level::Trace => 7,
    }
}

/// Sends `log` records to the journal (once; later calls do nothing).
pub fn init() {
    if log::set_logger(&Journal).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}
