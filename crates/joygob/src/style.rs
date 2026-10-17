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
    painter.circle_stroke(
        c,
        h * 0.32,
        egui::Stroke::new(1.5_f32, visuals.fg_stroke.color),
    );
    painter.circle_filled(c, h * 0.18, red);
    response.on_hover_text(tooltip)
}

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
        .min_size(Vec2::new(0.0, 32.0));
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
            .min_size(Vec2::new(0.0, 32.0)),
    )
}

/// Toggle switch with an optional caption, used instead of checkboxes:
/// `ui.add(style::switch(&mut on, "caption"))`.
pub fn switch<'a>(on: &'a mut bool, text: impl Into<egui::WidgetText>) -> impl egui::Widget + 'a {
    let text = text.into();
    move |ui: &mut egui::Ui| {
        let h = ui.spacing().interact_size.y * 0.8;
        let track = Vec2::new(h * 1.8, h);
        let gap = ui.spacing().icon_spacing * 2.0;
        let galley = (!text.is_empty()).then(|| {
            let wrap = ui.available_width() - track.x - gap;
            text.into_galley(ui, None, wrap, egui::TextStyle::Button)
        });
        let width = track.x + galley.as_ref().map_or(0.0, |g| gap + g.size().x);
        let height = galley
            .as_ref()
            .map_or(h, |g| g.size().y.max(h))
            .max(ui.spacing().interact_size.y);
        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        let checked = *on;
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                ui.is_enabled(),
                checked,
                galley.as_ref().map_or("", |g| g.text()),
            )
        });
        if ui.is_rect_visible(rect) {
            let t = ui.ctx().animate_bool_responsive(response.id, checked);
            let visuals = ui.style().interact(&response);
            let track_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.center().y - h / 2.0),
                track,
            );
            let radius = 0.5 * h;
            let fill = if checked {
                accent(ui).gamma_multiply(if ui.is_enabled() { 1.0 } else { 0.5 })
            } else {
                visuals.bg_fill
            };
            let painter = ui.painter();
            painter.rect(
                track_rect,
                radius,
                fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
            let x = egui::lerp(
                (track_rect.left() + radius)..=(track_rect.right() - radius),
                t,
            );
            let knob = if checked {
                ui.visuals().extreme_bg_color
            } else {
                visuals.fg_stroke.color
            };
            painter.circle_filled(egui::pos2(x, track_rect.center().y), radius * 0.7, knob);
            if let Some(galley) = galley {
                let pos = egui::pos2(
                    track_rect.right() + gap,
                    rect.center().y - galley.size().y / 2.0,
                );
                painter.galley(pos, galley, visuals.text_color());
            }
        }
        response
    }
}
