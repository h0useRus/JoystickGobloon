//! Profile model shared by the importer, the engine and the GUI.
//!
//! Joystick Gobloon's own profile model. It started from Joystick Gremlin's R14 format, which
//! is still imported, and grows with what Joystick Gobloon needs (e.g. [`modifier`]s). Typed
//! views (see [`action::ActionKind`]) are decoded on demand from the generic property tree.

pub mod action;
pub mod automap;
pub mod button_axis;
pub mod combos;
pub mod condition;
pub mod curve;
pub mod edit;
pub mod events;
pub mod logical;
pub mod modifier;
pub mod profile;
pub mod script;
pub mod two_axis;
pub mod voice;
pub mod xml;

pub use action::{
    Action, ActionKind, ActivationMode, ButtonSend, InputType, Item, Key, LedBlend, LedMode,
    MacroRepeat, MacroStep, MergeOp, MouseAction, MouseButton, Property, SUPPORTED_KINDS, SendMode,
    format_color, parse_color,
};
pub use automap::Layout as InputLayout;
pub use button_axis::{AxisMotion, AxisOutput, BUTTON_AXIS, ButtonAxis, DecreaseInput};
pub use combos::{COMBO_INPUT_TYPE, KeyCombo, MAX_COMBO_KEYS, is_combo_binding};
pub use condition::{Comparator, Condition, Source as ConditionSource};
pub use edit::{NewAction, child_groups, new_uuid};
pub use events::{
    EVENT_INPUT_TYPE, EVENTS_GUID, ProfileEvent, is_builtin_device, is_event_binding,
};
pub use logical::{
    KEYBOARD_GUID, LOGICAL_DEVICE_GUID, LogicalInput, key_from_input_id, key_input_id,
};
pub use modifier::{Layer, Modifier, Selection};
pub use profile::{
    ActionConfig, AxisDirection, Device, InputBinding, Mode, OutputUse, Profile, Section,
    VirtualButton, VirtualDevices, VirtualJoystick, VirtualOutput, same_device_name,
    virtual_output, virtual_outputs,
};
pub use script::{ScriptInstance, ScriptLanguage, ScriptSource, ScriptValue, script_path};
pub use two_axis::{AxisInput, TWO_AXIS_KINDS};
pub use voice::{
    DEFAULT_HOLD as DEFAULT_VOICE_HOLD, VOICE_GUID, VOICE_INPUT_TYPE, VoiceCommand, VoiceInput,
    is_voice_binding,
};
pub use xml::{Element, Node};
