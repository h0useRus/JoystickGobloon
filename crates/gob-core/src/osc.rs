//! OSC inputs (Joystick Gobloon only): messages from touch panels and Bitfocus Companion
//! (Stream Deck and other button panels) act like axes and buttons. Each OSC address the
//! profile knows is an input of the OSC device [`OSC_GUID`] (input type `axis` or `button`,
//! input id = the input's id) and is bound to actions like a joystick's.
//!
//! A message's first argument is the value: an axis maps `low..high` onto its travel, a button
//! is pressed from half of 1 up (1 / 0, true / false); a button message without a number (no
//! arguments, an impulse) is a short press.
//!
//! The `send-osc` action sends messages back, e.g. to put the mode on a Stream Deck key.

use crate::profile::{Profile, Section};

/// Device id of OSC input bindings.
pub const OSC_GUID: &str = "0c5f1a7e-3d92-4b6a-a8e1-6f2d9c4b0e57";

/// How long a button message without a number holds its button down, like a short press.
pub const OSC_PULSE_MS: u64 = 100;

/// Where `send-osc` actions send unless set otherwise: Bitfocus Companion on this computer.
pub const OSC_SEND_HOST: &str = "127.0.0.1";
pub const OSC_SEND_PORT: u16 = 12321;

/// What an OSC input acts as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OscKind {
    #[default]
    Button,
    Axis,
}

impl OscKind {
    /// Its input type in bindings and in `<osc-inputs>`.
    pub fn input_type(self) -> &'static str {
        match self {
            OscKind::Button => "button",
            OscKind::Axis => "axis",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "button" => Some(OscKind::Button),
            "axis" => Some(OscKind::Axis),
            _ => None,
        }
    }
}

/// One OSC input (`<input>` in `<osc-inputs>`).
#[derive(Debug, Clone, PartialEq)]
pub struct OscInput {
    /// Its input id, unique, from 1.
    pub id: u32,
    /// The OSC address it answers to, exactly (`/panel/fader1`).
    pub address: String,
    pub kind: OscKind,
    /// The numbers an axis gets at the two ends of its travel (0 and 1 for most panels).
    pub low: f64,
    pub high: f64,
}

impl OscInput {
    pub fn new(id: u32, address: &str, kind: OscKind) -> Self {
        Self {
            id,
            address: address.to_string(),
            kind,
            low: 0.0,
            high: 1.0,
        }
    }

    /// An axis value (-1..1) for the number `n`: `low` is -1, `high` is 1 (either way round).
    pub fn axis_value(&self, n: f64) -> f32 {
        let span = self.high - self.low;
        if !n.is_finite() || span == 0.0 || !span.is_finite() {
            return 0.0;
        }
        ((n - self.low) / span * 2.0 - 1.0).clamp(-1.0, 1.0) as f32
    }
}

/// Whether the number `n` presses a button.
pub fn osc_pressed(n: f64) -> bool {
    n >= 0.5
}

/// An OSC address as kept: trimmed, starting with `/`, without spaces; `None` when nothing is
/// left.
pub fn osc_address(text: &str) -> Option<String> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let text = text.trim_start_matches('/');
    (!text.is_empty()).then(|| format!("/{text}"))
}

/// Whether a binding belongs to an OSC input (padded hand-edited values too).
pub fn is_osc_binding(device_id: &str) -> bool {
    device_id.trim().eq_ignore_ascii_case(OSC_GUID)
}

impl Profile {
    pub fn osc_input(&self, id: u32) -> Option<&OscInput> {
        self.osc_inputs.iter().find(|i| i.id == id)
    }

    /// The input answering to `address`.
    pub fn osc_input_at(&self, address: &str) -> Option<&OscInput> {
        self.osc_inputs.iter().find(|i| i.address == address)
    }

    /// The id the next new input gets: the lowest free one.
    pub fn next_osc_input_id(&self) -> u32 {
        (1..).find(|n| self.osc_input(*n).is_none()).unwrap_or(1)
    }

    /// Adds an input for `address` (made tidy by [`osc_address`]) acting as `kind`, or finds the
    /// one that has the address already; returns its id. `None` for an empty address.
    pub fn add_osc_input(&mut self, address: &str, kind: OscKind) -> Option<u32> {
        let address = osc_address(address)?;
        if let Some(i) = self.osc_input_at(&address) {
            return Some(i.id);
        }
        let id = self.next_osc_input_id();
        self.osc_inputs.push(OscInput::new(id, &address, kind));
        self.add_section(Section::OscInputs);
        Some(id)
    }

