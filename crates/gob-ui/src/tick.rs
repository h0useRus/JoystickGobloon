//! Tick box in a color of its own, for legends.

use egui::{Color32, Vec2};

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
