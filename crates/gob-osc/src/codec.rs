//! OSC 1.0 packets (with the common 1.1 types): messages and bundles, big-endian, every part
//! padded to 4 bytes.

/// One argument of a message.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// `i` (also `c` char, `r` color and `m` MIDI, read as their 32 bits).
    Int(i32),
    /// `h`.
    Long(i64),
    /// `f`.
    Float(f32),
    /// `d`.
    Double(f64),
    /// `s` (also `S` symbol).
    Str(String),
    /// `b`.
    Blob(Vec<u8>),
    /// `T` / `F`.
    Bool(bool),
    /// `N`.
    Nil,
    /// `I` (impulse, "bang").
    Impulse,
}

impl Arg {
    /// The argument as a number: integers and floats as they are, booleans 1 / 0, text that
    /// reads as a number; else `None`.
    pub fn number(&self) -> Option<f64> {
        match self {
            Arg::Int(v) => Some(f64::from(*v)),
            Arg::Long(v) => Some(*v as f64),
            Arg::Float(v) => Some(f64::from(*v)),
            Arg::Double(v) => Some(*v),
            Arg::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Arg::Str(s) => s.trim().parse().ok(),
            Arg::Blob(_) | Arg::Nil | Arg::Impulse => None,
        }
    }
}

/// One OSC message: an address like `/panel/fader1` and its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub address: String,
    pub args: Vec<Arg>,
}

impl Message {
    pub fn new(address: impl Into<String>, args: Vec<Arg>) -> Self {
        Self {
            address: address.into(),
            args,
        }
    }

    /// The message as one packet.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + self.address.len());
        put_str(&mut out, &self.address);
        let mut tags = String::with_capacity(self.args.len() + 1);
        tags.push(',');
        for a in &self.args {
            tags.push(match a {
                Arg::Int(_) => 'i',
                Arg::Long(_) => 'h',
                Arg::Float(_) => 'f',
                Arg::Double(_) => 'd',
                Arg::Str(_) => 's',
                Arg::Blob(_) => 'b',
                Arg::Bool(true) => 'T',
                Arg::Bool(false) => 'F',
                Arg::Nil => 'N',
                Arg::Impulse => 'I',
            });
        }
        put_str(&mut out, &tags);
        for a in &self.args {
            match a {
                Arg::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
                Arg::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
                Arg::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
                Arg::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
                Arg::Str(s) => put_str(&mut out, s),
                Arg::Blob(b) => {
                    out.extend_from_slice(&(b.len() as i32).to_be_bytes());
                    out.extend_from_slice(b);
                    pad(&mut out);
                }
                Arg::Bool(_) | Arg::Nil | Arg::Impulse => {}
            }
        }
        out
    }
}

