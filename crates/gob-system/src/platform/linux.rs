//! Linux transport: frames over the service's Unix stream socket, a descriptor (hidden device
//! node, virtual device) next to a reply in `SCM_RIGHTS`. [`send`] and [`recv`] are used by the
//! service too.

use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::{fmt, mem, ptr};

use crate::protocol::{
    ApplyVirtual, Code, DeviceKind, ErrorCode, ErrorReply, Gamepad, Header, Hello, HelloReply,
    Hidden, Joystick, OpenHidden, Record, SetHidden, SetHideEnabled, Text, VirtualRef,
    hidden_records, virtual_records,
};

/// Where the service listens.
pub const SOCKET_PATH: &str = "/run/system-gobloon.sock";

/// One received frame.
#[derive(Debug)]
pub struct Frame {
    /// Raw code: replies to unknown requests still need it.
    pub code: u16,
    pub protocol: u16,
    pub payload: Vec<u8>,
    pub fd: Option<OwnedFd>,
}

/// Room for one descriptor in a control message, aligned like `cmsghdr`.
#[repr(C, align(8))]
struct Control([u8; 64]);

/// Sends a frame, with `fd` attached to its first bytes.
pub fn send(
    stream: &UnixStream,
    code: Code,
    payload: &[u8],
    fd: Option<BorrowedFd<'_>>,
) -> io::Result<()> {
    if payload.len() > crate::protocol::MAX_PAYLOAD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "payload too big",
        ));
    }
    let mut buf = vec![0; Header::SIZE + payload.len()];
    Header::new(code, payload.len()).write(&mut buf);
    buf[Header::SIZE..].copy_from_slice(payload);

    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    let mut control = Control([0; 64]);
    // SAFETY: zeroed msghdr is valid; the pointers stay alive for the sendmsg call, and the
    // control buffer is big and aligned enough for one SCM_RIGHTS message.
    let sent = unsafe {
        let mut msg: libc::msghdr = mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        if let Some(fd) = fd {
            let len = size_of::<RawFd>() as u32;
            msg.msg_control = control.0.as_mut_ptr().cast();
            msg.msg_controllen = libc::CMSG_SPACE(len) as _;
            let cmsg = libc::CMSG_FIRSTHDR(&msg);
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            (*cmsg).cmsg_len = libc::CMSG_LEN(len) as _;
            ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>(), fd.as_raw_fd());
        }
        libc::sendmsg(stream.as_raw_fd(), &msg, libc::MSG_NOSIGNAL)
    };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut stream = stream;
    stream.write_all(&buf[sent as usize..])
}

/// Receives a frame; `None` when the other side closed the connection between frames.
pub fn recv(stream: &UnixStream) -> io::Result<Option<Frame>> {
    let mut head = [0u8; Header::SIZE];
    let mut iov = libc::iovec {
        iov_base: head.as_mut_ptr().cast(),
        iov_len: head.len(),
    };
    let mut control = Control([0; 64]);
    // SAFETY: as in `send`; descriptors received are owned by us from here on.
    let (got, fd, truncated) = unsafe {
        let mut msg: libc::msghdr = mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.0.as_mut_ptr().cast();
        msg.msg_controllen = control.0.len() as _;
        let got = libc::recvmsg(stream.as_raw_fd(), &mut msg, libc::MSG_CMSG_CLOEXEC);
        let mut fd = None;
        if got >= 0 {
            let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
            while !cmsg.is_null() {
                if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
                    let raw = ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>());
                    // More than one descriptor: the first is kept, the others closed.
                    let owned = OwnedFd::from_raw_fd(raw);
                    fd.get_or_insert(owned);
                }
                cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
            }
        }
        (got, fd, msg.msg_flags & libc::MSG_CTRUNC != 0)
    };
    if got < 0 {
        return Err(io::Error::last_os_error());
    }
    if got == 0 {
        return Ok(None);
    }
    if truncated {
        return Err(invalid("descriptor dropped"));
    }
    let mut stream = stream;
    stream.read_exact(&mut head[got as usize..])?;
    let header = Header::read(&head).ok_or_else(|| invalid("not a SystemGobloon frame"))?;
    let mut payload = vec![0; header.len as usize];
    stream.read_exact(&mut payload)?;
    Ok(Some(Frame {
        code: header.code,
        protocol: header.protocol,
        payload,
        fd,
    }))
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

