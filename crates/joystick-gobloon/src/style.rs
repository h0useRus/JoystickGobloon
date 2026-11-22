//! Look and feel of Joystick Gobloon: gob-ui's widget kit with the app's palette (goblin green,
//! from the icon), fonts with the system's Chinese and Korean glyphs, the system's light/dark
//! setting, and the few widgets only this app has (output marks, the device eye).

use eframe::egui::{self, Color32, RichText, Vec2};
pub use gob_ui::*;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::settings::Theme;

/// The app's colors, light and dark.
const PALETTES: Palettes = Palettes {
    dark: Palette {
        accent: Color32::from_rgb(0x8c, 0xc8, 0x2c),
        panel: Color32::from_rgb(0x1b, 0x1d, 0x22),
        window: Color32::from_rgb(0x23, 0x26, 0x2c),
        faint: Color32::from_rgb(0x2a, 0x2d, 0x34),
    },
    light: Palette {
        accent: Color32::from_rgb(0x6f, 0x9e, 0x1e),
        panel: Color32::from_rgb(0xf4, 0xf5, 0xf7),
        window: Color32::from_rgb(0xff, 0xff, 0xff),
        faint: Color32::from_rgb(0xe8, 0xea, 0xee),
    },
};

/// Fonts at start: the Chinese and Korean ones only when `cjk` (they are tens of megabytes);
/// otherwise they load the first time such text is about to show ([`want_cjk`]).
pub fn install_fonts(ctx: &egui::Context, cjk: bool) {
    CJK_LOADED.store(cjk, Ordering::Relaxed);
    set_fonts(ctx, cjk);
}

fn set_fonts(ctx: &egui::Context, cjk: bool) {
    let mut fonts = egui::FontDefinitions::default();
    add_icon_font(&mut fonts);
    if cjk {
        add_cjk_fallbacks(&mut fonts);
    }
    ctx.set_fonts(fonts);
}

static CJK_LOADED: AtomicBool = AtomicBool::new(false);
static CJK_WANTED: AtomicBool = AtomicBool::new(false);

/// Whether `text` has Chinese, Japanese or Korean characters.
pub fn has_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
    })
}

/// Chinese or Korean text is about to show (the language list, a profile or device with such
/// names): the fonts load before the next frame.
pub fn want_cjk() {
    CJK_WANTED.store(true, Ordering::Relaxed);
}

/// Notes text that may need the Chinese and Korean fonts.
pub fn note_text(text: &str) {
    if !CJK_LOADED.load(Ordering::Relaxed) && has_cjk(text) {
        want_cjk();
    }
}

/// Loads the Chinese and Korean fonts once something asked for them; cheap every frame.
pub fn load_cjk_if_wanted(ctx: &egui::Context) {
    if CJK_WANTED.load(Ordering::Relaxed) && !CJK_LOADED.swap(true, Ordering::Relaxed) {
        set_fonts(ctx, true);
    }
}

/// Chinese and Korean glyphs from the system's fonts (`platform::cjk_fonts`), after egui's own
/// fonts.
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

/// The app's palette and the theme from the settings; [`Theme::System`] follows the system's
/// light/dark setting, also when it changes while the app runs (see [`follow_system_theme`]).
pub fn apply(ctx: &egui::Context, theme: Theme) {
    let preference = match theme {
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
        Theme::System => egui::ThemePreference::System,
    };
    gob_ui::apply(ctx, &PALETTES, preference);
}

/// Where the window library cannot tell the system's light/dark setting (X11, for one), egui
/// uses its fallback theme: set it from the platform's own reading, light when there is none,
/// and keep it up to date.
pub fn follow_system_theme(ctx: &egui::Context) {
    set_system_fallback(ctx, crate::platform::dark_mode());
    let ctx = ctx.clone();
    crate::platform::watch_dark_mode(move |dark| {
        set_system_fallback(&ctx, dark);
        ctx.request_repaint();
    });
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

/// The rail button of the Game bindings page: a gamepad wired to three contacts, drawn here
/// because the icon font has nothing like it. Sized and coloured like [`nav_toggle`].
pub fn game_nav_toggle(ui: &mut egui::Ui, selected: bool, tooltip: &str) -> egui::Response {
    let response = nav_toggle(ui, selected, " ", tooltip);
    let color = if selected {
        accent(ui)
    } else {
        ui.style().interact(&response).fg_stroke.color
    };
    game_icon(
        ui.painter(),
        response.rect.center(),
        LARGE_ICON_SIZE + 4.0,
        color,
    );
    response
}

/// The Game bindings icon, `size` wide and high around `center`.
fn game_icon(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    use egui::{Pos2, Rect, Stroke};
    // Drawn on a 24 x 24 grid.
    let unit = size / 24.0;
    let at = |x: f32, y: f32| center + Vec2::new(x - 12.0, y - 12.0) * unit;
    let stroke = Stroke::new((1.6 * unit).max(1.2), color);
    // The gamepad, with its d-pad and two buttons.
    painter.rect_stroke(
        Rect::from_min_max(at(2.0, 12.5), at(22.0, 21.5)),
        4.0 * unit,
        stroke,
        egui::StrokeKind::Middle,
    );
    painter.line_segment([at(5.6, 17.0), at(9.4, 17.0)], stroke);
    painter.line_segment([at(7.5, 15.1), at(7.5, 18.9)], stroke);
    painter.circle_filled(at(15.4, 18.0), 1.1 * unit, color);
    painter.circle_filled(at(18.2, 16.0), 1.1 * unit, color);
    // The wires up to their contacts.
    let wire = |points: &[Pos2], contact: Pos2| {
        painter.add(egui::Shape::line(points.to_vec(), stroke));
        painter.circle_stroke(contact, 1.6 * unit, stroke);
    };
    wire(&[at(12.0, 12.5), at(12.0, 6.6)], at(12.0, 5.0));
    wire(
        &[at(8.0, 12.5), at(8.0, 9.8), at(5.0, 9.8), at(5.0, 5.6)],
        at(5.0, 4.0),
    );
    wire(
        &[at(16.0, 12.5), at(16.0, 9.8), at(19.0, 9.8), at(19.0, 5.6)],
        at(19.0, 4.0),
    );
}