/// Why a packet could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// It ends inside a part.
    Truncated,
    /// Not a message or bundle: the address does not start with `/`.
    NotOsc,
    /// A type tag this format does not know.
    UnknownType(char),
    /// Bundles inside bundles deeper than [`MAX_DEPTH`].
    TooDeep,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Truncated => write!(f, "the packet is cut short"),
            DecodeError::NotOsc => write!(f, "not an OSC packet"),
            DecodeError::UnknownType(c) => write!(f, "unknown OSC type tag '{c}'"),
            DecodeError::TooDeep => write!(f, "bundles nested too deep"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Bundles nested deeper than this are refused.
const MAX_DEPTH: usize = 8;

/// The messages of a packet: one, or every message of a bundle (and its bundles) in order.
/// Bundle time tags are ignored: everything counts as "now".
pub fn decode(packet: &[u8]) -> Result<Vec<Message>, DecodeError> {
    let mut out = Vec::new();
    decode_into(packet, 0, &mut out)?;
    Ok(out)
}

fn decode_into(packet: &[u8], depth: usize, out: &mut Vec<Message>) -> Result<(), DecodeError> {
    if depth > MAX_DEPTH {
        return Err(DecodeError::TooDeep);
    }
    if packet.starts_with(b"#bundle\0") {
        // "#bundle", then an 8-byte time tag, then elements: a 32-bit size and the element.
        let mut at = 16;
        if packet.len() < at {
            return Err(DecodeError::Truncated);
        }
        while at < packet.len() {
            let size = read_i32(packet, &mut at)?;
            let size = usize::try_from(size).map_err(|_| DecodeError::Truncated)?;
            let end = at.checked_add(size).ok_or(DecodeError::Truncated)?;
            let element = packet.get(at..end).ok_or(DecodeError::Truncated)?;
            decode_into(element, depth + 1, out)?;
            at = end;
        }
        return Ok(());
    }
    out.push(decode_message(packet)?);
    Ok(())
}

fn decode_message(packet: &[u8]) -> Result<Message, DecodeError> {
    let mut at = 0;
    let address = read_str(packet, &mut at)?;
    if !address.starts_with('/') {
        return Err(DecodeError::NotOsc);
    }
    // Very old senders leave out the type tags: a message without arguments then.
    if at >= packet.len() {
        return Ok(Message::new(address, Vec::new()));
    }
    let tags = read_str(packet, &mut at)?;
    let Some(tags) = tags.strip_prefix(',') else {
        return Ok(Message::new(address, Vec::new()));
    };
    let mut args = Vec::with_capacity(tags.len());
    for tag in tags.chars() {
        let arg = match tag {
            'i' | 'c' | 'r' | 'm' => Arg::Int(read_i32(packet, &mut at)?),
            'h' | 't' => Arg::Long(i64::from_be_bytes(read_8(packet, &mut at)?)),
            'f' => Arg::Float(f32::from_bits(read_i32(packet, &mut at)? as u32)),
            'd' => Arg::Double(f64::from_be_bytes(read_8(packet, &mut at)?)),
            's' | 'S' => Arg::Str(read_str(packet, &mut at)?),
            'b' => {
                let size = usize::try_from(read_i32(packet, &mut at)?)
                    .map_err(|_| DecodeError::Truncated)?;
                let end = at.checked_add(size).ok_or(DecodeError::Truncated)?;
                let blob = packet.get(at..end).ok_or(DecodeError::Truncated)?.to_vec();
                at = padded(end);
                Arg::Blob(blob)
            }
            'T' => Arg::Bool(true),
            'F' => Arg::Bool(false),
            'N' => Arg::Nil,
            'I' => Arg::Impulse,
            // Arrays: their elements count as arguments of the message.
            '[' | ']' => continue,
            other => return Err(DecodeError::UnknownType(other)),
        };
        args.push(arg);
    }
    Ok(Message::new(address, args))
}

/// `n` rounded up to a multiple of 4.
fn padded(n: usize) -> usize {
    n.div_ceil(4) * 4
}

/// A null-terminated string padded to 4 bytes; invalid UTF-8 is replaced.
fn read_str(packet: &[u8], at: &mut usize) -> Result<String, DecodeError> {
    let rest = packet.get(*at..).ok_or(DecodeError::Truncated)?;
    let len = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or(DecodeError::Truncated)?;
    let text = String::from_utf8_lossy(&rest[..len]).into_owned();
    *at = padded(*at + len + 1);
    Ok(text)
}

fn read_i32(packet: &[u8], at: &mut usize) -> Result<i32, DecodeError> {
    let bytes = packet.get(*at..*at + 4).ok_or(DecodeError::Truncated)?;
    *at += 4;
    Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_8(packet: &[u8], at: &mut usize) -> Result<[u8; 8], DecodeError> {
    let bytes = packet.get(*at..*at + 8).ok_or(DecodeError::Truncated)?;
    *at += 8;
    let mut out = [0; 8];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    // A null inside would end the string early for the reader.
    out.extend(s.bytes().filter(|&b| b != 0));
    out.push(0);
    pad(out);
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_round_trips() {
        let m = Message::new(
            "/panel/fader1",
            vec![
                Arg::Int(-7),
                Arg::Float(0.25),
                Arg::Str("Combat".into()),
                Arg::Bool(true),
                Arg::Bool(false),
                Arg::Long(1 << 40),
                Arg::Double(0.5),
                Arg::Blob(vec![1, 2, 3]),
                Arg::Nil,
                Arg::Impulse,
            ],
        );
        let bytes = m.encode();
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(decode(&bytes).unwrap(), vec![m]);
    }

    #[test]
    fn known_bytes_decode() {
        // "/a" + pad, ",if" + pad, 1000, 0.5
        let bytes = [
            b'/', b'a', 0, 0, b',', b'i', b'f', 0, 0, 0, 3, 0xE8, 0x3F, 0, 0, 0,
        ];
        assert_eq!(
            decode(&bytes).unwrap(),
            vec![Message::new("/a", vec![Arg::Int(1000), Arg::Float(0.5)])]
        );
        // The same written by us.
        assert_eq!(
            Message::new("/a", vec![Arg::Int(1000), Arg::Float(0.5)]).encode(),
            bytes
        );
    }

    #[test]
    fn bundles_give_their_messages_in_order() {
        let a = Message::new("/a", vec![Arg::Int(1)]).encode();
        let b = Message::new("/b", vec![]).encode();
        let mut inner = b"#bundle\0".to_vec();
        inner.extend_from_slice(&[0; 8]);
        inner.extend_from_slice(&(b.len() as i32).to_be_bytes());
        inner.extend_from_slice(&b);
        let mut bundle = b"#bundle\0".to_vec();
        bundle.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        for part in [&a, &inner] {
            bundle.extend_from_slice(&(part.len() as i32).to_be_bytes());
            bundle.extend_from_slice(part);
        }
        let got = decode(&bundle).unwrap();
        assert_eq!(
            got.iter().map(|m| m.address.as_str()).collect::<Vec<_>>(),
            ["/a", "/b"]
        );
    }

    #[test]
    fn broken_packets_are_refused_not_panicking() {
        assert_eq!(decode(b"hello\0\0\0"), Err(DecodeError::NotOsc));
        assert_eq!(decode(b"/abc"), Err(DecodeError::Truncated));
        assert_eq!(decode(b"/a\0\0,i\0\0\0\0"), Err(DecodeError::Truncated));
        assert_eq!(decode(b"/a\0\0,x\0\0"), Err(DecodeError::UnknownType('x')));
        let mut bundle = b"#bundle\0".to_vec();
        bundle.extend_from_slice(&[0; 8]);
        bundle.extend_from_slice(&100i32.to_be_bytes());
        assert_eq!(decode(&bundle), Err(DecodeError::Truncated));
        // Every cut of a good packet fails cleanly.
        let good = Message::new("/x/y", vec![Arg::Str("abc".into()), Arg::Int(1)]).encode();
        for n in 0..good.len() {
            let _ = decode(&good[..n]);
        }
    }

    #[test]
    fn no_type_tags_means_no_arguments() {
        assert_eq!(
            decode(b"/go\0").unwrap(),
            vec![Message::new("/go", Vec::new())]
        );
    }

    #[test]
    fn numbers_of_arguments() {
        assert_eq!(Arg::Int(3).number(), Some(3.0));
        assert_eq!(Arg::Bool(true).number(), Some(1.0));
        assert_eq!(Arg::Str(" 0.5 ".into()).number(), Some(0.5));
        assert_eq!(Arg::Str("on".into()).number(), None);
        assert_eq!(Arg::Impulse.number(), None);
    }
}
