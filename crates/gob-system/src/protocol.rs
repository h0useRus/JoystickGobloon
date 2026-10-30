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
    /// Virtual joysticks kept by the service (layouts set without administrator rights).
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

/// A hidden device. Bytes: 0 id (40, gob-input's device id), 40 name (96), 136 vendor,
/// 138 product, 140 present (1: attached now), 141 reserved (3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Hidden {
    pub id: Text<40>,
    pub name: Text<96>,
    pub vendor: u16,
    pub product: u16,
    pub present: u8,
    pub reserved: [u8; 3],
}

impl Record for Hidden {
    const SIZE: usize = 144;

    fn write(&self, out: &mut [u8]) {
        self.id.put(out, 0);
        self.name.put(out, 40);
        put_u16(out, 136, self.vendor);
        put_u16(out, 138, self.product);
        out[140] = self.present;
        out[141..144].copy_from_slice(&self.reserved);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| Hidden {
            id: Text::get(b, 0),
            name: Text::get(b, 40),
            vendor: get_u16(b, 136),
            product: get_u16(b, 138),
            present: b[140],
            reserved: [b[141], b[142], b[143]],
        })
    }
}

/// Reply to [`Code::HideState`]: this record, then `count` [`Hidden`] records. Bytes: 0 enabled
/// (1: hiding is on), 1 reserved (3), 4 count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct HideState {
    pub enabled: u8,
    pub reserved: [u8; 3],
    pub count: u32,
}

impl Record for HideState {
    const SIZE: usize = 8;

    fn write(&self, out: &mut [u8]) {
        out[0] = self.enabled;
        out[1..4].copy_from_slice(&self.reserved);
        put_u32(out, 4, self.count);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| HideState {
            enabled: b[0],
            reserved: [b[1], b[2], b[3]],
            count: get_u32(b, 4),
        })
    }
}

/// [`Code::SetHideEnabled`] request (empty reply). Bytes: 0 enabled, 1 reserved (3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct SetHideEnabled {
    pub enabled: u8,
    pub reserved: [u8; 3],
}

impl Record for SetHideEnabled {
    const SIZE: usize = 4;

    fn write(&self, out: &mut [u8]) {
        out[0] = self.enabled;
        out[1..4].copy_from_slice(&self.reserved);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| SetHideEnabled {
            enabled: b[0],
            reserved: [b[1], b[2], b[3]],
        })
    }
}

/// [`Code::SetHidden`] request (empty reply): hides (`hidden` 1) or shows a device. Bytes:
/// 0 hidden, 1 reserved (3), 4 device ([`Hidden`], its `present` ignored).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct SetHidden {
    pub hidden: u8,
    pub reserved: [u8; 3],
    pub device: Hidden,
}

impl Record for SetHidden {
    const SIZE: usize = 4 + Hidden::SIZE;

    fn write(&self, out: &mut [u8]) {
        out[0] = self.hidden;
        out[1..4].copy_from_slice(&self.reserved);
        self.device.write(&mut out[4..]);
    }

    fn read(b: &[u8]) -> Option<Self> {
        if b.len() < Self::SIZE {
            return None;
        }
        Some(SetHidden {
            hidden: b[0],
            reserved: [b[1], b[2], b[3]],
            device: Hidden::read(&b[4..])?,
        })
    }
}

/// [`Code::OpenHidden`] request: a node of an attached hidden device, which this user may no
/// longer open (Linux: `/dev/input/eventN`, `/dev/input/jsN`, `/dev/hidrawN`). The reply is empty
/// with the open descriptor (read-only, non-blocking) next to it. Bytes: 0 path (64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct OpenHidden {
    pub path: Text<64>,
}

impl Record for OpenHidden {
    const SIZE: usize = 64;

    fn write(&self, out: &mut [u8]) {
        self.path.put(out, 0);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| OpenHidden {
            path: Text::get(b, 0),
        })
    }
}

