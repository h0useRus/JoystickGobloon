//! Dialogs laid out like WinUI's: [`DIALOG_MARGIN`] around everything, a title
//! ([`dialog_title`]) with [`SPACE_M`] below it, and the buttons ([`dialog_buttons`]) right-aligned
//! [`SPACE_XL`] under the content, [`SPACE_S`] apart. Windows inside the main window
//! ([`dialog_window`]) get a compact title bar like a Windows dialog instead of a title row.

use egui::RichText;

use crate::tokens::{DIALOG_MARGIN, ICON_SIZE, SPACE_L, SPACE_M, SPACE_S, SPACE_XL, SPACE_XS};

/// A modal dialog with [`DIALOG_MARGIN`] around its content.
pub fn modal(ctx: &egui::Context, id: &str) -> egui::Modal {
    let frame = egui::Frame::popup(&ctx.style()).inner_margin(DIALOG_MARGIN);
    egui::Modal::new(egui::Id::new(id)).frame(frame)
}

/// Height of [`dialog_window`]'s title bar, like a Windows dialog's caption.
pub const TITLE_BAR_HEIGHT: f32 = 32.0;

/// Shows `window` (an `egui::Window` with the caller's size and position options) laid out like a
/// Windows dialog: a compact title bar with `title` on the left and a close button on the right,
/// then the content with [`DIALOG_MARGIN`] at the sides and bottom. Dragging the window's
/// background moves it. The close button sets `open` to false; nothing shows while it is false.
pub fn dialog_window<R>(
    ctx: &egui::Context,
    window: egui::Window<'_>,
    title: &str,
    open: &mut bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    if !*open {
        return None;
    }
    let frame = egui::Frame::window(&ctx.style()).inner_margin(0);
    let mut close = false;
    let shown = window.title_bar(false).frame(frame).show(ctx, |ui| {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let color = ui.visuals().text_color();
        let galley = ui.painter().layout_no_wrap(title.to_string(), font, color);
        // The bar is painted after the content, across the width the window then has; an
        // auto-sized window would grow to the whole screen if the bar filled the available width.
        let min_width = SPACE_M + galley.size().x + SPACE_L + TITLE_BAR_HEIGHT;
        let (bar, _) = ui.allocate_exact_size(
            egui::vec2(min_width, TITLE_BAR_HEIGHT),
            egui::Sense::hover(),
        );
        let margin = egui::Margin {
            left: DIALOG_MARGIN,
            right: DIALOG_MARGIN,
            top: SPACE_XS as i8,
            bottom: DIALOG_MARGIN,
        };
        let inner = egui::Frame::new()
            .inner_margin(margin)
            .show(ui, add_contents)
            .inner;
        let bar = egui::Rect::from_min_size(
            bar.min,
            egui::vec2(ui.min_rect().width().max(min_width), TITLE_BAR_HEIGHT),
        );
        let text_pos = egui::pos2(bar.left() + SPACE_M, bar.center().y - galley.size().y / 2.0);
        ui.painter().galley(text_pos, galley, color);
        let button = egui::Rect::from_min_max(
            egui::pos2(bar.right() - TITLE_BAR_HEIGHT - SPACE_S, bar.top()),
            bar.right_bottom(),
        );
        if close_button(ui, button).clicked() {
            close = true;
        }
        inner
    });
    if close {
        *open = false;
    }
    shown.and_then(|r| r.inner)
}

/// A title bar's close button in `rect`: a plain X that turns red on hover, like Windows.
fn close_button(ui: &mut egui::Ui, rect: egui::Rect) -> egui::Response {
    let r = ui.interact(rect, ui.id().with("dialog-close"), egui::Sense::click());
    let painter = ui.painter();
    let mut color = ui.visuals().text_color();
    if r.hovered() {
        // The window's top right corner (theme: 12).
        let corner = egui::CornerRadius {
            ne: 12,
            ..Default::default()
        };
        painter.rect_filled(rect, corner, egui::Color32::from_rgb(196, 43, 28));
        color = egui::Color32::WHITE;
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        crate::icon::X,
        egui::FontId::proportional(ICON_SIZE - 2.0),
        color,
    );
    r
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
