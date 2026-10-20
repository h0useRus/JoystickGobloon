//! One running copy per user: a second start wakes the first one and exits.
//!
//! The first instance listens on an abstract Unix socket named after the user (no file, so
//! nothing is left behind after a crash); a later start connects to it, which is the signal.

use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};

/// Debug builds get their own name, so a development build runs next to an installed one.
const BASE: &str = if cfg!(debug_assertions) {
    "JoystickGobloon.debug"
} else {
    "JoystickGobloon"
};

/// The listening socket; `None` when it could not be made (the copy then starts without it).
pub(crate) type Event = Option<UnixListener>;

/// This user's socket name (abstract names are shared by all users of the system).
fn name() -> String {
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    format!("{BASE}.{uid}")
}

/// `Some(listener)` for the first instance; `None` when another one runs (it was woken).
pub(super) fn claim() -> Option<Event> {
    claim_named(&name())
}

fn claim_named(name: &str) -> Option<Event> {
    let addr = match SocketAddr::from_abstract_name(name.as_bytes()) {
        Ok(addr) => addr,
        Err(e) => {
            log::warn!("single instance: {e}");
            return Some(None);
        }
    };
    match UnixListener::bind_addr(&addr) {
        Ok(listener) => Some(Some(listener)),
        // Taken: another copy runs. Connecting is its wake-up.
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => match UnixStream::connect_addr(&addr) {
            Ok(_) => None,
            Err(e) => {
                log::warn!("single instance: the running copy does not answer: {e}");
                Some(None)
            }
        },
        Err(e) => {
            log::warn!("single instance: {e}");
            Some(None)
        }
    }
}

/// Calls `show` whenever another start connects.
pub(super) fn listen(listener: Event, show: Box<dyn Fn() + Send>) {
    let Some(listener) = listener else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("gob-instance".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(_) => show(),
                    Err(e) => {
                        log::error!("single instance listener: {e}");
                        return;
                    }
                }
            }
        });
    if let Err(e) = spawned {
        log::error!("single instance listener: {e}");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn second_start_wakes_the_first() {
        let name = format!("JoystickGobloon.test.{}", std::process::id());
        let first = claim_named(&name).expect("first copy");
        assert!(first.is_some(), "listening");
        let (tx, rx) = mpsc::channel();
        listen(first, Box::new(move || tx.send(()).unwrap()));
        assert!(claim_named(&name).is_none(), "second copy exits");
        rx.recv_timeout(Duration::from_secs(2))
            .expect("first copy woken");
    }
}
