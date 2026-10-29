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
    add_cjk_fallbacks(&mut fonts);
    ctx.set_fonts(fonts);
}

/// Chinese and Korean glyphs from the system's fonts (`platform::cjk_fonts`), after egui's own
/// fonts. Always loaded: the language list shows 简体中文 and 한국어 whatever the UI language is.
fn add_cjk_fallbacks(fonts: &mut egui::FontDefinitions) {
    for (name, file) in crate::platform::cjk_fonts() {
        let Ok(bytes) = std::fs::read(&file) else {
            log::warn!(
                "font {} not found: {name} text will show as boxes",
                file.display()
            );
            continue;
        };
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.into());
        }
    }
}

/// Installs both palettes and picks one; [`Theme::System`] lets egui follow the
/// system's light/dark setting, also when it changes while the app runs (see
/// [`follow_system_theme`]).
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

/// Where the window library cannot tell the system's light/dark setting (X11, for one), egui
/// uses its fallback theme: set it from the platform's own reading, light when there is none,
/// and keep it up to date.
pub fn follow_system_theme(ctx: &egui::Context) {
    fn set(ctx: &egui::Context, dark: Option<bool>) {
        let theme = if dark == Some(true) {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.options_mut(|o| o.fallback_theme = theme);
    }
    set(ctx, crate::platform::dark_mode());
    let ctx = ctx.clone();
    crate::platform::watch_dark_mode(move |dark| {
        set(&ctx, dark);
        ctx.request_repaint();
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

/// How the loaded profile uses a virtual output (see `output_marks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMark {
    Free,
    Used,
    /// Written by several inputs that run together.
    Conflict,
}

/// Fill and ink (text and outline) of a used output: blue once, amber in a conflict. Green
/// stays for "pressed", and blue keeps apart from it with red-green color blindness too.
pub fn mark_colors(ui: &egui::Ui, mark: OutputMark) -> Option<(Color32, Color32)> {
    let dark = ui.visuals().dark_mode;
    let rgb = Color32::from_rgb;
    match (mark, dark) {
        (OutputMark::Free, _) => None,
        (OutputMark::Used, true) => Some((rgb(0x1f, 0x3a, 0x5c), rgb(0x5a, 0xa9, 0xff))),
        (OutputMark::Used, false) => Some((rgb(0xdb, 0xe9, 0xfb), rgb(0x2f, 0x6f, 0xd0))),
        (OutputMark::Conflict, true) => Some((rgb(0x4a, 0x35, 0x10), rgb(0xf0, 0xa5, 0x30))),
        (OutputMark::Conflict, false) => Some((rgb(0xfd, 0xf0, 0xd5), rgb(0xa0, 0x62, 0x10))),
    }
}

/// Output button of a virtual device, colored by `mark`; `count` above 1 is drawn in its
/// top right corner, so the mark does not rely on color alone. Selected (pressed) buttons
/// keep the accent fill and only the outline of the mark.
pub fn output_button(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    size: Vec2,
    selected: bool,
    mark: OutputMark,
    count: usize,
) -> egui::Response {
    let colors = mark_colors(ui, mark);
    let mut text = RichText::new(label);
    if let (Some((_, ink)), false) = (colors, selected) {
        text = text.color(ink);
    }
    let mut button = egui::Button::new(text)
        .selected(selected)
        .corner_radius(RADIUS);
    if let Some((fill, ink)) = colors {
        button = button.stroke(egui::Stroke::new(1.0_f32, ink));
        if !selected {
            button = button.fill(fill);
        }
    }
    let r = ui.add_sized(size, button);
    if let (Some((_, ink)), true) = (colors, count > 1) {
        ui.painter().text(
            r.rect.right_top() + Vec2::new(-3.0, 1.0),
            egui::Align2::RIGHT_TOP,
            count.to_string(),
            egui::FontId::proportional(9.0),
            ink,
        );
    }
    r
}

/// Tick box in `color` (box, tick and caption), for legends whose entries are colors; plain
/// on/off options use [`switch`].
pub fn tick(ui: &mut egui::Ui, on: &mut bool, caption: &str, color: Color32) -> egui::Response {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(caption.to_string(), font, color);
    let side = 14.0;
    let gap = 6.0;
    let size = Vec2::new(side + gap + galley.size().x, side.max(galley.size().y));
    let (rect, mut r) = ui.allocate_exact_size(size, egui::Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let boxed = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - side / 2.0),
        Vec2::splat(side),
    );
    let painter = ui.painter();
    let stroke = egui::Stroke::new(if r.hovered() { 2.0_f32 } else { 1.5 }, color);
    painter.rect(
        boxed,
        3.0,
        Color32::TRANSPARENT,
        stroke,
        egui::StrokeKind::Inside,
    );
    if *on {
        painter.rect_filled(boxed.shrink(1.0), 2.0, color);
        let p = |x: f32, y: f32| boxed.min + Vec2::new(x, y) * side;
        let ink = egui::Stroke::new(2.0_f32, ui.visuals().panel_fill);
        painter.line(vec![p(0.22, 0.52), p(0.42, 0.72), p(0.78, 0.3)], ink);
    }
    let text_pos = egui::pos2(boxed.right() + gap, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_pos, galley, color);
    r
}

/// Red of hidden devices and the record dot.
pub const RED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);

/// Device eye: closed and red when hidden by HidHide, open and green when visible; gray
/// while device hiding is off as a whole (`active` false).
pub fn eye(ui: &egui::Ui, hidden: bool, active: bool) -> (&'static str, Color32) {
    let glyph = if hidden { icon::EYE_CLOSED } else { icon::EYE };
    let color = match (active, hidden) {
        (false, _) => ui.visuals().weak_text_color(),
        (true, true) => RED,
        (true, false) => accent(ui),
    };
    (glyph, color)
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

/// Slider with a [`Spin`] for its value, instead of egui's drag box; both move in `step`s.
pub fn slider<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    step: f64,
    suffix: &str,
) -> egui::Response {
    ui.horizontal(|ui| {
        let slider = egui::Slider::new(value, range.clone())
            .step_by(step)
            .show_value(false);
        let r = ui.add(slider);
        r | ui.add(Spin::new(value).range(range).step(step).suffix(suffix))
    })
    .inner
}

/// Numeric up-down for every number field: a text field with up/down arrows inside its right
/// edge, as tall as other fields. The mouse wheel over it steps the value (Shift: 10 steps), as
/// do the arrows and Up/Down while typing; typed values outside the range are clamped when the
/// field loses focus. Built like `egui::DragValue`: `Spin::new(&mut v).range(..).step(..)`.
pub struct Spin<'a, T> {
    value: &'a mut T,
    min: f64,
    max: f64,
    step: Option<f64>,
    decimals: Option<usize>,
    prefix: String,
    suffix: String,
    width: Option<f32>,
}

