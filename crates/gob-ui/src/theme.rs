//! Palette and text sizes, for light and dark.

use egui::{Color32, CornerRadius, Vec2};

use crate::tokens::RADIUS;

/// The colors an app brings: its accent and three backgrounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub accent: Color32,
    /// Behind panels (the page).
    pub panel: Color32,
    /// Behind windows and dialogs.
    pub window: Color32,
    /// Striped rows and other faint areas.
    pub faint: Color32,
}

/// One [`Palette`] per theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palettes {
    pub light: Palette,
    pub dark: Palette,
}

/// Phosphor's regular icons into `fonts` (after egui's own, before any fallbacks).
pub fn add_icon_font(fonts: &mut egui::FontDefinitions) {
    egui_phosphor::add_to_fonts(fonts, egui_phosphor::Variant::Regular);
}

/// Installs both palettes, the text sizes and spacing, and picks the theme (`System` follows
/// the system's light/dark setting; see [`set_system_fallback`] for where it cannot be read).
pub fn apply(ctx: &egui::Context, palettes: &Palettes, preference: egui::ThemePreference) {
    ctx.set_visuals_of(egui::Theme::Light, visuals(&palettes.light, false));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&palettes.dark, true));
    ctx.all_styles_mut(|s| {
        // egui's sizes + 1 pt; absolute so applying the style again does not grow them.
        use egui::{FontFamily, FontId, TextStyle};
        s.text_styles = [
            (
                TextStyle::Small,
                FontId::new(10.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(13.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
            (
                TextStyle::Heading,
                FontId::new(19.0, FontFamily::Proportional),
            ),
        ]
        .into();
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
    });
    ctx.set_theme(preference);
}

/// The theme egui uses for `System` where the window library cannot tell the system's
/// setting (X11, for one): dark only when `dark` says so, light otherwise.
pub fn set_system_fallback(ctx: &egui::Context, dark: Option<bool>) {
    let theme = if dark == Some(true) {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.options_mut(|o| o.fallback_theme = theme);
}

fn visuals(p: &Palette, dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.selection.bg_fill = p.accent.gamma_multiply(0.35);
    v.selection.stroke.color = p.accent;
    v.hyperlink_color = p.accent;
    v.panel_fill = p.panel;
    v.window_fill = p.window;
    v.faint_bg_color = p.faint;
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(RADIUS);
    }
    v
}

/// Accent color of the current theme.
pub fn accent(ui: &egui::Ui) -> Color32 {
    ui.visuals().selection.stroke.color
}
