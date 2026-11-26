//! The picture with a numbered tag per placed input, and the inputs waiting to be placed in its
//! top left corner: each is dragged to the spot of the picture where its control is. Placed tags
//! are dragged to move them, clicked to select them, right-clicked for their menu. Buttons go
//! into hats made of buttons (a button's menu makes one) and axes into other axes (they become a
//! stick): dragged near such a group, it opens a slot per part to drop onto. Axes are square.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use gob_devices::{Control, Input, Kind};
use gob_ui as style;
use style::icon;

use gob_devices::draw::{self as glyphs, HAT_PARTS, TAG};

use crate::app::MapApp;

/// Room per waiting number in the corner.
const SLOT: f32 = 44.0;
/// How near a dragged input has to come to a group (hat of buttons, stick of axes) to open it.
const REACH: f32 = 56.0;
/// Distance of a group's part slots from its tag, and their radius.
const GROUP_GAP: f32 = 32.0;
const GROUP_SLOT: f32 = 12.0;

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
                .filter(|(_, c, d)| {
                    *d <= if c.kind == Kind::Hat {
                        glyphs::RADIUS
                    } else {
                        glyphs::GAUGE.y / 2.0
                    }
                })
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(n, c, _)| (n, c.clone()))
        };
        let near = pointer.and_then(near_at);

        // What is being dragged: a waiting number, or a placed tag with one input. Either can
        // go into a group: a button into a hat of buttons, an axis into another axis or a stick.
        let dragged_tag = self.moving.as_ref().and_then(|(id, _)| {
            let (_, c) = controls.iter().find(|(_, c)| c.id == *id)?;
            match self.inputs_of(c)[..] {
                [input] => Some((id.clone(), input)),
                _ => None,
            }
        });
        let dragged = self
            .pending
            .iter()
            .copied()
            .find(|i| ctx.is_being_dragged(egui::Id::unique(("pending", *i))))
            .or(dragged_tag.as_ref().map(|(_, i)| *i));
        let skip = dragged_tag.as_ref().map(|(id, _)| id.clone());
        // The group the pointer is at while dragging `input`, and the part under it.
        // What each control takes: (control, parts for a button, parts for an axis).
        type Parts = Option<&'static [&'static str]>;
        let groups: Vec<(Control, Parts, Parts)> = controls
            .iter()
            .filter(|(_, c)| Some(&c.id) != skip.as_ref())
            .map(|(_, c)| {
                let button = self.group_parts(c, Input::Button(1));
                (c.clone(), button, self.group_parts(c, Input::Axis(1)))
            })
            .collect();
        let group_at = |p: Pos2, input: Input| -> Option<(Control, Option<&'static str>)> {
            let (group, parts) = groups
                .iter()
                .filter_map(|(c, button, axis)| {
                    let parts = if matches!(input, Input::Axis(_)) {
                        axis
                    } else {
                        button
                    };
                    Some((c, (*parts)?))
                })
                .map(|(c, parts)| (c, parts, to_screen(c.at).distance(p)))
                .filter(|(_, _, d)| *d <= REACH)
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(c, parts, _)| (c, parts))?;
            let center = to_screen(group.at);
            let part = if group.kind == Kind::Hat {
                let push = group.parts().contains(&"push");
                glyphs::hat_part_at(p - center, push).map(|n| HAT_PARTS[n])
            } else {
                parts.iter().copied().find(|part| {
                    (center + slot_offset(part) * GROUP_GAP).distance(p) <= GROUP_SLOT + 3.0
                })
            };
            Some((group.clone(), part))
        };
        let target = pointer
            .zip(dragged)
            .and_then(|(p, input)| Some((group_at(p, input)?, input)));

        let accent = style::accent(ui);
        let visuals = ui.visuals().clone();
        let pal = glyphs::Pal::new(&visuals, accent);
        for (n, c) in &controls {
            let at = to_screen(spot(c));
            let open = target
                .as_ref()
                .filter(|((g, _), _)| g.id == c.id)
                .map(|((_, part), input)| (*part, *input));
            if let Some((part, _)) = open
                && c.kind == Kind::Hat
            {
                self.draw_control(&painter, &pal, c, at, *n, true, part);
                continue;
            }
            if let Some((part, input)) = open {
                let parts = self.group_parts(c, input).unwrap_or(&[]);
                self.draw_slots(&painter, &visuals, accent, c, at, parts, part);
                continue;
            }
            let selected = self.selected.as_deref() == Some(c.id.as_str());
            let hovered = near.as_ref().is_some_and(|(m, _)| m == n);
            self.draw_control(&painter, &pal, c, at, *n, selected || hovered, None);
        }
        if let Some(((group, part), _)) = &target {
            let caption = match part {
                Some(part) => format!("{} {}", group.name, self.part_label(part)),
                None => group.name.clone(),
            };
            let at = to_screen(group.at) + Vec2::new(0.0, -GROUP_GAP - GROUP_SLOT - 16.0);
            glyphs::caption(&painter, &pal, at, &caption);
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
        enum Drop {
            New(Input, [f32; 2]),
            Part(Input, String, &'static str),
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
            let square = matches!(input, Input::Axis(_));
            glyphs::tag(&on_top, &pal, at, &n.to_string(), lit, true, square);
            on_top.text(
                at + Vec2::new(0.0, TAG + 8.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                visuals.strong_text_color(),
            );
            if r.drag_stopped()
                && let Some(p) = pointer
            {
                dropped = match group_at(p, *input) {
                    Some((group, Some(part))) => Some(Drop::Part(*input, group.id, part)),
                    Some((_, None)) => None,
                    None => picture
                        .contains(p)
                        .then(|| Drop::New(*input, to_picture(p))),
                };
            }
            if r.secondary_clicked() {
                self.pending.retain(|i| i != input);
            }
        }
        match dropped {
            Some(Drop::New(input, at)) => return self.place(input, at, size.x),
            Some(Drop::Part(input, group, part)) => return self.place_part(input, &group, part),
            None => {}
        }

        // Moving and selecting placed tags; a tag moves from where the button went down, and a
        // tag with one input dropped on a group's part goes into the group.
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
        {
            let into = pointer
                .zip(dragged_tag.as_ref().filter(|(t, _)| *t == id))
                .and_then(|(p, (_, input))| group_at(p, *input));
            match into {
                Some((group, Some(part))) => self.merge_into(&id, &group.id, part),
                Some((_, None)) => {}
                None => {
                    if let Some(mut c) = self.plugin().and_then(|pl| pl.control(&id)).cloned()
                        && c.at != at
                    {
                        c.at = [at[0].round(), at[1].round()];
                        self.put_control(&id, c);
                    }
                }
            }
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
            glyphs::caption(&painter, &pal, at + Vec2::new(0.0, -TAG - 14.0), &c.name);
        }
        if let Some(id) = self.menu.clone() {
            let control = controls
                .iter()
                .find(|(_, c)| c.id == id)
                .map(|(_, c)| c.clone());
            response.context_menu(|ui| {
                if let Some(c) = &control {
                    if c.kind == Kind::Button && ui.button(self.i18n.tr("tag-make-hat")).clicked() {
                        self.make_hat(&c.id);
                        self.menu = None;
                        ui.close();
                    }
                    if (self.is_button_hat(c) || c.kind == Kind::Stick)
                        && ui.button(self.i18n.tr("tag-split")).clicked()
                    {
                        self.split(&c.id);
                        self.menu = None;
                        ui.close();
                    }
                }
                if ui.button(self.i18n.tr("tag-remove")).clicked() {
                    self.delete_control(&id);
                    self.menu = None;
                    ui.close();
                }
            });
        }
    }

    /// A group opened while something is dragged near it: a big slot per part to drop onto
    /// (filled when it has its input), the one under the pointer highlighted.
    #[allow(clippy::too_many_arguments)]
    fn draw_slots(
        &self,
        painter: &egui::Painter,
        visuals: &egui::Visuals,
        accent: Color32,
        c: &Control,
        at: Pos2,
        parts: &[&str],
        over: Option<&str>,
    ) {
        let inputs = self
            .plugin()
            .map(|p| p.defaults.clone())
            .unwrap_or_default();
        let own = self.inputs_of(c);
        for part in parts {
            let center = at + slot_offset(part) * GROUP_GAP;
            let over = over == Some(*part);
            // A single axis about to become a stick: its axis shows on X.
            let filled = inputs.get(&c.id, part).is_some()
                || (c.kind == Kind::Axis && *part == "x" && !own.is_empty());
            let fill = if over {
                accent
            } else if filled {
                accent.gamma_multiply(0.45)
            } else {
                visuals.extreme_bg_color
            };
            painter.circle_filled(center, GROUP_SLOT, fill);
            painter.circle_stroke(center, GROUP_SLOT, Stroke::new(1.5, accent));
            let color = if over {
                visuals.extreme_bg_color
            } else {
                visuals.strong_text_color()
            };
            painter.text(
                center,
                Align2::CENTER_CENTER,
                slot_glyph(part),
                FontId::proportional(12.0),
                color,
            );
        }
    }

    /// A placed control by its kind: a hat as a compass, stick axes as a ring, one axis as a
    /// gauge (each with its number in a badge), anything else as a numbered tag.
    #[allow(clippy::too_many_arguments)]
    fn draw_control(
        &self,
        painter: &egui::Painter,
        pal: &glyphs::Pal,
        c: &Control,
        at: Pos2,
        n: usize,
        strong: bool,
        over: Option<&str>,
    ) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let live = |i: Input| glyphs::Live {
            lit: self.is_lit(i),
            value: self.axis_value(i),
        };
        glyphs::control(
            painter,
            pal,
            c,
            &plugin.inputs,
            at,
            Some(n),
            strong,
            over,
            &live,
        );
    }

    /// A part's caption: arrows for directions, letters for axes.
    fn part_label(&self, part: &str) -> String {
        match part {
            "push" => self.i18n.tr("part-push"),
            "twist" => self.i18n.tr("part-twist"),
            other => slot_glyph(other).into(),
        }
    }
}

/// Where a group's part sits around its tag (unit offset; push in the middle).
fn slot_offset(part: &str) -> Vec2 {
    let d = std::f32::consts::FRAC_1_SQRT_2;
    match part {
        "up" => Vec2::new(0.0, -1.0),
        "right" | "x" => Vec2::new(1.0, 0.0),
        "down" | "y" => Vec2::new(0.0, 1.0),
        "left" => Vec2::new(-1.0, 0.0),
        "twist" => Vec2::new(-d, -d),
        _ => Vec2::ZERO,
    }
}

fn slot_glyph(part: &str) -> &'static str {
    match part {
        "up" => icon::ARROW_UP,
        "right" => icon::ARROW_RIGHT,
        "down" => icon::ARROW_DOWN,
        "left" => icon::ARROW_LEFT,
        "x" => "X",
        "y" => "Y",
        "twist" => icon::ARROWS_CLOCKWISE,
        _ => icon::CIRCLE,
    }
}
