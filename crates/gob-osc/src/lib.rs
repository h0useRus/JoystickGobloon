//! Open Sound Control over UDP: the packet format ([`decode`], [`Message::encode`]), a
//! [`Listener`] that receives messages on one port and a [`Sender`] that sends them from a
//! thread of its own (host names are looked up there, so callers never wait).
//!
//! OSC is what touch panels (TouchOSC, Open Stage Control) and Bitfocus Companion (Stream Deck,
//! Loupedeck and other button panels) speak. Plain `std::net`: the same on every system.

mod codec;
mod net;

pub use codec::{Arg, DecodeError, Message, decode};
pub use net::{DEFAULT_PORT, Listener, Sender};
