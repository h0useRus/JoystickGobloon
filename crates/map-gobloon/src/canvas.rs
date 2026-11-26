//! The picture with a numbered tag per placed input, and the inputs waiting to be placed in its
//! top left corner: each is dragged to the spot of the picture where its control is. Placed tags
//! are dragged to move them, clicked to select them, right-clicked to take them off again.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use gob_devices::Control;
use gob_ui as style;
use style::icon;

use crate::app::MapApp;

/// Tag radius, in screen points.
const TAG: f32 = 12.0;
/// Room per waiting number in the corner.
const SLOT: f32 = 44.0;

impl MapApp {
    pub fn canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(plugin) = self.plugin() else {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new(self.i18n.tr("canvas-empty")).weak());
            });
            return;
        };
        let views: Vec<(String, String)> = plugin
            .manifest
            .views
            .iter()
            .map(|v| (v.id.clone(), v.name.clone()))
            .collect();
        if views.len() > 1 {
            ui.horizontal(|ui| {
                for (id, name) in &views {
                    if style::tab(ui, self.view == *id, icon::IMAGE, name).clicked() {
                        self.view = id.clone();
                    }
                }
            });
            ui.add_space(style::SPACE_S);
        }
        let Some((texture, size)) = self.texture(&ctx) else {
            return;
        };
        let Some(plugin) = self.plugin() else {
            return;
        };
        let total = plugin.manifest.controls.len();
        let controls: Vec<(usize, Control)> = plugin
            .manifest
            .controls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.view == self.view)
            .map(|(n, c)| (n + 1, c.clone()))
            .collect();

        let area = ui.available_rect_before_wrap();
        // A row above the picture stays free for the waiting numbers.
        let room = Rect::from_min_max(area.min + Vec2::new(0.0, SLOT + 8.0), area.max);
        let scale = (room.width() / size.x).min(room.height() / size.y).min(3.0);
        let picture = Rect::from_center_size(room.center(), size * scale);
        let response = ui.allocate_rect(area, Sense::click_and_drag());
        self.layout.picture = Some((picture, scale));
        let painter = ui.painter_at(area);
        painter.image(
            texture.id(),
            picture,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        let to_screen = |at: [f32; 2]| picture.min + Vec2::new(at[0], at[1]) * scale;
        let to_picture = |p: Pos2| {
            let v = (p - picture.min) / scale;
            [v.x.clamp(0.0, size.x), v.y.clamp(0.0, size.y)]
        };
        let moving = self.moving.clone();
        let spot = |c: &Control| match &moving {
            Some((id, at)) if *id == c.id => *at,
            _ => c.at,
        };
        let pointer = ctx.pointer_latest_pos().filter(|p| area.contains(*p));
        let near_at = |p: Pos2| {
            controls
                .iter()
                .map(|(n, c)| (*n, c, to_screen(spot(c)).distance(p)))
                .filter(|(_, _, d)| *d <= TAG + 3.0)
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(n, c, _)| (n, c.clone()))
        };
        let near = pointer.and_then(near_at);

        let accent = style::accent(ui);
        let visuals = ui.visuals().clone();
        for (n, c) in &controls {
            let lit = self.inputs_of(c).into_iter().any(|i| self.is_lit(i));
            let selected = self.selected.as_deref() == Some(c.id.as_str());
            let hovered = near.as_ref().is_some_and(|(m, _)| m == n);
            draw_tag(
                &painter,
                &visuals,
                accent,
                to_screen(spot(c)),
                *n,
                lit,
                selected || hovered,
            );
        }

        // The waiting numbers, in the corner; dragged onto the picture to place them.
        self.layout.pending.clear();
        let pending = self.pending.clone();
        let corner = Rect::from_min_size(area.min, Vec2::new(area.width(), SLOT));
        if pending.is_empty() {
            painter.text(
                corner.left_center() + Vec2::new(4.0, 0.0),
                Align2::LEFT_CENTER,
                self.i18n.tr("dock-hint"),
                FontId::proportional(13.0),
                visuals.weak_text_color(),
            );
        }
        let mut dropped = None;
        for (k, input) in pending.iter().enumerate() {
            let n = total + k + 1;
            let home = corner.left_center() + Vec2::new(TAG + 8.0 + k as f32 * SLOT, -6.0);
            let slot = Rect::from_center_size(home + Vec2::new(0.0, 6.0), Vec2::splat(SLOT));
            self.layout.pending.push((*input, slot));
            let label = self.input_label(*input);
            let r = ui
                .interact(
                    slot,
                    egui::Id::unique(("pending", *input)),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text(format!("{label}\n{}", self.i18n.tr("dock-drag")));
            let lit = self.is_lit(*input);
            let at = match (r.dragged(), ctx.pointer_latest_pos()) {
                (true, Some(p)) => p,
                _ => home,
            };
            let on_top = if r.dragged() {
                ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::unique(("pending-drag", *input)),
                ))
            } else {
                painter.clone()
            };
            draw_tag(&on_top, &visuals, accent, at, n, lit, true);
            on_top.text(
                at + Vec2::new(0.0, TAG + 8.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                visuals.strong_text_color(),
            );
            if r.drag_stopped()
                && let Some(p) = pointer
                && picture.contains(p)
            {
                dropped = Some((*input, to_picture(p)));
            }
            if r.secondary_clicked() {
                self.pending.retain(|i| i != input);
            }
        }
        if let Some((input, at)) = dropped {
            self.place(input, at, size.x);
            return;
        }

        // Moving and selecting placed tags; a tag moves from where the button went down.
        let pressed_on = ui.input(|i| i.pointer.press_origin()).and_then(near_at);
        if response.drag_started()
            && let Some((_, c)) = &pressed_on
        {
            self.moving = Some((c.id.clone(), c.at));
            self.selected = Some(c.id.clone());
        }
        if let Some((_, at)) = &mut self.moving
            && let Some(p) = pointer
        {
            *at = to_picture(p);
        }
        if response.drag_stopped()
            && let Some((id, at)) = self.moving.take()
            && let Some(mut c) = self.plugin().and_then(|pl| pl.control(&id)).cloned()
            && c.at != at
        {
            c.at = [at[0].round(), at[1].round()];
            self.put_control(&id, c);
        }
        if response.clicked() {
            self.selected = near.as_ref().map(|(_, c)| c.id.clone());
        }
        if response.secondary_clicked() {
            self.menu = near.as_ref().map(|(_, c)| c.id.clone());
        }
        if let Some((_, c)) = &near
            && self.moving.is_none()
        {
            let at = to_screen(spot(c));
            caption_at(
                &painter,
                &visuals,
                at + Vec2::new(0.0, -TAG - 14.0),
                &c.name,
            );
        }
        if self.menu.is_some() {
            response.context_menu(|ui| {
                if ui.button(self.i18n.tr("tag-remove")).clicked()
                    && let Some(id) = self.menu.take()
                {
                    self.delete_control(&id);
                    ui.close();
                }
            });
        }
    }
}

