//! The SystemGobloon protocol: frames of a fixed header and a payload of fixed little-endian
//! records. Every record has a `SIZE` and explicit byte offsets (tests pin them), so the Linux
//! service and the Windows driver read the same bytes. No allocation here (`no_std`).
//!
//! Frame: [`Header`] (12 bytes: magic `GOBS`, request [`Code`], protocol version, payload
//! length), then the payload. A reply carries the request's code, or [`Code::Error`] with an
//! [`ErrorReply`]. A descriptor (Linux) travels next to the reply, not in it.

/// Protocol version; a service answers other versions with [`ErrorCode::Protocol`].
pub const PROTOCOL: u16 = 1;
pub const MAGIC: [u8; 4] = *b"GOBS";
/// The biggest payload either side accepts.
pub const MAX_PAYLOAD: usize = 64 * 1024;

/// Request codes (on Windows also the IOCTL function numbers). Gaps leave room per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Code {
    Hello = 0x0001,
    /// Only in replies: the request failed, payload [`ErrorReply`].
    Error = 0x0002,
    HideState = 0x0100,
    SetHideEnabled = 0x0101,
    SetHidden = 0x0102,
    OpenHidden = 0x0103,
    VirtualState = 0x0200,
    ApplyVirtual = 0x0201,
    AcquireJoystick = 0x0202,
    AcquireGamepad = 0x0203,
    Release = 0x0204,
}

impl Code {
    pub const ALL: [Code; 11] = [
        Code::Hello,
        Code::Error,
        Code::HideState,
        Code::SetHideEnabled,
        Code::SetHidden,
        Code::OpenHidden,
        Code::VirtualState,
        Code::ApplyVirtual,
        Code::AcquireJoystick,
        Code::AcquireGamepad,
        Code::Release,
    ];

    pub fn from_u16(v: u16) -> Option<Code> {
        Code::ALL.into_iter().find(|c| *c as u16 == v)
    }
}

/// Why a request failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    /// This service or driver does not do that (see [`HelloReply::capabilities`]).
    Unsupported = 1,
    /// The caller may not use the service.
    Denied = 2,
    /// The payload does not fit the request.
    BadRequest = 3,
    /// Another protocol version.
    Protocol = 4,
    /// No such device.
    NotFound = 5,
    /// Another client owns the device.
    Busy = 6,
    /// The system refused (details in the service's log).
    Failed = 7,
}

impl ErrorCode {
    pub fn from_u32(v: u32) -> ErrorCode {
        match v {
            1 => ErrorCode::Unsupported,
            2 => ErrorCode::Denied,
            3 => ErrorCode::BadRequest,
            4 => ErrorCode::Protocol,
            5 => ErrorCode::NotFound,
            6 => ErrorCode::Busy,
            _ => ErrorCode::Failed,
        }
    }
}

/// Bits of [`HelloReply::capabilities`].
pub mod caps {
    /// A list of programs that still see hidden devices (Windows).
    pub const APP_LIST: u32 = 1 << 0;
    /// Hiding for the session only (Windows).
    pub const SESSION_HIDE: u32 = 1 << 1;
    /// Hidden devices are closed to every other program, raw HID included.
    pub const HIDE_EVERYWHERE: u32 = 1 << 2;
    /// Virtual joysticks kept by the service.
    pub const VIRTUAL_JOYSTICKS: u32 = 1 << 3;
    /// Virtual gamepads kept by the service.
    pub const VIRTUAL_GAMEPADS: u32 = 1 << 4;
}

/// A fixed-size record of the protocol.
pub trait Record: Sized {
    const SIZE: usize;
    /// Writes the record into `out[..SIZE]`.
    fn write(&self, out: &mut [u8]);
    /// Reads a record from `bytes[..SIZE]`; `None` when too short or invalid.
    fn read(bytes: &[u8]) -> Option<Self>;
}

fn put_u16(out: &mut [u8], at: usize, v: u16) {
    out[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut [u8], at: usize, v: u32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn get_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn get_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// UTF-8 text in `N` bytes, padded with zeros (cut at a character boundary when longer).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Text<const N: usize>(pub [u8; N]);

impl<const N: usize> Text<N> {
    pub fn new(s: &str) -> Self {
        let mut end = s.len().min(N);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        let mut bytes = [0; N];
        bytes[..end].copy_from_slice(&s.as_bytes()[..end]);
        Text(bytes)
    }

    /// The text up to the first zero (invalid UTF-8: up to the last valid character).
    pub fn as_str(&self) -> &str {
        let end = self.0.iter().position(|&b| b == 0).unwrap_or(N);
        match core::str::from_utf8(&self.0[..end]) {
            Ok(s) => s,
            Err(e) => core::str::from_utf8(&self.0[..e.valid_up_to()]).unwrap_or_default(),
        }
    }

    fn put(&self, out: &mut [u8], at: usize) {
        out[at..at + N].copy_from_slice(&self.0);
    }

    fn get(b: &[u8], at: usize) -> Self {
        let mut bytes = [0; N];
        bytes.copy_from_slice(&b[at..at + N]);
        Text(bytes)
    }
}

impl<const N: usize> core::fmt::Debug for Text<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// Frame header. Bytes: 0 magic, 4 code, 6 protocol, 8 payload length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Header {
    pub magic: [u8; 4],
    pub code: u16,
    pub protocol: u16,
    pub len: u32,
}

impl Header {
    pub fn new(code: Code, len: usize) -> Self {
        Header {
            magic: MAGIC,
            code: code as u16,
            protocol: PROTOCOL,
            len: len as u32,
        }
    }
}

impl Record for Header {
    const SIZE: usize = 12;

    fn write(&self, out: &mut [u8]) {
        out[..4].copy_from_slice(&self.magic);
        put_u16(out, 4, self.code);
        put_u16(out, 6, self.protocol);
        put_u32(out, 8, self.len);
    }

    /// `None` without the magic or with a payload over [`MAX_PAYLOAD`].
    fn read(b: &[u8]) -> Option<Self> {
        if b.len() < Self::SIZE || b[..4] != MAGIC {
            return None;
        }
        let header = Header {
            magic: MAGIC,
            code: get_u16(b, 4),
            protocol: get_u16(b, 6),
            len: get_u32(b, 8),
        };
        (header.len as usize <= MAX_PAYLOAD).then_some(header)
    }
}

/// [`Code::Hello`] request. Bytes: 0 protocol, 2 reserved, 4 client version (32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Hello {
    pub protocol: u16,
    pub reserved: u16,
    pub client_version: Text<32>,
}

