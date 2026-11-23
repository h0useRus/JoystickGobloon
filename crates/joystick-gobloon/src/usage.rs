//! The memory and CPU the program uses, for the status bar and the overlay: read on demand, at
//! most every [`EVERY`] (cheap system calls, no sampling thread). The CPU share is the CPU time
//! between two readings.

use std::time::{Duration, Instant};

use crate::platform::Usage;

/// Time between readings. Showing a new one redraws a window, which costs far more than
/// reading it: rare enough to be negligible, often enough to follow a change.
pub const EVERY: Duration = Duration::from_secs(5);

/// One reading, as shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub mb: u64,
    /// Percent of all cores; `None` until two readings were taken.
    pub cpu: Option<f64>,
}

impl Reading {
    /// The CPU share as shown, "-" before it is known.
    pub fn cpu_text(&self) -> String {
        self.cpu
            .map_or_else(|| "-".to_string(), |c| format!("{c:.1}"))
    }

    /// `122 MB · 0.4 % CPU`.
    pub fn text(&self) -> String {
        format!("{} MB  \u{b7}  {} % CPU", self.mb, self.cpu_text())
    }
}

#[derive(Default)]
pub struct UsageMeter {
    last: Option<(Instant, Option<Usage>)>,
    cpu: Option<f64>,
}

impl UsageMeter {
    /// The latest reading, read anew when the last one is older than [`EVERY`]; `None` where
    /// the system does not tell.
    pub fn read(&mut self) -> Option<Reading> {
        let now = Instant::now();
        if self.last.is_none_or(|(at, _)| now - at >= EVERY) {
            let fresh = crate::platform::usage();
            if let (Some((at, Some(before))), Some(after)) = (self.last, fresh) {
                let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
                let used = after.cpu_time.saturating_sub(before.cpu_time);
                let seconds = (now - at).as_secs_f64().max(0.001);
                self.cpu = Some(used.as_secs_f64() / seconds / cores as f64 * 100.0);
            }
            self.last = Some((now, fresh));
        }
        let usage = self.last?.1?;
        Some(Reading {
            mb: usage.memory.div_ceil(1_048_576),
            cpu: self.cpu,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_as_one_line() {
        let r = Reading {
            mb: 122,
            cpu: Some(0.44),
        };
        assert_eq!(r.text(), "122 MB  \u{b7}  0.4 % CPU");
        assert_eq!(Reading { cpu: None, ..r }.cpu_text(), "-");
    }

    #[test]
    fn reads_once_per_interval() {
        let mut meter = UsageMeter::default();
        let first = meter.read();
        // Where the system tells: memory at once, the CPU share from the second reading.
        if let Some(r) = first {
            assert!(r.mb > 0);
            assert_eq!(r.cpu, None);
            assert_eq!(meter.read(), first, "no new reading within the interval");
        }
    }
}
