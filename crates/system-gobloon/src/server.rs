//! The socket: one thread per client, requests answered in order.

use std::io;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};

use gob_system::protocol::{
    self, ApplyVirtual, Code, ErrorCode, ErrorReply, Hello, HelloReply, Hidden, OpenHidden,
    PROTOCOL, Record, SetHidden, SetHideEnabled, Text, VirtualRef, caps,
};
use gob_system::{Frame, recv, send};

use crate::Service;

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
pub fn serve(listener: UnixListener, policy: Policy, service: Arc<Service>) -> io::Result<()> {
    for stream in listener.incoming() {
        let stream = stream?;
        let policy = policy.clone();
        let service = service.clone();
        std::thread::spawn(move || {
            if let Err(e) = client(&stream, &policy, &service) {
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

/// Numbers the connections, to know who has a virtual device.
static NEXT_CONN: AtomicU64 = AtomicU64::new(1);

fn client(stream: &UnixStream, policy: &Policy, service: &Service) -> io::Result<()> {
    let uid = peer_uid(stream)?;
    let conn = NEXT_CONN.fetch_add(1, Ordering::Relaxed);
    let result = requests(stream, uid, conn, policy, service);
    // Gone (closed, crashed, refused): its virtual devices are centered and free again.
    service
        .virtuals
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .release_all(conn);
    result
}

fn requests(
    stream: &UnixStream,
    uid: u32,
    conn: u64,
    policy: &Policy,
    service: &Service,
) -> io::Result<()> {
    while let Some(frame) = recv(stream)? {
        // Checked per request: the active user changes when users switch.
        if !policy(uid) {
            log::warn!("uid {uid} may not use the service");
            return error(stream, ErrorCode::Denied);
        }
        let reply = if frame.protocol == PROTOCOL {
            answer(uid, conn, &frame, service)
        } else {
            Err(ErrorCode::Protocol)
        };
        match reply {
            Ok((payload, fd)) => send(
                stream,
                Code::from_u16(frame.code).unwrap_or(Code::Error),
                &payload,
                fd.as_ref().map(AsFd::as_fd),
            )?,
            Err(code) => error(stream, code)?,
        }
    }
    Ok(())
}

fn error(stream: &UnixStream, code: ErrorCode) -> io::Result<()> {
    send(stream, Code::Error, &record(&ErrorReply { code }), None)
}

fn record<R: Record>(r: &R) -> Vec<u8> {
    let mut payload = vec![0; R::SIZE];
    r.write(&mut payload);
    payload
}

fn read<R: Record>(frame: &Frame) -> Result<R, ErrorCode> {
    R::read(&frame.payload).ok_or(ErrorCode::BadRequest)
}

/// A failed system change: logged here, the client only learns that it failed.
fn failed(what: &str, e: io::Error) -> ErrorCode {
    log::error!("{what}: {e}");
    ErrorCode::Failed
}

/// The reply payload of one request and the descriptor that goes with it.
fn answer(
    uid: u32,
    conn: u64,
    frame: &Frame,
    service: &Service,
) -> Result<(Vec<u8>, Option<OwnedFd>), ErrorCode> {
    let virtuals = || {
        service
            .virtuals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    };
    let reply = match Code::from_u16(frame.code) {
        Some(code @ (Code::AcquireJoystick | Code::AcquireGamepad)) => {
            let r: VirtualRef = read(frame)?;
            let gamepad = code == Code::AcquireGamepad;
            let fd = virtuals().acquire(conn, gamepad, r.id)?;
            return Ok((Vec::new(), Some(fd)));
        }
        Some(Code::Release) => {
            let r: VirtualRef = read(frame)?;
            virtuals().release(conn, r.gamepad != 0, r.id);
            Vec::new()
        }
        Some(Code::VirtualState) => {
            let (joysticks, gamepads) = virtuals().state(conn);
            let len = protocol::VirtualState::SIZE
                + joysticks.len() * protocol::Joystick::SIZE
                + gamepads.len() * protocol::Gamepad::SIZE;
            let mut payload = vec![0; len];
            protocol::write_virtual_state(&joysticks, &gamepads, &mut payload)
                .ok_or(ErrorCode::Failed)?;
            payload
        }
        Some(Code::ApplyVirtual) => {
            let change: ApplyVirtual = read(frame)?;
            virtuals().apply(&change)?;
            Vec::new()
        }
        Some(Code::OpenHidden) => {
            let open: OpenHidden = read(frame)?;
            let path = std::path::Path::new(open.path.as_str());
            let hide = service.hide.lock().unwrap_or_else(PoisonError::into_inner);
            let fd = hide.open_hidden(path).map_err(|e| match e.kind() {
                io::ErrorKind::NotFound => ErrorCode::NotFound,
                _ => failed(&path.to_string_lossy(), e),
            })?;
            log::info!("uid {uid}: {} handed over", path.display());
            return Ok((Vec::new(), Some(fd)));
        }
        _ => payload(uid, frame, service)?,
    };
    Ok((reply, None))
}

/// The reply payload of a request without a descriptor.
fn payload(uid: u32, frame: &Frame, service: &Service) -> Result<Vec<u8>, ErrorCode> {
    let hide = || service.hide.lock().unwrap_or_else(PoisonError::into_inner);
    match Code::from_u16(frame.code) {
        Some(Code::Hello) => {
            let hello: Hello = read(frame)?;
            log::info!(
                "uid {uid}: Joystick Gobloon {} connected",
                hello.client_version.as_str()
            );
            Ok(record(&HelloReply {
                protocol: PROTOCOL,
                reserved: 0,
                capabilities: caps::HIDE_EVERYWHERE
                    | caps::VIRTUAL_JOYSTICKS
                    | caps::VIRTUAL_GAMEPADS,
                service_version: Text::new(crate::VERSION),
            }))
        }
        Some(Code::HideState) => {
            let hide = hide();
            let hidden: Vec<Hidden> = hide.hidden();
            let mut payload = vec![0; protocol::HideState::SIZE + hidden.len() * Hidden::SIZE];
            protocol::write_hide_state(hide.enabled(), &hidden, &mut payload)
                .ok_or(ErrorCode::Failed)?;
            Ok(payload)
        }
        Some(Code::SetHideEnabled) => {
            let set: SetHideEnabled = read(frame)?;
            hide()
                .set_enabled(set.enabled != 0)
                .map_err(|e| failed("switching hiding", e))?;
            Ok(Vec::new())
        }
        Some(Code::SetHidden) => {
            let set: SetHidden = read(frame)?;
            hide()
                .set_hidden(&set.device, set.hidden != 0)
                .map_err(|e| failed("hiding", e))?;
            Ok(Vec::new())
        }
        _ => Err(ErrorCode::Unsupported),
    }
}