    /// Gives an input another address; false when it is empty, another input's, or there is no
    /// such input.
    pub fn set_osc_address(&mut self, id: u32, address: &str) -> bool {
        let Some(address) = osc_address(address) else {
            return false;
        };
        if self.osc_input_at(&address).is_some_and(|i| i.id != id) {
            return false;
        }
        match self.osc_inputs.iter_mut().find(|i| i.id == id) {
            Some(i) => {
                i.address = address;
                true
            }
            None => false,
        }
    }

    /// Makes an input act as `kind`; its bindings (every mode) go along with it.
    pub fn set_osc_kind(&mut self, id: u32, kind: OscKind) -> bool {
        let Some(input) = self.osc_inputs.iter_mut().find(|i| i.id == id) else {
            return false;
        };
        let old = input.kind;
        input.kind = kind;
        if old != kind {
            let id = id.to_string();
            for b in self.inputs.iter_mut().filter(|b| {
                is_osc_binding(&b.device_id)
                    && b.input_type.trim() == old.input_type()
                    && b.input_id.trim() == id
            }) {
                b.input_type = kind.input_type().to_string();
            }
        }
        true
    }

    /// Sets the numbers at the ends of an axis input's travel (finite, not equal).
    pub fn set_osc_range(&mut self, id: u32, low: f64, high: f64) -> bool {
        if !low.is_finite() || !high.is_finite() || low == high {
            return false;
        }
        match self.osc_inputs.iter_mut().find(|i| i.id == id) {
            Some(i) => {
                i.low = low;
                i.high = high;
                true
            }
            None => false,
        }
    }

    /// Removes an input and its bindings (with the actions only they used).
    pub fn remove_osc_input(&mut self, id: u32) {
        self.osc_inputs.retain(|i| i.id != id);
        let id = id.to_string();
        self.inputs
            .retain(|b| !(is_osc_binding(&b.device_id) && b.input_id.trim() == id));
        self.collect_garbage();
    }
}

/// Action type that sends an OSC message.
pub const SEND_OSC: &str = "send-osc";

/// What an argument of a `send-osc` message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OscArgKind {
    #[default]
    Int,
    Float,
    Text,
    True,
    False,
    /// The value of the input that runs the action: an axis 0..1, a button 1 / 0.
    Input,
}

impl OscArgKind {
    pub const ALL: [OscArgKind; 6] = [
        Self::Int,
        Self::Float,
        Self::Text,
        Self::True,
        Self::False,
        Self::Input,
    ];

    /// Its name in profiles.
    pub fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Text => "string",
            Self::True => "true",
            Self::False => "false",
            Self::Input => "input",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == text.trim())
    }

    /// Whether it has a value of its own to type in.
    pub fn has_value(self) -> bool {
        matches!(self, Self::Int | Self::Float | Self::Text)
    }
}

/// One argument of a `send-osc` message (`<argument>` with its `type` and `value`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OscArg {
    pub kind: OscArgKind,
    /// As typed: a number for int and float (read when sent), any text for string.
    pub value: String,
}

