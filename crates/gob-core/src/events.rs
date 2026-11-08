//! Profile events (Joystick Gobloon only): the profile starting or stopping and a mode being
//! entered or left act like buttons. Each event is an input of the events device
//! [`EVENTS_GUID`] (input type `event`, input id = [`ProfileEvent::id`]) and is bound to actions
//! like any button, per mode and layer: the engine presses it when the event happens and lets
//! go a moment later.

/// Device id of profile event bindings.
pub const EVENTS_GUID: &str = "48426ea0-1a55-4d4b-879f-334813df154f";

/// Input type of profile event bindings.
pub const EVENT_INPUT_TYPE: &str = "event";

/// Something that happens to the profile, bound like a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProfileEvent {
    /// The profile starts (activated, or switched to for the program in front), in its
    /// starting mode; not when an edit is applied to the running profile.
    Start,
    /// The profile stops (deactivated, another profile takes over, the program quits), in the
    /// mode it is in.
    Stop,
    /// A mode is entered, also the starting mode when the profile starts; runs in that mode.
    Enter,
    /// A mode is left, also when the profile stops; runs in the mode being left.
    Leave,
}

impl ProfileEvent {
    pub const ALL: [ProfileEvent; 4] = [Self::Start, Self::Stop, Self::Enter, Self::Leave];

    /// Its input id in profiles.
    pub const fn id(self) -> u32 {
        match self {
            Self::Start => 1,
            Self::Stop => 2,
            Self::Enter => 3,
            Self::Leave => 4,
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.id() == id)
    }
}

/// Whether a binding belongs to a profile event (padded hand-edited values too).
pub fn is_event_binding(device_id: &str, input_type: &str) -> bool {
    device_id.trim().eq_ignore_ascii_case(EVENTS_GUID) && input_type.trim() == EVENT_INPUT_TYPE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for e in ProfileEvent::ALL {
            assert_eq!(ProfileEvent::from_id(e.id()), Some(e));
        }
        assert_eq!(ProfileEvent::from_id(0), None);
        assert_eq!(ProfileEvent::from_id(5), None);
        assert!(is_event_binding(
            &format!(" {} ", EVENTS_GUID.to_uppercase()),
            " event "
        ));
        assert!(!is_event_binding(EVENTS_GUID, "button"));
    }
}
