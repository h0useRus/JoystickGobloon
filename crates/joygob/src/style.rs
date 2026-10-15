//! Look and feel: icon font, accent colors, spacing and rounded corners.

use eframe::egui::{self, Color32, CornerRadius, RichText, Vec2};
pub use egui_phosphor::regular as icon;

use crate::settings::Theme;

pub const ICON_SIZE: f32 = 20.0;

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, theme: Theme) {
    let mut v = match theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    };
    // Goblin green accent, taken from the app icon.
    let (accent, panel, window, faint) = match theme {
        Theme::Light => (
            Color32::from_rgb(0x6f, 0x9e, 0x1e),
            Color32::from_rgb(0xf4, 0xf5, 0xf7),
            Color32::from_rgb(0xff, 0xff, 0xff),
            Color32::from_rgb(0xe8, 0xea, 0xee),
        ),
        Theme::Dark => (
            Color32::from_rgb(0x8c, 0xc8, 0x2c),
            Color32::from_rgb(0x1b, 0x1d, 0x22),
            Color32::from_rgb(0x23, 0x26, 0x2c),
            Color32::from_rgb(0x2a, 0x2d, 0x34),
        ),
    };
    v.selection.bg_fill = accent.gamma_multiply(0.35);
    v.selection.stroke.color = accent;
    v.hyperlink_color = accent;
    v.panel_fill = panel;
    v.window_fill = window;
    v.faint_bg_color = faint;
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(8);
    }
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
    });
}

/// Accent color of the current theme.
pub fn accent(ui: &egui::Ui) -> Color32 {
    ui.visuals().selection.stroke.color
}

/// Large icon-only toggle used in the navigation rail and toolbars.
pub fn icon_toggle(
    ui: &mut egui::Ui,
    selected: bool,
    glyph: &str,
    tooltip: &str,
) -> egui::Response {
    let text = if selected {
        RichText::new(glyph).size(ICON_SIZE + 2.0).color(accent(ui))
    } else {
        RichText::new(glyph).size(ICON_SIZE + 2.0)
    };
    ui.add_sized(
        [44.0, 40.0],
        egui::Button::new(text).selected(selected).frame(selected),
    )
    .on_hover_text(tooltip)
}

/// Icon-only push button with a tooltip.
pub fn icon_button(ui: &mut egui::Ui, enabled: bool, glyph: &str, tooltip: &str) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(glyph).size(ICON_SIZE)).frame(false),
    )
    .on_hover_text(tooltip)
}

/// Horizontal tab with icon and short caption.
pub fn tab(ui: &mut egui::Ui, selected: bool, glyph: &str, caption: &str) -> egui::Response {
    ui.add(
        egui::Button::new(format!("{glyph}  {caption}"))
            .selected(selected)
            .min_size(Vec2::new(0.0, 32.0)),
    )
}
