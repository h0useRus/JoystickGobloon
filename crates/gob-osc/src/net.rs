//! UDP: a listener thread per port and one sender thread.

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Message, decode};

/// The port messages are received on unless set otherwise (the usual "send to" port of touch
/// panels).
pub const DEFAULT_PORT: u16 = 8000;

/// How often the listener thread looks whether it should stop.
const STOP_CHECK: Duration = Duration::from_millis(200);
/// Largest UDP payload.
const MAX_PACKET: usize = 65_536;

/// Receives OSC packets on one UDP port in a thread of its own; stops when dropped.
pub struct Listener {
    local: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// Binds `addr` (`127.0.0.1:port`: only this computer can send; `0.0.0.0:port`: the network
    /// too) and calls `on_message` with every message received and who sent it. Packets that are
    /// not OSC are logged (debug) and skipped. Fails when the port cannot be taken (another
    /// program has it).
    pub fn start(
        addr: SocketAddr,
        mut on_message: impl FnMut(Message, SocketAddr) + Send + 'static,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_read_timeout(Some(STOP_CHECK))?;
        let local = socket.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("gob-osc".into())
                .spawn(move || {
                    let mut buf = vec![0u8; MAX_PACKET];
                    while !stop.load(Ordering::Relaxed) {
                        let (len, from) = match socket.recv_from(&mut buf) {
                            Ok(r) => r,
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                                ) =>
                            {
                                continue;
                            }
                            // Windows reports an earlier send's ICMP "port unreachable" here.
                            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                            Err(e) => {
                                log::warn!("OSC: receiving on {local} stopped: {e}");
                                break;
                            }
                        };
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        match decode(&buf[..len]) {
                            Ok(messages) => {
                                for m in messages {
                                    on_message(m, from);
                                }
                            }
                            Err(e) => log::debug!("OSC: packet from {from} skipped: {e}"),
                        }
                    }
                })?
        };
        Ok(Self {
            local,
            stop,
            thread: Some(thread),
        })
    }

    /// The address it listens on (the real port when 0 was asked for).
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wakes the thread at once instead of after its read timeout: the caller (the GUI,
        // changing the port) must not wait.
        let mut wake = self.local;
        if wake.ip().is_unspecified() {
            wake.set_ip(match wake {
                SocketAddr::V4(_) => std::net::Ipv4Addr::LOCALHOST.into(),
                SocketAddr::V6(_) => std::net::Ipv6Addr::LOCALHOST.into(),
            });
        }
        let any: SocketAddr = if wake.is_ipv4() {
            "0.0.0.0:0".parse().expect("valid address")
        } else {
            "[::]:0".parse().expect("valid address")
        };
        if let Ok(s) = UdpSocket::bind(any) {
            let _ = s.send_to(&[], wake);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Sends OSC messages from a thread of its own: [`Sender::send`] never waits, also not for a
/// host name to be looked up. Stops when dropped.
pub struct Sender {
    tx: mpsc::Sender<(String, u16, Vec<u8>)>,
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

impl Sender {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<(String, u16, Vec<u8>)>();
        let spawned = std::thread::Builder::new()
            .name("gob-osc-send".into())
            .spawn(move || send_loop(&rx));
        if let Err(e) = spawned {
            log::error!("OSC: the sending thread did not start: {e}");
        }
        Self { tx }
    }

    /// Sends `message` to `host` (an address or a name) at `port`.
    pub fn send(&self, host: &str, port: u16, message: &Message) {
        let _ = self
            .tx
            .send((host.trim().to_string(), port, message.encode()));
    }
}

fn send_loop(rx: &mpsc::Receiver<(String, u16, Vec<u8>)>) {
    let mut v4: Option<UdpSocket> = None;
    let mut v6: Option<UdpSocket> = None;
    let mut resolved: HashMap<(String, u16), SocketAddr> = HashMap::new();
    // Hosts that failed, logged once until they work again.
    let mut failing: HashMap<String, String> = HashMap::new();
    for (host, port, packet) in rx {
        let key = (host.clone(), port);
        let target = match resolved.get(&key) {
            Some(a) => Ok(*a),
            None => resolve(&host, port),
        };
        let result = target.and_then(|target| {
            resolved.insert(key, target);
            let slot = if target.is_ipv4() { &mut v4 } else { &mut v6 };
            if slot.is_none() {
                let any: SocketAddr = if target.is_ipv4() {
                    "0.0.0.0:0".parse().expect("valid address")
                } else {
                    "[::]:0".parse().expect("valid address")
                };
                *slot = Some(UdpSocket::bind(any)?);
            }
            let socket = slot.as_ref().expect("bound above");
            socket.send_to(&packet, target).map(|_| ())
        });
        match result {
            Ok(()) => {
                if failing.remove(&host).is_some() {
                    log::info!("OSC: sending to {host} works again");
                }
            }
            Err(e) => {
                let text = e.to_string();
                if failing.get(&host) != Some(&text) {
                    log::warn!("OSC: could not send to {host}:{port}: {text}");
                    failing.insert(host.clone(), text);
                }
                resolved.retain(|(h, _), _| h != &host);
            }
        }
    }
}

/// `localhost` and empty are this computer (IPv4, as most panels listen there).
fn resolve(host: &str, port: u16) -> io::Result<SocketAddr> {
    let host = if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1"
    } else {
        host
    };
    let mut found: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    // IPv4 first: a name with both is usually reached that way by panels.
    found.sort_by_key(|a| !a.is_ipv4());
    found
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address for this host"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Arg;
    use std::time::Instant;

    #[test]
    fn messages_go_from_the_sender_to_the_listener() {
        let (tx, rx) = mpsc::channel();
        let listener = Listener::start("127.0.0.1:0".parse().unwrap(), move |m, _| {
            let _ = tx.send(m);
        })
        .unwrap();
        let port = listener.local_addr().port();
        let sender = Sender::new();
        let sent = Message::new("/gobloon/1", vec![Arg::Int(1)]);
        sender.send("localhost", port, &sent);
        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got, sent);
    }

    #[test]
    fn a_taken_port_fails_to_start() {
        let first = Listener::start("127.0.0.1:0".parse().unwrap(), |_, _| {}).unwrap();
        let again = Listener::start(first.local_addr(), |_, _| {});
        assert!(again.is_err());
    }

    #[test]
    fn dropping_the_listener_frees_its_port_soon() {
        let listener = Listener::start("127.0.0.1:0".parse().unwrap(), |_, _| {}).unwrap();
        let addr = listener.local_addr();
        let start = Instant::now();
        drop(listener);
        assert!(
            start.elapsed() < STOP_CHECK,
            "woken, not waiting for the read timeout"
        );
        Listener::start(addr, |_, _| {}).unwrap();
    }
}