/// A [`HideState`] and its [`Hidden`] records as one payload; `None` when the records do not fit.
pub fn write_hide_state(enabled: bool, hidden: &[Hidden], out: &mut [u8]) -> Option<usize> {
    let len = HideState::SIZE + hidden.len() * Hidden::SIZE;
    if out.len() < len {
        return None;
    }
    HideState {
        enabled: enabled.into(),
        reserved: [0; 3],
        count: hidden.len() as u32,
    }
    .write(out);
    for (i, h) in hidden.iter().enumerate() {
        h.write(&mut out[HideState::SIZE + i * Hidden::SIZE..]);
    }
    Some(len)
}

/// The [`Hidden`] records of a [`HideState`] payload, in order.
pub fn hidden_records(payload: &[u8]) -> Option<(HideState, impl Iterator<Item = Hidden> + '_)> {
    let state = HideState::read(payload)?;
    let records = &payload[HideState::SIZE..];
    if records.len() < state.count as usize * Hidden::SIZE {
        return None;
    }
    let all = records
        .as_chunks::<{ Hidden::SIZE }>()
        .0
        .iter()
        .take(state.count as usize)
        .filter_map(|r| Hidden::read(r));
    Some((state, all))
}

/// Who has a virtual device, as the service sees the asking connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Owner {
    Free = 0,
    /// This connection.
    You = 1,
    /// Another connection.
    Other = 2,
}

impl Owner {
    pub fn from_u8(v: u8) -> Owner {
        match v {
            1 => Owner::You,
            2 => Owner::Other,
            _ => Owner::Free,
        }
    }
}

/// A virtual joystick's layout (and, in replies, who has it). Bytes: 0 id, 4 buttons, 6 axes
/// (bit n = axis slot n + 1: X, Y, Z, Rx, Ry, Rz, Slider 0, Slider 1), 7 hats, 8 flags (bit 0:
/// 4-direction hats), 9 owner ([`Owner`]), 10 product id (0: by the id), 12 name (80, empty:
/// the default name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Joystick {
    pub id: u32,
    pub buttons: u16,
    pub axes: u8,
    pub hats: u8,
    pub flags: u8,
    pub owner: u8,
    pub product: u16,
    pub name: Text<80>,
}

impl Joystick {
    pub const DISCRETE_HATS: u8 = 1;
}

impl Record for Joystick {
    const SIZE: usize = 92;

    fn write(&self, out: &mut [u8]) {
        put_u32(out, 0, self.id);
        put_u16(out, 4, self.buttons);
        out[6] = self.axes;
        out[7] = self.hats;
        out[8] = self.flags;
        out[9] = self.owner;
        put_u16(out, 10, self.product);
        self.name.put(out, 12);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| Joystick {
            id: get_u32(b, 0),
            buttons: get_u16(b, 4),
            axes: b[6],
            hats: b[7],
            flags: b[8],
            owner: b[9],
            product: get_u16(b, 10),
            name: Text::get(b, 12),
        })
    }
}

/// A virtual gamepad (fixed Xbox 360 layout). Bytes: 0 id, 4 owner ([`Owner`]), 5 reserved (3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Gamepad {
    pub id: u32,
    pub owner: u8,
    pub reserved: [u8; 3],
}

impl Record for Gamepad {
    const SIZE: usize = 8;

    fn write(&self, out: &mut [u8]) {
        put_u32(out, 0, self.id);
        out[4] = self.owner;
        out[5..8].copy_from_slice(&self.reserved);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| Gamepad {
            id: get_u32(b, 0),
            owner: b[4],
            reserved: [b[5], b[6], b[7]],
        })
    }
}

/// Reply to [`Code::VirtualState`]: this record, `joysticks` [`Joystick`] records, then
/// `gamepads` [`Gamepad`] records. Bytes: 0 joysticks, 4 gamepads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct VirtualState {
    pub joysticks: u32,
    pub gamepads: u32,
}

impl Record for VirtualState {
    const SIZE: usize = 8;

    fn write(&self, out: &mut [u8]) {
        put_u32(out, 0, self.joysticks);
        put_u32(out, 4, self.gamepads);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| VirtualState {
            joysticks: get_u32(b, 0),
            gamepads: get_u32(b, 4),
        })
    }
}

