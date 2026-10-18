//! Profile model shared by the importer, the engine and the GUI.
//!
//! The model mirrors Joystick Gremlin's R14 profile format closely so that
//! profiles round-trip without loss. Typed views (see [`action::ActionKind`])
//! are decoded on demand from the generic property tree.

pub mod action;
pub mod condition;
pub mod curve;
pub mod edit;
pub mod logical;
pub mod profile;
pub mod xml;

pub use action::{
    format_color, parse_color, Action, ActionKind, ActivationMode, InputType, Item, Key, LedMode,
    MacroRepeat, MacroStep, MergeOp, MouseAction, MouseButton, Property, SUPPORTED_KINDS,
};
pub use condition::{Comparator, Condition, Source as ConditionSource};
pub use edit::{child_groups, new_uuid, NewAction};
pub use logical::{
    key_from_input_id, key_input_id, LogicalInput, KEYBOARD_GUID, LOGICAL_DEVICE_GUID,
};
pub use profile::{
    same_device_name, ActionConfig, AxisDirection, Device, InputBinding, Mode, Profile, Section,
    VirtualButton,
};
pub use xml::{Element, Node};
