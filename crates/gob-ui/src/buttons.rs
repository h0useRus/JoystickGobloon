//! Icon buttons and toggles: navigation rail, main toolbar, rows of controls.

use egui::{Color32, RichText, Vec2};

use crate::theme::accent;
use crate::tokens::{ICON_SIZE, LARGE_ICON_SIZE, RADIUS, RED, TOOLBAR_BUTTON};

/// Large icon-only toggle of the navigation rail.
pub fn nav_toggle(ui: &mut egui::Ui, selected: bool, glyph: &str, tooltip: &str) -> egui::Response {
    toggle(
        ui,
        selected,
        glyph,
        tooltip,
        LARGE_ICON_SIZE,
        Vec2::new(44.0, 40.0),
    )
}

/// Icon-only toggle as tall as text fields and dropdowns.
pub fn icon_toggle(
    ui: &mut egui::Ui,
    selected: bool,
    glyph: &str,
    tooltip: &str,
) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    toggle(
        ui,
        selected,
        glyph,
        tooltip,
        ICON_SIZE,
        Vec2::new(h + 4.0, h),
    )
}

fn toggle(
    ui: &mut egui::Ui,
    selected: bool,
    glyph: &str,
    tooltip: &str,
    size: f32,
    button: Vec2,
) -> egui::Response {
    let text = if selected {
        RichText::new(glyph).size(size).color(accent(ui))
    } else {
        RichText::new(glyph).size(size)
    };
    let response = ui.add_sized(
        button,
        egui::Button::new(text)
            .selected(selected)
            .corner_radius(RADIUS)
            .frame(selected),
    );
    response.on_hover_text(tooltip)
}

/// Main toolbar push button.
pub fn toolbar_button(
    ui: &mut egui::Ui,
    enabled: bool,
    glyph: &str,
    tooltip: &str,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(glyph).size(LARGE_ICON_SIZE))
            .frame(false)
            .min_size(Vec2::splat(TOOLBAR_BUTTON)),
    )
    .on_hover_text(tooltip)
}

/// Main toolbar toggle (activate / stop), same size as [`toolbar_button`].
pub fn toolbar_toggle(
    ui: &mut egui::Ui,
    selected: bool,
    glyph: &str,
    tooltip: &str,
) -> egui::Response {
    toggle(
        ui,
        selected,
        glyph,
        tooltip,
        LARGE_ICON_SIZE,
        Vec2::splat(TOOLBAR_BUTTON),
    )
}

/// [`icon_button`] with a colored icon.
pub fn icon_button_colored(
    ui: &mut egui::Ui,
    enabled: bool,
    glyph: &str,
    color: Color32,
    tooltip: &str,
) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(glyph).size(ICON_SIZE).color(color))
            .frame(false)
            .min_size(Vec2::new(h, h)),
    )
    .on_hover_text(tooltip)
}

/// Icon-only push button with a tooltip, as tall as text fields.
pub fn icon_button(ui: &mut egui::Ui, enabled: bool, glyph: &str, tooltip: &str) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(glyph).size(ICON_SIZE))
            .frame(false)
            .min_size(Vec2::new(h, h)),
    )
    .on_hover_text(tooltip)
}

/// Record button: a ring with a red dot in the middle, as tall as text fields.
pub fn record_button(ui: &mut egui::Ui, tooltip: &str) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(h), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, RADIUS, visuals.bg_fill);
    }
    let c = rect.center();
    let red = if response.hovered() {
        Color32::from_rgb(0xff, 0x4d, 0x4f)
    } else {
        RED
    };
    painter.circle_stroke(
        c,
        h * 0.32,
        egui::Stroke::new(1.5_f32, visuals.fg_stroke.color),
    );
    painter.circle_filled(c, h * 0.18, red);
    response.on_hover_text(tooltip)
}