impl<'a, T: egui::emath::Numeric> Spin<'a, T> {
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value,
            min: T::MIN.to_f64(),
            max: T::MAX.to_f64(),
            step: None,
            decimals: None,
            prefix: String::new(),
            suffix: String::new(),
            width: None,
        }
    }

    pub fn range(mut self, range: std::ops::RangeInclusive<T>) -> Self {
        self.min = range.start().to_f64();
        self.max = range.end().to_f64();
        self
    }

    /// One arrow click or wheel notch; by default 1, or the last shown decimal.
    pub fn step(mut self, step: impl Into<f64>) -> Self {
        self.step = Some(step.into().abs());
        self
    }

    /// Shown decimals; by default as many as the step has.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    pub fn prefix(mut self, prefix: impl ToString) -> Self {
        self.prefix = prefix.to_string();
        self
    }

    pub fn suffix(mut self, suffix: impl ToString) -> Self {
        self.suffix = suffix.to_string();
        self
    }

    /// Width of the whole field; by default it fits the widest value of the range.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    fn decimals_shown(&self) -> usize {
        self.decimals.unwrap_or_else(|| match self.step {
            _ if T::INTEGRAL => 0,
            // 0.05 -> 2, 0.5 -> 1, 5 -> 0.
            Some(s) if s > 0.0 => (0..6)
                .find(|&d| (s * 10f64.powi(d as i32)).fract().abs() < 1e-9)
                .unwrap_or(6),
            _ => 2,
        })
    }
}