/// The payload of a [`VirtualState`] reply; `None` when `out` is too small.
pub fn write_virtual_state(
    joysticks: &[Joystick],
    gamepads: &[Gamepad],
    out: &mut [u8],
) -> Option<usize> {
    let len =
        VirtualState::SIZE + joysticks.len() * Joystick::SIZE + gamepads.len() * Gamepad::SIZE;
    if out.len() < len {
        return None;
    }
    VirtualState {
        joysticks: joysticks.len() as u32,
        gamepads: gamepads.len() as u32,
    }
    .write(out);
    let mut at = VirtualState::SIZE;
    for j in joysticks {
        j.write(&mut out[at..]);
        at += Joystick::SIZE;
    }
    for g in gamepads {
        g.write(&mut out[at..]);
        at += Gamepad::SIZE;
    }
    Some(len)
}

/// The records of a [`VirtualState`] reply, joysticks and gamepads in order.
pub fn virtual_records(
    payload: &[u8],
) -> Option<(
    impl Iterator<Item = Joystick> + '_,
    impl Iterator<Item = Gamepad> + '_,
)> {
    let state = VirtualState::read(payload)?;
    let (j, g) = (state.joysticks as usize, state.gamepads as usize);
    let joysticks = payload.get(VirtualState::SIZE..VirtualState::SIZE + j * Joystick::SIZE)?;
    let rest = VirtualState::SIZE + j * Joystick::SIZE;
    let gamepads = payload.get(rest..rest + g * Gamepad::SIZE)?;
    Some((
        joysticks.chunks(Joystick::SIZE).filter_map(Joystick::read),
        gamepads.chunks(Gamepad::SIZE).filter_map(Gamepad::read),
    ))
}

/// What an [`ApplyVirtual`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChangeKind {
    /// Create or replace joystick `joystick.id` with that layout.
    SetJoystick = 1,
    DeleteJoystick = 2,
    AddGamepad = 3,
    DeleteGamepad = 4,
}

impl ChangeKind {
    pub fn from_u8(v: u8) -> Option<ChangeKind> {
        Some(match v {
            1 => ChangeKind::SetJoystick,
            2 => ChangeKind::DeleteJoystick,
            3 => ChangeKind::AddGamepad,
            4 => ChangeKind::DeleteGamepad,
            _ => return None,
        })
    }
}

/// [`Code::ApplyVirtual`] request (empty reply; [`ErrorCode::Busy`] while someone has the
/// device). Bytes: 0 kind ([`ChangeKind`]), 1 reserved (3), 4 joystick (the layout for
/// `SetJoystick`; only its id for the others, gamepads included).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ApplyVirtual {
    pub kind: u8,
    pub reserved: [u8; 3],
    pub joystick: Joystick,
}

impl Record for ApplyVirtual {
    const SIZE: usize = 4 + Joystick::SIZE;

    fn write(&self, out: &mut [u8]) {
        out[0] = self.kind;
        out[1..4].copy_from_slice(&self.reserved);
        self.joystick.write(&mut out[4..]);
    }

    fn read(b: &[u8]) -> Option<Self> {
        if b.len() < Self::SIZE {
            return None;
        }
        Some(ApplyVirtual {
            kind: b[0],
            reserved: [b[1], b[2], b[3]],
            joystick: Joystick::read(&b[4..])?,
        })
    }
}

/// [`Code::AcquireJoystick`], [`Code::AcquireGamepad`] and [`Code::Release`] request: a device.
/// Acquiring replies empty with the device's descriptor (Linux: its uinput descriptor; write
/// events to it); [`ErrorCode::Busy`] when another connection has it. Releasing centers it.
/// Bytes: 0 gamepad (1: a gamepad, for `Release`), 1 reserved (3), 4 id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct VirtualRef {
    pub gamepad: u8,
    pub reserved: [u8; 3],
    pub id: u32,
}

impl Record for VirtualRef {
    const SIZE: usize = 8;

    fn write(&self, out: &mut [u8]) {
        out[0] = self.gamepad;
        out[1..4].copy_from_slice(&self.reserved);
        put_u32(out, 4, self.id);
    }

