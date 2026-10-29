//! Dialogs laid out like WinUI's: [`DIALOG_MARGIN`] around everything, a title
//! ([`dialog_title`]) with [`SPACE_M`] below it, and the buttons ([`dialog_buttons`]) right-aligned
//! [`SPACE_XL`] under the content, [`SPACE_S`] apart.

use egui::RichText;

use crate::tokens::{DIALOG_MARGIN, ICON_SIZE, SPACE_M, SPACE_S, SPACE_XL};

/// A modal dialog with [`DIALOG_MARGIN`] around its content.
pub fn modal(ctx: &egui::Context, id: &str) -> egui::Modal {
    let frame = egui::Frame::popup(&ctx.style()).inner_margin(DIALOG_MARGIN);
    egui::Modal::new(egui::Id::new(id)).frame(frame)
}

/// Frame of an `egui::Window` (a dialog inside the main window): [`DIALOG_MARGIN`] around
/// its content.
pub fn window_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::window(&ctx.style()).inner_margin(DIALOG_MARGIN)
}

/// A dialog's title row: an optional icon (in its own color, e.g. a warning) and the title as a
/// heading, then [`SPACE_M`] of space.
pub fn dialog_title(ui: &mut egui::Ui, glyph: Option<(&str, Option<egui::Color32>)>, title: &str) {
    ui.horizontal(|ui| {
        if let Some((glyph, color)) = glyph {
            let mut text = RichText::new(glyph).size(ICON_SIZE + 4.0);
            if let Some(color) = color {
                text = text.color(color);
            }
            ui.label(text);
        }
        ui.heading(title);
    });
    ui.add_space(SPACE_M);
}

/// One button of [`dialog_buttons`].
pub struct DialogButton {
    pub text: String,
    pub enabled: bool,
    /// Filled with the accent: what Enter would do.
    pub primary: bool,
    pub tip: Option<String>,
}

impl DialogButton {
    pub fn primary(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: true,
            primary: true,
            tip: None,
        }
    }

    pub fn secondary(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: true,
            primary: false,
            tip: None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn tip(mut self, tip: impl Into<String>) -> Self {
        self.tip = Some(tip.into());
        self
    }
}

/// Smallest width of a dialog button, so short captions ("OK") are not tiny targets.
const BUTTON_MIN_WIDTH: f32 = 96.0;

/// The dialog's buttons, [`SPACE_XL`] under the content, right-aligned in the given order (the
/// primary one first, Cancel last), [`SPACE_S`] apart. Returns the index of the clicked one.
pub fn dialog_buttons(ui: &mut egui::Ui, buttons: &[DialogButton]) -> Option<usize> {
    ui.add_space(SPACE_XL);
    let mut clicked = None;
    // Right-aligned within the width the content already has: an auto-sized window would
    // otherwise grow to the whole screen.
    let row = egui::vec2(ui.min_rect().width(), ui.spacing().interact_size.y + 4.0);
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    ui.allocate_ui_with_layout(row, layout, |ui| {
        ui.spacing_mut().item_spacing.x = SPACE_S;
        // Right to left: the last button goes on the right.
        for (i, b) in buttons.iter().enumerate().rev() {
            let size = egui::vec2(BUTTON_MIN_WIDTH, ui.spacing().interact_size.y + 4.0);
            let button = if b.primary {
                // Accent fill with dark text, like WinUI's accent button.
                let ink = ui.visuals().extreme_bg_color;
                egui::Button::new(RichText::new(&b.text).color(ink)).fill(crate::accent(ui))
            } else {
                egui::Button::new(&b.text)
            }
            .min_size(size);
            let mut r = ui.add_enabled(b.enabled, button);
            if let Some(tip) = &b.tip {
                r = r.on_hover_text(tip);
            }
            if r.clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}
