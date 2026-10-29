//! Text fields with room around their text.

use crate::tokens::TEXT_MARGIN;

/// Single-line text field with room around its text (egui's own has 4 x 2 px).
pub fn text_field(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text).margin(TEXT_MARGIN)
}

/// Multi-line text field, like [`text_field`].
pub fn text_area(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(text).margin(TEXT_MARGIN)
}
