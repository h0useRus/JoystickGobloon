//! Text that edits in place: shown as text until clicked, then a text field with the focus.

use std::hash::Hash;

use egui::{Color32, Vec2};

use crate::text::{text_area, text_field};
use crate::tokens::{RADIUS, TEXT_MARGIN};

/// A title or note that reads as text and turns into a text field when clicked; Enter (single
/// line), Escape or a click elsewhere ends editing. Takes all the width it is given.
/// `changed()` on the response: the text was edited.
///
/// `ui.add(EditableText::new(&mut name, "title"))`, or with `.multiline().panel()` for a note
/// shown on a quiet panel.
pub struct EditableText<'a> {
    text: &'a mut String,
    id: egui::Id,
    hint: Option<String>,
    multiline: bool,
    panel: bool,
    strong: bool,
}

impl<'a> EditableText<'a> {
    /// `id_salt` keeps the editing state apart from other editable texts.
    pub fn new(text: &'a mut String, id_salt: impl Hash) -> Self {
        Self {
            text,
            id: egui::Id::new(("editable-text", id_salt)),
            hint: None,
            multiline: false,
            panel: false,
            strong: false,
        }
    }

    /// Shown (weak) while the text is empty.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Several lines; Enter adds a line instead of ending the edit.
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// Shown on a quiet panel (a note), not as bare text (a title).
    pub fn panel(mut self) -> Self {
        self.panel = true;
        self
    }

    /// In the strong text color, like a heading.
    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }
}

impl egui::Widget for EditableText<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let edit_id = self.id.with("field");
        let editing = ui.data(|d| d.get_temp::<bool>(self.id)).unwrap_or(false);
        let width = ui.available_width();

        if editing {
            let field = if self.multiline {
                text_area(self.text)
            } else {
                text_field(self.text)
            };
            let mut field = field.id(edit_id).desired_width(width);
            if let Some(hint) = &self.hint {
                field = field.hint_text(hint);
            }
            let r = ui.add(field);
            if !r.has_focus() && !r.lost_focus() {
                // The first frame: take the focus with the cursor at the end.
                r.request_focus();
            }
            if r.lost_focus() {
                ui.data_mut(|d| d.remove::<bool>(self.id));
            }
            return r;
        }

        // Shown as text: as tall as the field it turns into, so nothing jumps.
        let visuals = ui.visuals();
        let empty = self.text.is_empty();
        let (shown, color) = match (&self.hint, empty) {
            (Some(hint), true) => (hint.clone(), visuals.weak_text_color()),
            _ if self.strong => (self.text.clone(), visuals.strong_text_color()),
            _ => (self.text.clone(), visuals.text_color()),
        };
        let font = egui::TextStyle::Body.resolve(ui.style());
        let inner = width - TEXT_MARGIN.sum().x;
        // One line ends in "…" when it does not fit; the whole text is then in the tooltip.
        let full_width = ui
            .painter()
            .layout_no_wrap(shown.clone(), font.clone(), color)
            .size()
            .x;
        let cut = !self.multiline && full_width > inner;
        let wrap = if self.multiline {
            egui::TextWrapMode::Wrap
        } else {
            egui::TextWrapMode::Truncate
        };
        let galley = egui::WidgetText::from(egui::RichText::new(shown.clone()).color(color))
            .into_galley(ui, Some(wrap), inner, egui::TextStyle::Body);
        let row = ui.fonts(|f| f.row_height(&egui::TextStyle::Body.resolve(ui.style())));
        let height = galley.size().y.max(row) + TEXT_MARGIN.sum().y;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
        let hovered = response.hovered();
        let fill = match (self.panel, hovered) {
            (_, true) => ui.visuals().widgets.hovered.weak_bg_fill,
            (true, false) => ui.visuals().extreme_bg_color.gamma_multiply(0.6),
            (false, false) => Color32::TRANSPARENT,
        };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS, fill);
        let pos = rect.min + Vec2::new(TEXT_MARGIN.left.into(), TEXT_MARGIN.top.into());
        painter.galley(pos, galley, color);
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        let response = if cut {
            response.on_hover_text(shown)
        } else {
            response
        };
        if response.clicked() {
            ui.data_mut(|d| d.insert_temp(self.id, true));
            ui.ctx().request_repaint();
        }
        response
    }
}