impl OscArg {
    pub fn new(kind: OscArgKind, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

/// The `<argument>`s of a send-osc action, in order; unknown types count as text.
pub(crate) fn send_osc_args(items: &[crate::Item]) -> Vec<OscArg> {
    items
        .iter()
        .filter_map(|i| match i {
            crate::Item::Group { tag, items, .. } if tag == "argument" => {
                let get = |name: &str| {
                    items.iter().find_map(|i| match i {
                        crate::Item::Property(p) if p.name == name => Some(p.value.as_str()),
                        _ => None,
                    })
                };
                Some(OscArg {
                    kind: get("type")
                        .and_then(OscArgKind::parse)
                        .unwrap_or(OscArgKind::Text),
                    value: get("value").unwrap_or_default().to_string(),
                })
            }
            _ => None,
        })
        .collect()
}

impl Profile {
    /// Replaces the arguments of send-osc action `id`.
    pub fn set_osc_args(&mut self, id: &str, args: &[OscArg]) {
        let Some(a) = self.action_mut(id) else {
            return;
        };
        let at = a
            .items
            .iter()
            .position(|i| matches!(i, crate::Item::Group { tag, .. } if tag == "argument"))
            .unwrap_or_else(|| {
                a.items
                    .iter()
                    .position(|i| matches!(i, crate::Item::Property(p) if p.name == "action-label" || p.name == "activation-mode"))
                    .unwrap_or(a.items.len())
            });
        a.items
            .retain(|i| !matches!(i, crate::Item::Group { tag, .. } if tag == "argument"));
        let at = at.min(a.items.len());
        let groups = args.iter().map(|arg| crate::Item::Group {
            tag: "argument".into(),
            attrs: Vec::new(),
            items: vec![
                crate::Item::Property(crate::Property {
                    ty: "string".into(),
                    name: "type".into(),
                    value: arg.kind.name().into(),
                }),
                crate::Item::Property(crate::Property {
                    ty: "string".into(),
                    name: "value".into(),
                    value: arg.value.clone(),
                }),
            ],
        });
        a.items.splice(at..at, groups);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_tidied() {
        assert_eq!(osc_address(" fader 1 ").as_deref(), Some("/fader1"));
        assert_eq!(osc_address("//a/b").as_deref(), Some("/a/b"));
        assert_eq!(osc_address(" / "), None);
    }

    #[test]
    fn axis_values_follow_the_range() {
        let mut i = OscInput::new(1, "/f", OscKind::Axis);
        assert_eq!(i.axis_value(0.0), -1.0);
        assert_eq!(i.axis_value(0.5), 0.0);
        assert_eq!(i.axis_value(1.0), 1.0);
        assert_eq!(i.axis_value(7.0), 1.0, "clamped");
        (i.low, i.high) = (127.0, 0.0);
        assert_eq!(i.axis_value(127.0), -1.0, "either way round");
        assert_eq!(i.axis_value(f64::NAN), 0.0);
        assert!(osc_pressed(1.0) && !osc_pressed(0.0) && osc_pressed(0.5));
    }

    #[test]
    fn inputs_are_found_by_address_and_removed_with_their_bindings() {
        let mut p = Profile::default();
        assert_eq!(p.add_osc_input("  ", OscKind::Button), None);
        let fire = p.add_osc_input("/deck/fire", OscKind::Button).unwrap();
        assert_eq!(p.add_osc_input("deck/fire", OscKind::Axis), Some(fire));
        let fader = p.add_osc_input("/fader", OscKind::Axis).unwrap();
        assert_ne!(fire, fader);
        assert!(!p.set_osc_address(fader, "/deck/fire"), "taken");
        assert!(p.set_osc_address(fader, "/fader2"));
        assert!(p.set_osc_range(fader, 0.0, 127.0));
        assert!(!p.set_osc_range(fader, 1.0, 1.0));
        p.bind(OSC_GUID, "button", &fire.to_string(), "Default");
        assert!(p.set_osc_kind(fire, OscKind::Axis));
        assert_eq!(p.inputs[0].input_type, "axis", "the binding follows");
        let actions = p.library.len();
        p.remove_osc_input(fire);
        assert!(p.osc_input(fire).is_none());
        assert!(p.inputs.is_empty());
        assert!(p.library.len() < actions);
        assert!(p.order.contains(&Section::OscInputs));
        assert!(p.needs_own_format());
    }

    #[test]
    fn send_osc_arguments_are_kept_in_order() {
        let mut p = Profile::default();
        let root = p.bind(OSC_GUID, "button", "1", "Default");
        p.add_child(
            &root,
            "actions",
            crate::Action::new(&crate::NewAction::SendOsc),
        );
        let id = p.action(&root).unwrap().child_ids()[0].to_string();
        let args = vec![
            OscArg::new(OscArgKind::Text, "Combat"),
            OscArg::new(OscArgKind::Input, ""),
            OscArg::new(OscArgKind::Int, "3"),
        ];
        p.set_osc_args(&id, &args);
        match p.action(&id).unwrap().kind() {
            crate::ActionKind::SendOsc {
                host,
                port,
                address,
                args: got,
            } => {
                assert_eq!((host, port), (OSC_SEND_HOST, OSC_SEND_PORT));
                assert_eq!(address, "/");
                assert_eq!(got, args);
            }
            other => panic!("{other:?}"),
        }
        p.set_osc_args(&id, &args[..1]);
        assert_eq!(send_osc_args(&p.action(&id).unwrap().items).len(), 1);
    }
}
