//! Horizontal tabs: an icon and a short caption, optionally with clickable icons inside.

use egui::{Color32, Vec2};

use crate::tokens::{ICON_SIZE, RADIUS, TAB_HEIGHT};

/// A clickable icon inside a tab, see [`tab_with_icons`].
pub struct TabIcon<'a> {
    pub glyph: &'a str,
    /// `None`: the tab's text color.
    pub color: Option<Color32>,
    pub tip: &'a str,
}

/// Tab like [`tab`] with small clickable icons inside its right end, in the given order.
/// Returns the tab's response and the index of the icon clicked (not the tab), if any.
pub fn tab_with_icons(
    ui: &mut egui::Ui,
    selected: bool,
    glyph: &str,
    caption: &str,
    icons: &[TabIcon],
) -> (egui::Response, Option<usize>) {
    // A normal egui Button (same look as [`tab`]) with the icons in its right-hand slot.
    let font = egui::FontId::proportional(ICON_SIZE);
    let text_color = ui.visuals().text_color();
    let gap = "  ";
    let mut job = egui::text::LayoutJob::default();
    for (i, icon) in icons.iter().enumerate() {
        if i > 0 {
            job.append(gap, 0.0, egui::TextFormat::simple(font.clone(), text_color));
        }
        let color = icon.color.unwrap_or(text_color);
        job.append(
            icon.glyph,
            0.0,
            egui::TextFormat::simple(font.clone(), color),
        );
    }
    let button = egui::Button::new(format!("{glyph}  {caption}"))
        .shortcut_text(job)
        .selected(selected)
        .corner_radius(RADIUS)
        .min_size(Vec2::new(0.0, TAB_HEIGHT));
    let response = ui.add(button);
    // The icon areas are registered after the button, so they get the clicks there.
    let width = |text: &str| {
        ui.fonts(|f| {
            f.layout_no_wrap(text.to_owned(), font.clone(), text_color)
                .size()
                .x
        })
    };
    let gap_w = width(gap);
    let mut right = response.rect.right() - ui.spacing().button_padding.x;
    let mut clicked = None;
    for (i, icon) in icons.iter().enumerate().rev() {
        let w = width(icon.glyph);
        let side = ICON_SIZE + 8.0;
        let rect = egui::Rect::from_center_size(
            egui::pos2(right - w / 2.0, response.rect.center().y),
            Vec2::new(side, side),
        );
        let r = ui
            .interact(rect, response.id.with(("icon", i)), egui::Sense::click())
            .on_hover_text(icon.tip);
        if r.hovered() {
            let hover = ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.5);
            ui.painter().rect_filled(rect, RADIUS, hover);
        }
        if r.clicked() {
            clicked = Some(i);
        }
        right -= w + gap_w;
    }
    (response, clicked)
}

/// Horizontal tab with icon and short caption.
pub fn tab(ui: &mut egui::Ui, selected: bool, glyph: &str, caption: &str) -> egui::Response {
    ui.add(
        egui::Button::new(format!("{glyph}  {caption}"))
            .selected(selected)
            .corner_radius(RADIUS)
            .min_size(Vec2::new(0.0, TAB_HEIGHT)),
    )
}
