//! Device plugins: a picture of a game controller and which input each of its controls sends,
//! so the program can show "Trigger" and "Red button" instead of "button 1" and "button 3".
//!
//! A plugin is a folder (`plugins/devices/<id>/` in the source, `devices/` next to the program):
//! `device.gob` names the device, the connected devices it fits and its controls with their
//! hotspots ([`plugin`]); `inputs.gob` is the device's default numbering ([`inputs`]); `views/`
//! holds its pictures ([`picture`]): photos of the device as PNG with a transparent background,
//! or SVG drawings colored by the theme ([`svg`]). Brands and models live only in
//! plugins, never in the program.

pub mod catalog;
pub mod check;
pub mod input;
pub mod inputs;
pub mod picture;
pub mod plugin;
pub mod svg;

pub use catalog::{Catalog, taught_path};
pub use check::{Issue, Level, check};
pub use input::{Direction, Input};
pub use inputs::{Entry, Inputs};
pub use picture::Picture;
pub use plugin::{Control, DevicePlugin, Kind, Manifest, Match, Side, View};
pub use svg::{Palette, themed, view_box};