    fn read(b: &[u8]) -> Option<Self> {
        (b.len() >= Self::SIZE).then(|| VirtualRef {
            gamepad: b[0],
            reserved: [b[1], b[2], b[3]],
            id: get_u32(b, 4),
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
    fn hiding_layout() {
        let h = Hidden {
            id: Text::new("1f4b044f-0003-0000-0011-223344556677"),
            name: Text::new("T.16000M"),
            vendor: 0x044f,
            product: 0xb10a,
            present: 1,
            reserved: [0; 3],
        };
        let b: [u8; 144] = bytes(&h);
        assert_eq!(&b[..4], b"1f4b");
        assert_eq!(&b[40..48], b"T.16000M");
        assert_eq!(&b[136..141], &[0x4f, 0x04, 0x0a, 0xb1, 1]);
        assert_eq!(Hidden::read(&b), Some(h));

        let set = SetHidden {
            hidden: 1,
            reserved: [0; 3],
            device: h,
        };
        let b: [u8; 148] = bytes(&set);
        assert_eq!(&b[..5], &[1, 0, 0, 0, b'1']);
        assert_eq!(SetHidden::read(&b), Some(set));
        let b: [u8; 4] = bytes(&SetHideEnabled {
            enabled: 1,
            reserved: [0; 3],
        });
        assert_eq!(b, [1, 0, 0, 0]);

        let mut out = [0; 8 + 2 * 144];
        assert_eq!(write_hide_state(true, &[h, h], &mut out), Some(out.len()));
        assert_eq!(&out[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        let (state, records) = hidden_records(&out).unwrap();
        assert_eq!(state.enabled, 1);
        assert_eq!(records.collect::<Vec<_>>(), [h, h]);
        assert!(
            hidden_records(&out[..8 + 144]).is_none(),
            "one record missing"
        );
        assert_eq!(write_hide_state(true, &[h, h, h], &mut out), None);
    }

    #[test]
    fn open_hidden_layout() {
        let r = OpenHidden {
            path: Text::new("/dev/hidraw3"),
        };
        let b: [u8; 64] = bytes(&r);
        assert_eq!(&b[..13], b"/dev/hidraw3\0");
        assert_eq!(OpenHidden::read(&b), Some(r));
    }

    #[test]
    fn virtual_layout() {
        let j = Joystick {
            id: 2,
            buttons: 95,
            axes: 0b1100_0011,
            hats: 4,
            flags: Joystick::DISCRETE_HATS,
            owner: Owner::You as u8,
            product: 0xBE02,
            name: Text::new("Gobloon Stick"),
        };
        let b: [u8; 92] = bytes(&j);
        assert_eq!(
            &b[..14],
            [
                2,
                0,
                0,
                0,
                95,
                0,
                0b1100_0011,
                4,
                1,
                1,
                0x02,
                0xBE,
                b'G',
                b'o'
            ]
        );
        assert_eq!(b[91], 0);
        assert_eq!(Joystick::read(&b), Some(j));
        let g = Gamepad {
            id: 3,
            owner: Owner::Other as u8,
            reserved: [0; 3],
        };
        let b: [u8; 8] = bytes(&g);
        assert_eq!(b, [3, 0, 0, 0, 2, 0, 0, 0]);

        let mut out = [0; 8 + 2 * 92 + 8];
        assert_eq!(
            write_virtual_state(&[j, j], &[g], &mut out),
            Some(out.len())
        );
        assert_eq!(&out[..8], &[2, 0, 0, 0, 1, 0, 0, 0]);
        let (js, gs) = virtual_records(&out).unwrap();
        assert_eq!(js.collect::<Vec<_>>(), [j, j]);
        assert_eq!(gs.collect::<Vec<_>>(), [g]);
        assert!(
            virtual_records(&out[..out.len() - 1]).is_none(),
            "gamepad cut off"
        );

        let a = ApplyVirtual {
            kind: ChangeKind::SetJoystick as u8,
            reserved: [0; 3],
            joystick: j,
        };
        let b: [u8; 96] = bytes(&a);
        assert_eq!(&b[..5], &[1, 0, 0, 0, 2]);
        assert_eq!(ApplyVirtual::read(&b), Some(a));
        assert_eq!(ChangeKind::from_u8(9), None);
        let r = VirtualRef {
            gamepad: 1,
            reserved: [0; 3],
            id: 4,
        };
        let b: [u8; 8] = bytes(&r);
        assert_eq!(b, [1, 0, 0, 0, 4, 0, 0, 0]);
        assert_eq!(Owner::from_u8(7), Owner::Free);
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
