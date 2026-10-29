//! Numeric up-down ([`Spin`]) for every number field, and a slider with one.

use egui::{Color32, Vec2};

use crate::icon;

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
