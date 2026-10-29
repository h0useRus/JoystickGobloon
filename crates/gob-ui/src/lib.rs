//! A small egui widget kit with one look: palette and text sizes ([`theme`]), sizes and spacing
//! on a 4 px grid ([`tokens`]), and the widgets built on them. Changing a token or a widget here
//! changes every control of that kind in the app. Nothing in it knows about any app: colors come
//! in as a [`Palettes`], texts as arguments.
//!
//! Icons are Phosphor's regular set ([`icon`]); [`add_icon_font`] puts it into egui's fonts.

mod buttons;
mod dialog;
mod editable;
mod spin;
mod switch;
mod tabs;
mod text;
pub mod theme;
mod tick;
pub mod tokens;

pub use buttons::*;
pub use dialog::*;
pub use editable::*;
pub use egui_phosphor::regular as icon;
pub use spin::*;
pub use switch::*;
pub use tabs::*;
pub use text::*;
pub use theme::{Palette, Palettes, accent, add_icon_font, apply, set_system_fallback};
pub use tick::*;
pub use tokens::*;
