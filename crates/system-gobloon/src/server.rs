//! The socket: one thread per client, requests answered in order.

use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;

use gob_system::protocol::{
    Code, ErrorCode, ErrorReply, Hello, HelloReply, PROTOCOL, Record, Text,
};
use gob_system::{Frame, recv, send};

/// Decides whether a user id may use the service ([`crate::seat::allowed`] in the service).
pub type Policy = Arc<dyn Fn(u32) -> bool + Send + Sync>;

/// Listens on `path` (a stale socket there is replaced). Anyone may connect; requests are
/// checked by the peer's user id.
pub fn bind(path: &Path) -> io::Result<UnixListener> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
    Ok(listener)
}

/// Serves clients until the listener fails.
pub fn serve(listener: UnixListener, policy: Policy) -> io::Result<()> {
    for stream in listener.incoming() {
        let stream = stream?;
        let policy = policy.clone();
        std::thread::spawn(move || {
            if let Err(e) = client(&stream, &policy) {
                log::warn!("client: {e}");
            }
        });
    }
    Ok(())
}

/// The connected process's user id (`SO_PEERCRED`).
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: `cred` and `len` describe a valid buffer of the size SO_PEERCRED writes.
    let r = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &mut len,
        )
    };
    if r < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(cred.uid)
}

fn client(stream: &UnixStream, policy: &Policy) -> io::Result<()> {
    let uid = peer_uid(stream)?;
    while let Some(frame) = recv(stream)? {
        // Checked per request: the active user changes when users switch.
        if !policy(uid) {
            log::warn!("uid {uid} may not use the service");
            return error(stream, ErrorCode::Denied);
        }
        answer(stream, uid, &frame)?;
    }
    Ok(())
}

fn error(stream: &UnixStream, code: ErrorCode) -> io::Result<()> {
    let mut payload = [0; ErrorReply::SIZE];
    ErrorReply { code }.write(&mut payload);
    send(stream, Code::Error, &payload, None)
}

fn answer(stream: &UnixStream, uid: u32, frame: &Frame) -> io::Result<()> {
    if frame.protocol != PROTOCOL {
        return error(stream, ErrorCode::Protocol);
    }
    match Code::from_u16(frame.code) {
        Some(Code::Hello) => {
            let Some(hello) = Hello::read(&frame.payload) else {
                return error(stream, ErrorCode::BadRequest);
            };
            log::info!(
                "uid {uid}: Joystick Gobloon {} connected",
                hello.client_version.as_str()
            );
            let reply = HelloReply {
                protocol: PROTOCOL,
                reserved: 0,
                capabilities: 0,
                service_version: Text::new(crate::VERSION),
            };
            let mut payload = [0; HelloReply::SIZE];
            reply.write(&mut payload);
            send(stream, Code::Hello, &payload, None)
        }
        _ => error(stream, ErrorCode::Unsupported),
    }
}
