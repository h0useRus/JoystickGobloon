//! The Luau state every piece of user code runs in (user scripts here, game export plugins in
//! gob-export): sandboxed by the caller once its globals are set, every call stopped when it runs
//! past its time budget, and its memory capped.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mlua::{Lua, VmState};

/// The running call's time limit, checked by the state's interrupt.
pub struct Deadline {
    /// Nanoseconds after `clock` at which the running call is stopped.
    at: Arc<AtomicU64>,
    clock: Instant,
}

impl Deadline {
    /// Calls from now on may run for `budget`.
    pub fn arm(&self, budget: Duration) {
        let at = (self.clock.elapsed() + budget).as_nanos() as u64;
        self.at.store(at, Ordering::Relaxed);
    }

    /// No call is running: nothing to stop.
    pub fn disarm(&self) {
        self.at.store(u64::MAX, Ordering::Relaxed);
    }
}

/// A new state using at most `memory` bytes, with the interrupt that enforces its [`Deadline`].
/// The caller sets its globals, then calls `lua.sandbox(true)`.
pub fn new_state(memory: usize) -> mlua::Result<(Lua, Deadline)> {
    let lua = Lua::new();
    lua.set_memory_limit(memory)?;
    let deadline = Deadline {
        at: Arc::new(AtomicU64::new(u64::MAX)),
        clock: Instant::now(),
    };
    let (at, clock) = (deadline.at.clone(), deadline.clock);
    lua.set_interrupt(move |_| {
        if clock.elapsed().as_nanos() as u64 > at.load(Ordering::Relaxed) {
            Err(mlua::Error::runtime("ran too long and was stopped"))
        } else {
            Ok(VmState::Continue)
        }
    });
    Ok((lua, deadline))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_runaway_loop_is_stopped() {
        let (lua, deadline) = new_state(1 << 20).unwrap();
        deadline.arm(Duration::from_millis(20));
        let e = lua.load("while true do end").exec().unwrap_err();
        assert!(e.to_string().contains("ran too long"), "{e}");
        deadline.disarm();
        lua.load("local x = 1").exec().unwrap();
    }

    #[test]
    fn memory_past_the_limit_fails() {
        let (lua, _) = new_state(1 << 20).unwrap();
        let e = lua
            .load("local t = {} for i = 1, 1e7 do t[i] = i end")
            .exec()
            .unwrap_err();
        assert!(matches!(e, mlua::Error::MemoryError(_)), "{e}");
    }
}
