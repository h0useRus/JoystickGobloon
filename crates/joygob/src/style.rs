//! Look and feel: icon font, accent colors, spacing and rounded corners.

use eframe::egui::{self, Color32, CornerRadius, RichText, Vec2};
pub use egui_phosphor::regular as icon;

use crate::settings::Theme;

/// Icons inside rows of controls (buttons, captions next to fields).
pub const ICON_SIZE: f32 = 16.0;
const NAV_ICON_SIZE: f32 = 22.0;
/// Corner radius of widgets. Pass it explicitly to selectable buttons:
/// egui draws selected buttons with square corners otherwise.
pub const RADIUS: u8 = 8;

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

/// Installs both palettes and picks one; [`Theme::System`] lets egui follow the
/// Windows light/dark setting, also when it changes while the app runs.
pub fn apply(ctx: &egui::Context, theme: Theme) {
    ctx.set_visuals_of(egui::Theme::Light, visuals(false));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(true));
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
    ctx.set_theme(match theme {
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
        Theme::System => egui::ThemePreference::System,
    });
}

fn visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    // Goblin green accent, taken from the app icon.
    let (accent, panel, window, faint) = if dark {
        (
            Color32::from_rgb(0x8c, 0xc8, 0x2c),
            Color32::from_rgb(0x1b, 0x1d, 0x22),
            Color32::from_rgb(0x23, 0x26, 0x2c),
            Color32::from_rgb(0x2a, 0x2d, 0x34),
        )
    } else {
        (
            Color32::from_rgb(0x6f, 0x9e, 0x1e),
            Color32::from_rgb(0xf4, 0xf5, 0xf7),
            Color32::from_rgb(0xff, 0xff, 0xff),
            Color32::from_rgb(0xe8, 0xea, 0xee),
        )
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
        w.corner_radius = CornerRadius::same(RADIUS);
    }
    v
}

/// Accent color of the current theme.
pub fn accent(ui: &egui::Ui) -> Color32 {
    ui.visuals().selection.stroke.color
}

/// Large icon-only toggle of the navigation rail.
pub fn nav_toggle(ui: &mut egui::Ui, selected: bool, glyph: &str, tooltip: &str) -> egui::Response {
    toggle(
        ui,
        selected,
        glyph,
        tooltip,
        NAV_ICON_SIZE,
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

/// Icons of the main toolbar: larger than the ones in rows of controls.
const TOOLBAR_ICON_SIZE: f32 = 22.0;
/// Main toolbar buttons fill most of its 44 px row.
const TOOLBAR_BUTTON: f32 = 36.0;

/// Main toolbar push button.
pub fn toolbar_button(
    ui: &mut egui::Ui,
    enabled: bool,
    glyph: &str,
    tooltip: &str,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(glyph).size(TOOLBAR_ICON_SIZE))
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
        TOOLBAR_ICON_SIZE,
        Vec2::splat(TOOLBAR_BUTTON),
    )
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
        Color32::from_rgb(0xe5, 0x48, 0x4d)
    };
    painter.circle_stroke(c, h * 0.32, egui::Stroke::new(1.5, visuals.fg_stroke.color));
    painter.circle_filled(c, h * 0.18, red);
    response.on_hover_text(tooltip)
}

/// Tab like [`tab`] with a small info icon inside its right end. Returns the tab's
/// response and whether the info icon (not the tab) was clicked.
pub fn tab_with_info(
    ui: &mut egui::Ui,
    selected: bool,
    glyph: &str,
    caption: &str,
    info_tip: &str,
) -> (egui::Response, bool) {
    // A normal egui Button (same look as [`tab`]) with the icon in its right-hand slot.
    let button = egui::Button::new(format!("{glyph}  {caption}"))
        // The info icon is always in the accent green, selected tab or not.
        .shortcut_text(RichText::new(icon::INFO).size(ICON_SIZE).color(accent(ui)))
        .selected(selected)
        .corner_radius(RADIUS)
        .min_size(Vec2::new(0.0, 32.0));
    let response = ui.add(button);
    // The icon area is registered after the button, so it gets the click there.
    let pad = ui.spacing().button_padding.x;
    let icon_w = ICON_SIZE + 8.0;
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            response.rect.right() - pad - ICON_SIZE / 2.0,
            response.rect.center().y,
        ),
        Vec2::new(icon_w, icon_w),
    );
    let info = ui
        .interact(icon_rect, response.id.with("info"), egui::Sense::click())
        .on_hover_text(info_tip);
    if info.hovered() {
        let hover = ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.5);
        ui.painter().rect_filled(icon_rect, RADIUS, hover);
    }
    (response, info.clicked())
}

/// Horizontal tab with icon and short caption.
pub fn tab(ui: &mut egui::Ui, selected: bool, glyph: &str, caption: &str) -> egui::Response {
    ui.add(
        egui::Button::new(format!("{glyph}  {caption}"))
            .selected(selected)
            .corner_radius(RADIUS)
            .min_size(Vec2::new(0.0, 32.0)),
    )
}
