//! The picture with a numbered tag per control. An input chip dragged near a control shows the
//! control's parts around its tag: dropped on a part it goes there, dropped on the tag it goes
//! where it fits best, dropped on an empty spot it makes a new control there. Tags are dragged
//! to move their control and clicked to select it.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use gob_devices::{Control, Input, Kind};
use gob_ui as style;
use style::icon;

use crate::app::MapApp;
use crate::dialogs::NewControl;

/// Tag radius, in screen points.
const TAG: f32 = 11.0;
/// How near a dragged chip has to come to a tag to open its parts.
const REACH: f32 = 56.0;
/// Distance of the part slots from their tag, and their radius.
const SLOT_GAP: f32 = 32.0;
const SLOT: f32 = 12.0;

/// How a control's inputs are filled in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Filled {
    None,
    Some,
    All,
}

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
        let controls: Vec<(usize, Control)> = plugin
            .manifest
            .controls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.view == self.view)
            .map(|(n, c)| (n + 1, c.clone()))
            .collect();
        let inputs = plugin.defaults.clone();

        let area = ui.available_rect_before_wrap();
        let scale = (area.width() / size.x).min(area.height() / size.y).min(3.0);
        let picture = Rect::from_center_size(area.center(), size * scale);
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
        let chip = egui::DragAndDrop::payload::<Input>(&ctx).map(|i| *i);
        let released = ui.input(|i| i.pointer.any_released());

        // The control under the pointer: within reach of a dragged chip, on the tag otherwise.
        let reach = if chip.is_some() { REACH } else { TAG + 3.0 };
        let near_at = |p: Pos2| {
            controls
                .iter()
                .map(|(n, c)| (*n, c, to_screen(spot(c)).distance(p)))
                .filter(|(_, _, d)| *d <= reach)
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(n, c, _)| (n, c.clone()))
        };
        let near = pointer.and_then(near_at);

        let accent = style::accent(ui);
        let visuals = ui.visuals().clone();
        for (n, c) in &controls {
            let at = to_screen(spot(c));
            let parts = c.parts();
            let mapped = parts
                .iter()
                .filter(|p| inputs.get(&c.id, p).is_some())
                .count();
            let filled = match mapped {
                0 => Filled::None,
                m if m == parts.len() => Filled::All,
                _ => Filled::Some,
            };
            let lit = parts
                .iter()
                .filter_map(|p| inputs.get(&c.id, p))
                .any(|i| self.is_lit(i));
            let selected = self.selected.as_deref() == Some(c.id.as_str());
            let hovered = near.as_ref().is_some_and(|(m, _)| m == n);
            draw_tag(
                &painter,
                &visuals,
                accent,
                at,
                *n,
                filled,
                lit,
                selected || hovered,
            );
        }

        // Dragging a chip: the parts of the control it is near, or where a new control would go.
        if let (Some(input), Some(p)) = (chip, pointer) {
            match &near {
                Some((_, c)) => {
                    let at = to_screen(spot(c));
                    let parts = c.parts();
                    let mut target: Option<&str> = None;
                    if parts.len() > 1 {
                        for (part, offset) in slots(c) {
                            let center = at + offset * SLOT_GAP;
                            let over = center.distance(p) <= SLOT + 2.0;
                            if over {
                                target = Some(part);
                            }
                            let filled = inputs.get(&c.id, part).is_some();
                            let fits = c.kind.is_axis_part(part) == matches!(input, Input::Axis(_));
                            draw_slot(
                                &painter,
                                &visuals,
                                accent,
                                center,
                                &slot_glyph(c.kind, part),
                                filled,
                                fits,
                                over,
                            );
                        }
                    }
                    let caption = match target {
                        Some(part) => format!("{} {}", c.name, self.part_label(part)),
                        None => c.name.clone(),
                    };
                    caption_at(
                        &painter,
                        &visuals,
                        at + Vec2::new(0.0, -SLOT_GAP - 26.0),
                        &caption,
                    );
                    if released {
                        let id = c.id.clone();
                        let done = match target {
                            Some(part) => {
                                self.assign(&id, part, input);
                                true
                            }
                            None => self.assign_best(&id, input),
                        };
                        if !done {
                            self.error = Some(self.i18n.tr("canvas-no-fit"));
                        }
                    }
                }
                None => {
                    painter.circle_stroke(p, TAG, Stroke::new(1.5, accent));
                    painter.text(
                        p,
                        Align2::CENTER_CENTER,
                        icon::PLUS,
                        FontId::proportional(14.0),
                        accent,
                    );
                    caption_at(
                        &painter,
                        &visuals,
                        p + Vec2::new(0.0, -28.0),
                        &self.i18n.tr("canvas-drop-new"),
                    );
                    if released {
                        let at = to_picture(p);
                        self.new_control = Some(NewControl::new(self, at, size.x, Some(input)));
                    }
                }
            }
            return;
        }

        // Moving and selecting tags.
        // A tag is moved from where the button went down: the pointer has left it by now.
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
        response.context_menu(|ui| {
            if ui.button(self.i18n.tr("canvas-add")).clicked()
                && let Some(p) = ui.ctx().input(|i| i.pointer.press_origin())
            {
                self.new_control = Some(NewControl::new(self, to_picture(p), size.x, None));
                ui.close();
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tag(
    painter: &egui::Painter,
    visuals: &egui::Visuals,
    accent: Color32,
    at: Pos2,
    n: usize,
    filled: Filled,
    lit: bool,
    selected: bool,
) {
    let amber = Color32::from_rgb(0xf0, 0xa5, 0x30);
    let (fill, ink) = match filled {
        Filled::All => (accent, Color32::from_rgb(0x10, 0x12, 0x14)),
        Filled::Some => (amber, Color32::from_rgb(0x10, 0x12, 0x14)),
        Filled::None => (Color32::from_rgb(0x50, 0x54, 0x5c), Color32::WHITE),
    };
    if lit {
        painter.circle_filled(at, TAG + 7.0, accent.gamma_multiply(0.45));
    }
    painter.circle_filled(at, TAG, fill);
    let ring = if selected {
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
        ink,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_slot(
    painter: &egui::Painter,
    visuals: &egui::Visuals,
    accent: Color32,
    at: Pos2,
    glyph: &str,
    filled: bool,
    fits: bool,
    over: bool,
) {
    let bg = visuals.extreme_bg_color;
    let fill = if over {
        accent
    } else if filled {
        accent.gamma_multiply(0.35)
    } else {
        bg
    };
    let ink = if over {
        bg
    } else {
        visuals.strong_text_color()
    };
    let stroke = if fits {
        Stroke::new(1.5, accent)
    } else {
        Stroke::new(1.0, visuals.weak_text_color())
    };
    painter.circle_filled(at, SLOT, fill);
    painter.circle_stroke(at, SLOT, stroke);
    painter.text(
        at,
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(12.0),
        ink,
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

/// Where each part's slot sits around its tag (unit offsets): directions where they point,
/// the rest on free sides.
fn slots(c: &Control) -> Vec<(&str, Vec2)> {
    let parts = c.parts();
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let fixed = |part: &str| -> Option<Vec2> {
        Some(match (c.kind, part) {
            (_, "up") | (_, "fwd") => Vec2::new(0.0, -1.0),
            (_, "down") | (_, "back") => Vec2::new(0.0, 1.0),
            (_, "left") | (_, "first") | (_, "ccw") => Vec2::new(-1.0, 0.0),
            (_, "right") | (_, "second") | (_, "cw") | (_, "x") => Vec2::new(1.0, 0.0),
            (Kind::Stick, "y") => Vec2::new(0.0, 1.0),
            (_, "twist") => Vec2::new(-d, -d),
            (_, "push") | (_, "press") => Vec2::new(d, d),
            _ => return None,
        })
    };
    let others = parts.iter().filter(|p| fixed(p).is_none()).count().max(1);
    let mut k = 0;
    parts
        .into_iter()
        .map(|p| {
            let offset = fixed(p).unwrap_or_else(|| {
                let angle =
                    std::f32::consts::TAU * k as f32 / others as f32 - std::f32::consts::FRAC_PI_2;
                k += 1;
                Vec2::angled(angle)
            });
            (p, offset)
        })
        .collect()
}

fn slot_glyph(kind: Kind, part: &str) -> String {
    match (kind, part) {
        (_, "up") => icon::ARROW_UP.into(),
        (_, "down") => icon::ARROW_DOWN.into(),
        (_, "left") => icon::ARROW_LEFT.into(),
        (_, "right") => icon::ARROW_RIGHT.into(),
        (_, "first") => "1".into(),
        (_, "second") => "2".into(),
        (_, "fwd") => icon::CARET_UP.into(),
        (_, "back") => icon::CARET_DOWN.into(),
        (_, "cw") => icon::ARROW_CLOCKWISE.into(),
        (_, "ccw") => icon::ARROW_COUNTER_CLOCKWISE.into(),
        (_, "x") => "X".into(),
        (_, "y") => "Y".into(),
        (_, "twist") => icon::ARROWS_CLOCKWISE.into(),
        (_, "push") | (_, "press") => icon::CIRCLE.into(),
        (_, other) => other
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default(),
    }
}
