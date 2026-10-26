//! Profile model shared by the importer, the engine and the GUI.
//!
//! Joystick Gobloon's own profile model. It started from Joystick Gremlin's R14 format, which
//! is still imported, and grows with what Joystick Gobloon needs (e.g. [`modifier`]s). Typed
//! views (see [`action::ActionKind`]) are decoded on demand from the generic property tree.

pub mod action;
pub mod condition;
pub mod curve;
pub mod edit;
pub mod logical;
pub mod modifier;
pub mod profile;
pub mod xml;

pub use action::{
    Action, ActionKind, ActivationMode, InputType, Item, Key, LedBlend, LedMode, MacroRepeat,
    MacroStep, MergeOp, MouseAction, MouseButton, Property, SUPPORTED_KINDS, format_color,
    parse_color,
};
pub use condition::{Comparator, Condition, Source as ConditionSource};
pub use edit::{NewAction, child_groups, new_uuid};
pub use logical::{
    KEYBOARD_GUID, LOGICAL_DEVICE_GUID, LogicalInput, key_from_input_id, key_input_id,
};
pub use modifier::{Layer, Modifier, Selection};
pub use profile::{
    ActionConfig, AxisDirection, Device, InputBinding, Mode, Profile, Section, VirtualButton,
    VirtualDevices, VirtualJoystick, same_device_name,
};
pub use xml::{Element, Node};