impl Hello {
    pub fn new(client_version: &str) -> Self {
        Hello {
            protocol: PROTOCOL,
            reserved: 0,
            client_version: Text::new(client_version),
        }
    }
}

impl Record for Hello {
    const SIZE: usize = 36;

    fn write(&self, out: &mut [u8]) {
        put_u16(out, 0, self.protocol);
        put_u16(out, 2, self.reserved);
        self.client_version.put(out, 4);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| Hello {
            protocol: get_u16(b, 0),
            reserved: get_u16(b, 2),
            client_version: Text::get(b, 4),
        })
    }
}

/// Reply to [`Code::Hello`]. Bytes: 0 protocol, 2 reserved, 4 capabilities ([`caps`]),
/// 8 service version (32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct HelloReply {
    pub protocol: u16,
    pub reserved: u16,
    pub capabilities: u32,
    pub service_version: Text<32>,
}

impl Record for HelloReply {
    const SIZE: usize = 40;

    fn write(&self, out: &mut [u8]) {
        put_u16(out, 0, self.protocol);
        put_u16(out, 2, self.reserved);
        put_u32(out, 4, self.capabilities);
        self.service_version.put(out, 8);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| HelloReply {
            protocol: get_u16(b, 0),
            reserved: get_u16(b, 2),
            capabilities: get_u32(b, 4),
            service_version: Text::get(b, 8),
        })
    }
}

/// Payload of a [`Code::Error`] reply. Bytes: 0 error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ErrorReply {
    pub code: ErrorCode,
}

impl Record for ErrorReply {
    const SIZE: usize = 4;

    fn write(&self, out: &mut [u8]) {
        put_u32(out, 0, self.code as u32);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| ErrorReply {
            code: ErrorCode::from_u32(get_u32(b, 0)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes<R: Record, const N: usize>(r: &R) -> [u8; N] {
        assert_eq!(R::SIZE, N);
        let mut out = [0xAA; N];
        r.write(&mut out);
        out
    }

    #[test]
    fn header_layout() {
        let h = Header::new(Code::SetHidden, 0x0102_0304);
        let b: [u8; 12] = bytes(&h);
        assert_eq!(b, [b'G', b'O', b'B', b'S', 0x02, 0x01, 1, 0, 4, 3, 2, 1]);
        assert_eq!(Header::read(&b), None, "over the payload limit");
        let h = Header::new(Code::Hello, 36);
        let b: [u8; 12] = bytes(&h);
        assert_eq!(Header::read(&b), Some(h));
        assert_eq!(Header::read(&b[..11]), None);
        let mut bad = b;
        bad[0] = b'X';
        assert_eq!(Header::read(&bad), None, "no magic");
    }

    #[test]
    fn hello_layout() {
        let h = Hello::new("0.11.0");
        let b: [u8; 36] = bytes(&h);
        assert_eq!(&b[..10], &[1, 0, 0, 0, b'0', b'.', b'1', b'1', b'.', b'0']);
        assert!(b[10..].iter().all(|&x| x == 0));
        assert_eq!(Hello::read(&b), Some(h));
        assert_eq!(Hello::read(&b[..35]), None);

        let r = HelloReply {
            protocol: PROTOCOL,
            reserved: 0,
            capabilities: caps::HIDE_EVERYWHERE | caps::VIRTUAL_JOYSTICKS,
            service_version: Text::new("1.2.3"),
        };
        let b: [u8; 40] = bytes(&r);
        assert_eq!(&b[..9], &[1, 0, 0, 0, 0b1100, 0, 0, 0, b'1']);
        assert_eq!(HelloReply::read(&b), Some(r));
        assert_eq!(r.service_version.as_str(), "1.2.3");
    }

    #[test]
    fn error_layout() {
        let b: [u8; 4] = bytes(&ErrorReply {
            code: ErrorCode::Busy,
        });
        assert_eq!(b, [6, 0, 0, 0]);
        assert_eq!(
            ErrorReply::read(&[9, 0, 0, 0]).unwrap().code,
            ErrorCode::Failed
        );
    }

    #[test]
    fn codes_are_unique_and_round_trip() {
        for (i, c) in Code::ALL.iter().enumerate() {
            assert_eq!(Code::from_u16(*c as u16), Some(*c));
            assert!(Code::ALL[i + 1..].iter().all(|o| *o as u16 != *c as u16));
        }
        assert_eq!(Code::from_u16(0x7777), None);
    }

    #[test]
    fn text_cuts_at_characters() {
        assert_eq!(Text::<4>::new("abcdef").as_str(), "abcd");
        // "é" is two bytes: it does not fit in the last one.
        assert_eq!(Text::<4>::new("abcé").as_str(), "abc");
        assert_eq!(Text::<4>::new("").as_str(), "");
        assert_eq!(Text::<4>([b'a', 0xFF, 0, 0]).as_str(), "a");
    }
}
