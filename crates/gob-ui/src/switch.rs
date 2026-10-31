//! On/off switch, used instead of checkboxes.

use egui::Vec2;

use crate::theme::accent;

/// Toggle switch with an optional caption, used instead of checkboxes:
/// `ui.add(switch(&mut on, "caption"))`.
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
                egui::Role::CheckBox,
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
