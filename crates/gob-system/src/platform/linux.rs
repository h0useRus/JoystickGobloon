//! Linux transport: frames over the service's Unix stream socket, a descriptor (hidden device
//! node, virtual device) next to a reply in `SCM_RIGHTS`. [`send`] and [`recv`] are used by the
//! service too.

use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::{fmt, mem, ptr};

use crate::protocol::{Code, ErrorCode, ErrorReply, Header, Hello, HelloReply, Record};

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Seek;
    use std::os::fd::AsFd;

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