fn draw_tag(
    painter: &egui::Painter,
    visuals: &egui::Visuals,
    accent: Color32,
    at: Pos2,
    n: usize,
    lit: bool,
    strong: bool,
) {
    if lit {
        painter.circle_filled(at, TAG + 7.0, accent.gamma_multiply(0.45));
    }
    painter.circle_filled(at, TAG, accent);
    let ring = if strong {
        Stroke::new(2.5, visuals.strong_text_color())
    } else {
        Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 200))
    };
    painter.circle_stroke(at, TAG, ring);
    painter.text(
        at,
        Align2::CENTER_CENTER,
        n.to_string(),
        FontId::proportional(12.0),
        Color32::from_rgb(0x10, 0x12, 0x14),
    );
}

/// A caption on a dark plate, centered at `at`.
fn caption_at(painter: &egui::Painter, visuals: &egui::Visuals, at: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        FontId::proportional(13.0),
        visuals.strong_text_color(),
    );
    let rect = Rect::from_center_size(at, galley.size() + Vec2::new(12.0, 6.0));
    painter.rect_filled(rect, 4.0, visuals.extreme_bg_color.gamma_multiply(0.92));
    painter.galley(
        rect.min + Vec2::new(6.0, 3.0),
        galley,
        visuals.strong_text_color(),
    );
}