impl<T: egui::emath::Numeric> egui::Widget for Spin<'_, T> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let decimals = self.decimals_shown();
        let step = self.step.unwrap_or(if T::INTEGRAL {
            1.0
        } else {
            10f64.powi(-(decimals as i32))
        });
        let (min, max) = (self.min, self.max);
        let round = |v: f64| {
            let f = 10f64.powi(decimals as i32);
            ((v * f).round() / f).clamp(min, max)
        };
        // Rounded first and + 0.0, so a value just below zero shows as 0.00, not -0.00.
        let shown = |v: f64| {
            let f = 10f64.powi(decimals as i32);
            (v * f).round() / f + 0.0
        };
        let format = |v: f64| format!("{}{:.decimals$}{}", self.prefix, shown(v), self.suffix);
        let current = self.value.to_f64();

        let h = ui.spacing().interact_size.y;
        let arrows_w = 16.0;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let width = self.width.unwrap_or_else(|| {
            let widest = [min, max, current]
                .into_iter()
                .filter(|v| v.is_finite() && v.abs() < 1e9)
                .map(|v| {
                    ui.fonts(|f| {
                        f.layout_no_wrap(format(v), font.clone(), Color32::WHITE)
                            .size()
                            .x
                    })
                })
                .fold(24.0_f32, f32::max);
            widest + 16.0 + arrows_w
        });
        let (rect, area) = ui.allocate_exact_size(Vec2::new(width, h), egui::Sense::hover());
        // Not the area's own id: two widgets with one id lose keyboard focus.
        let id = area.id.with("text");
        // One frame around the digits and the arrows, filled like a button; the text field
        // inside has none. Darker while typing, an outline only on hover and focus.
        let radius = ui.visuals().widgets.inactive.corner_radius;
        let has_focus = ui.memory(|m| m.has_focus(id));
        let hovered = ui.is_enabled() && ui.rect_contains_pointer(rect);
        let widgets = &ui.visuals().widgets;
        let fill = if has_focus {
            ui.visuals().extreme_bg_color
        } else if hovered {
            widgets.hovered.weak_bg_fill
        } else {
            widgets.inactive.weak_bg_fill
        };
        ui.painter().rect_filled(rect, radius, fill);

        // While the field has focus it edits its own text (the number alone); otherwise it
        // shows the value with its prefix and suffix.
        let focused = ui.memory(|m| m.has_focus(id));
        let mut text = focused
            .then(|| ui.data(|d| d.get_temp::<String>(id)))
            .flatten()
            .unwrap_or_else(|| {
                if focused {
                    format!("{:.decimals$}", shown(current))
                } else {
                    format(current)
                }
            });
        let field =
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x - arrows_w, rect.max.y));
        // A child that takes no space in the layout: `ui.put` would move the cursor back to
        // the field's right edge, under the arrows.
        let mut response = ui.new_child(egui::UiBuilder::new().max_rect(field)).add(
            egui::TextEdit::singleline(&mut text)
                .id(id)
                .frame(false)
                .margin(Vec2::new(4.0, 0.0))
                .min_size(field.size())
                .vertical_align(egui::Align::Center)
                .horizontal_align(egui::Align::Center),
        );
        if response.gained_focus() {
            // Edit the bare number.
            text = format!("{:.decimals$}", shown(current));
        }
        let typed = text.trim().replace(',', ".").parse::<f64>().ok();
        let mut value = current;
        if response.changed()
            && let Some(v) = typed.filter(|v| (min..=max).contains(v))
        {
            value = v;
        }
        if response.lost_focus() {
            if let Some(v) = typed {
                value = round(v);
            }
            ui.data_mut(|d| d.remove::<String>(id));
        } else if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text));
        }

        // Steps from the wheel, an arrow or the Up/Down keys, applied at the end.
        let mut steps = 0.0_f64;
        if response.has_focus() {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                steps = 1.0;
            } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                steps = -1.0;
            }
        }
        if ui.rect_contains_pointer(rect) {
            // The wheel belongs to the field, even a disabled one: nothing around it scrolls.
            // egui hands one notch to scroll areas over several frames (`smooth_scroll_delta`),
            // so every frame's share is taken, not just the first.
            let (scroll, shift) = ui.input_mut(|i| {
                let scroll = i.raw_scroll_delta;
                i.raw_scroll_delta = Vec2::ZERO;
                i.smooth_scroll_delta = Vec2::ZERO;
                (scroll, i.modifiers.shift)
            });
            // Shift turns the wheel sideways on some systems: take both directions.
            let scroll = scroll.y + scroll.x;
            if scroll != 0.0 && ui.is_enabled() {
                steps = f64::from(scroll.signum()) * if shift { 10.0 } else { 1.0 };
            }
        }

        let arrows = egui::Rect::from_min_max(egui::pos2(field.max.x, rect.min.y), rect.max);
        let painter = ui.painter_at(rect);
        let stroke = if response.has_focus() {
            Some(ui.visuals().selection.stroke)
        } else if hovered {
            Some(ui.visuals().widgets.hovered.bg_stroke)
        } else {
            None
        };
        if let Some(stroke) = stroke {
            painter.rect_stroke(rect.shrink(0.5), radius, stroke, egui::StrokeKind::Inside);
            painter.vline(arrows.min.x, rect.y_range().shrink(5.0), stroke);
        }
        for (i, glyph) in [icon::CARET_UP, icon::CARET_DOWN].into_iter().enumerate() {
            let up = i == 0;
            let half = if up {
                egui::Rect::from_min_max(arrows.min, egui::pos2(arrows.max.x, arrows.center().y))
            } else {
                egui::Rect::from_min_max(egui::pos2(arrows.min.x, arrows.center().y), arrows.max)
            };
            let enabled = ui.is_enabled() && if up { value < max } else { value > min };
            let sense = if enabled {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let arrow = ui.interact(half, id.with(i), sense);
            let visuals = &ui.visuals().widgets;
            // Faint until the field is hovered, bright under the pointer.
            let color = if !enabled {
                visuals.noninteractive.fg_stroke.color.gamma_multiply(0.3)
            } else if arrow.hovered() {
                ui.visuals().selection.stroke.color
            } else if hovered || response.has_focus() {
                visuals.inactive.fg_stroke.color
            } else {
                visuals.inactive.fg_stroke.color.gamma_multiply(0.5)
            };
            painter.text(
                half.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                egui::FontId::proportional(10.0),
                color,
            );
            if arrow.clicked() {
                steps = if up { 1.0 } else { -1.0 };
            }
        }
        if steps != 0.0 {
            value = round(value + steps * step);
            if response.has_focus() {
                ui.data_mut(|d| d.insert_temp(id, format!("{value:.decimals$}")));
            }
        }
        if value != current {
            *self.value = T::from_f64(value);
            response.mark_changed();
        }
        response
    }
}
