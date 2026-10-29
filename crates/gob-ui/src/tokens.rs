//! Sizes and spacing shared by every widget. Spacing follows a 4 px grid like WinUI's.

use egui::Color32;

/// Spacing steps: between parts of one control, related controls, groups, sections and around
/// dialogs.
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_S: f32 = 8.0;
pub const SPACE_M: f32 = 12.0;
pub const SPACE_L: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;

/// Icons inside rows of controls (buttons, captions next to fields).
pub const ICON_SIZE: f32 = 16.0;
/// Icons of the navigation rail and the main toolbar.
pub const LARGE_ICON_SIZE: f32 = 22.0;
/// Corner radius of widgets. Pass it explicitly to selectable buttons:
/// egui draws selected buttons with square corners otherwise.
pub const RADIUS: u8 = 8;
/// Main toolbar buttons fill most of its 44 px row.
pub const TOOLBAR_BUTTON: f32 = 36.0;
/// Height of tabs.
pub const TAB_HEIGHT: f32 = 32.0;

/// Space between a text field's border and its text: clear of the rounded corners.
pub const TEXT_MARGIN: egui::Margin = egui::Margin::symmetric(8, 5);
/// Space between a dialog's border and its content, as WinUI dialogs have.
pub const DIALOG_MARGIN: i8 = SPACE_XL as i8;

/// Red of warnings drawn as a color (hidden devices, the record dot).
pub const RED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);
