//! Profile model shared by the importer, the engine and the GUI.
//!
//! The model mirrors Joystick Gremlin's R14 profile format closely so that
//! profiles round-trip without loss. Typed views (see [`action::ActionKind`])
//! are decoded on demand from the generic property tree.

pub mod action;
pub mod profile;
pub mod xml;

pub use action::{Action, ActionKind, ActivationMode, InputType, Item, Property, SUPPORTED_KINDS};
pub use profile::{same_device_name, Device, InputBinding, Mode, Profile, Section};
pub use xml::{Element, Node};