/// A failed request.
#[derive(Debug)]
pub enum Error {
    /// No service (not installed or not running) or the connection broke.
    Io(io::Error),
    /// The service refused.
    Service(ErrorCode),
    /// The reply made no sense.
    Protocol(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "SystemGobloon: {e}"),
            Error::Service(code) => write!(f, "SystemGobloon refused: {code:?}"),
            Error::Protocol(what) => write!(f, "SystemGobloon protocol: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// A connection to the service. Requests are answered in order, one at a time.
pub struct Client {
    stream: UnixStream,
}

impl Client {
    /// Connects to the installed service.
    pub fn connect() -> Result<Client, Error> {
        Self::connect_to(Path::new(SOCKET_PATH))
    }

    pub fn connect_to(path: &Path) -> Result<Client, Error> {
        Ok(Client {
            stream: UnixStream::connect(path)?,
        })
    }

    /// The service at the other end is gone (stopped or restarted).
    pub fn gone(&self) -> bool {
        hung_up(self.stream.as_raw_fd())
    }

    /// Tells later, even after this connection is closed or replaced, whether the service it
    /// talked to is gone: see [`Watch`].
    pub fn watch(&self) -> io::Result<Watch> {
        Ok(Watch(self.stream.as_fd().try_clone_to_owned()?))
    }

    /// Sends a request and returns the reply's payload and descriptor; an error reply becomes
    /// [`Error::Service`].
    pub fn call(
        &mut self,
        code: Code,
        payload: &[u8],
    ) -> Result<(Vec<u8>, Option<OwnedFd>), Error> {
        send(&self.stream, code, payload, None)?;
        let reply = recv(&self.stream)?.ok_or(Error::Protocol("connection closed"))?;
        if reply.code == Code::Error as u16 {
            let e = ErrorReply::read(&reply.payload).ok_or(Error::Protocol("short error"))?;
            return Err(Error::Service(e.code));
        }
        if reply.code != code as u16 {
            return Err(Error::Protocol("reply to another request"));
        }
        Ok((reply.payload, reply.fd))
    }

    /// Sends a record, reads a record back.
    pub fn call_record<Q: Record, A: Record>(
        &mut self,
        code: Code,
        request: &Q,
    ) -> Result<A, Error> {
        let mut payload = vec![0; Q::SIZE];
        request.write(&mut payload);
        let (reply, _) = self.call(code, &payload)?;
        A::read(&reply).ok_or(Error::Protocol("short reply"))
    }

    /// Protocol version and capabilities of the service; `client_version` goes to its log.
    pub fn hello(&mut self, client_version: &str) -> Result<HelloReply, Error> {
        self.call_record(Code::Hello, &Hello::new(client_version))
    }

    /// Sends a record, expects an empty reply.
    fn call_empty<Q: Record>(&mut self, code: Code, request: &Q) -> Result<(), Error> {
        let mut payload = vec![0; Q::SIZE];
        request.write(&mut payload);
        self.call(code, &payload).map(|_| ())
    }

    /// Whether hiding is on, and the hidden devices.
    pub fn hide_state(&mut self) -> Result<(bool, Vec<Hidden>), Error> {
        let (reply, _) = self.call(Code::HideState, &[])?;
        let (state, records) = hidden_records(&reply).ok_or(Error::Protocol("short state"))?;
        Ok((state.enabled != 0, records.collect()))
    }

    pub fn set_hide_enabled(&mut self, on: bool) -> Result<(), Error> {
        let request = SetHideEnabled {
            enabled: on.into(),
            reserved: [0; 3],
        };
        self.call_empty(Code::SetHideEnabled, &request)
    }

    pub fn set_hidden(&mut self, device: Hidden, hidden: bool) -> Result<(), Error> {
        let request = SetHidden {
            hidden: hidden.into(),
            reserved: [0; 3],
            device,
        };
        self.call_empty(Code::SetHidden, &request)
    }

    /// Opens a node of an attached hidden device, which this user cannot open itself.
    pub fn open_hidden(&mut self, path: &Path) -> Result<OwnedFd, Error> {
        let path = path.to_str().ok_or(Error::Protocol("path not UTF-8"))?;
        if path.len() > 64 {
            return Err(Error::Protocol("path too long"));
        }
        let request = OpenHidden {
            path: Text::new(path),
        };
        let mut payload = vec![0; OpenHidden::SIZE];
        request.write(&mut payload);
        let (_, fd) = self.call(Code::OpenHidden, &payload)?;
        fd.ok_or(Error::Protocol("no descriptor"))
    }

    /// The service's virtual joysticks and gamepads, with who has them.
    pub fn virtual_state(&mut self) -> Result<(Vec<Joystick>, Vec<Gamepad>), Error> {
        let (reply, _) = self.call(Code::VirtualState, &[])?;
        let (joysticks, gamepads) =
            virtual_records(&reply).ok_or(Error::Protocol("short state"))?;
        Ok((joysticks.collect(), gamepads.collect()))
    }

    /// Creates, replaces or deletes a virtual device.
    pub fn apply_virtual(&mut self, change: ApplyVirtual) -> Result<(), Error> {
        self.call_empty(Code::ApplyVirtual, &change)
    }

    /// Takes a virtual device: events written to the descriptor are the device's.
    pub fn acquire(&mut self, kind: DeviceKind, id: u32) -> Result<OwnedFd, Error> {
        let mut payload = vec![0; VirtualRef::SIZE];
        virtual_ref(kind, id).write(&mut payload);
        let (_, fd) = self.call(Code::Acquire, &payload)?;
        fd.ok_or(Error::Protocol("no descriptor"))
    }

    /// Gives a virtual device back; the service centers it.
    pub fn release(&mut self, kind: DeviceKind, id: u32) -> Result<(), Error> {
        self.call_empty(Code::Release, &virtual_ref(kind, id))
    }
}

/// Watches the service a descriptor came from. A lent virtual device lives as long as any
/// process holds its descriptor, so when the service stops or restarts the borrower keeps a
/// copy of the device alive next to the new service's own: drop the descriptor once
/// [`Watch::gone`] says so, and take the device again.
pub struct Watch(OwnedFd);

impl Watch {
    pub fn gone(&self) -> bool {
        hung_up(self.0.as_raw_fd())
    }
}

/// The other end of the stream socket closed (asked without waiting).
fn hung_up(fd: RawFd) -> bool {
    let mut p = libc::pollfd {
        fd,
        events: libc::POLLRDHUP,
        revents: 0,
    };
    // SAFETY: one valid pollfd; a zero timeout only asks.
    let n = unsafe { libc::poll(&mut p, 1, 0) };
    n > 0 && p.revents & (libc::POLLHUP | libc::POLLRDHUP | libc::POLLERR) != 0
}

fn virtual_ref(kind: DeviceKind, id: u32) -> VirtualRef {
    VirtualRef {
        kind: kind as u8,
        reserved: [0; 3],
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Seek;

    #[test]
    fn watch_sees_the_service_go() {
        let (client, service) = UnixStream::pair().unwrap();
        let client = Client { stream: client };
        let watch = client.watch().unwrap();
        assert!(!client.gone() && !watch.gone());
        // The connection is replaced on this side: the service is still there.
        drop(client);
        assert!(!watch.gone());
        drop(service);
        assert!(watch.gone());
    }

    #[test]
    fn frames_carry_payload_and_descriptor() {
        let (a, b) = UnixStream::pair().unwrap();
        let mut file = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
        file.seek(io::SeekFrom::Start(1)).unwrap();
        send(&a, Code::OpenHidden, b"payload", Some(file.as_fd())).unwrap();
        send(&a, Code::Hello, &[], None).unwrap();

        let frame = recv(&b).unwrap().unwrap();
        assert_eq!(frame.code, Code::OpenHidden as u16);
        assert_eq!(frame.payload, b"payload");
        // The same open file: it shares the offset.
        let mut got = std::fs::File::from(frame.fd.expect("descriptor"));
        assert_eq!(got.stream_position().unwrap(), 1);

        let frame = recv(&b).unwrap().unwrap();
        assert_eq!(frame.code, Code::Hello as u16);
        assert!(frame.payload.is_empty() && frame.fd.is_none());

        drop(a);
        assert!(recv(&b).unwrap().is_none(), "closed");
    }

    #[test]
    fn garbage_is_rejected() {
        let (a, b) = UnixStream::pair().unwrap();
        (&a).write_all(b"GET / HTTP/1.1\r\n").unwrap();
        assert_eq!(recv(&b).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
