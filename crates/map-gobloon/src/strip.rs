//! The strip of inputs: what the connected controller sent, newest first (or every input it
//! has), each a chip to drag onto a control of the picture.

use eframe::egui::{self, FontId, Rect, RichText, Sense, Stroke};
use gob_devices::{Direction, Input};
use gob_ui as style;
use style::icon;

use crate::app::MapApp;

impl MapApp {
    pub fn strip(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let tip = self.i18n.tr("strip-all");
            if style::icon_toggle(ui, self.show_all, icon::SQUARES_FOUR, &tip).clicked() {
                self.show_all = !self.show_all;
            }
            ui.add_space(style::SPACE_S);
            let inputs = self.strip_inputs();
            if inputs.is_empty() {
                let hint = if self.device.is_none() {
                    self.i18n.tr("strip-no-controller")
                } else {
                    self.i18n.tr("strip-hint")
                };
                ui.label(RichText::new(hint).weak());
                return;
            }
            let height = if self.show_all { 150.0 } else { 70.0 };
            let size = egui::vec2(ui.available_width(), 0.0);
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing =
                                egui::vec2(style::SPACE_S, style::SPACE_S);
                            for input in inputs {
                                self.chip(ui, input);
                            }
                        });
                    });
            });
        });
    }

    fn strip_inputs(&self) -> Vec<Input> {
        if !self.show_all {
            return self.recent.clone();
        }
        let Some(state) = self.device_state() else {
            return Vec::new();
        };
        let info = &state.info;
        let mut all: Vec<Input> = info.axes.iter().map(|a| Input::Axis(*a)).collect();
        for n in 1..=info.hats as u8 {
            all.extend(Direction::ALL.map(|d| Input::HatDirection(n, d)));
        }
        all.extend((1..=info.buttons as u16).map(Input::Button));
        all
    }

    /// One input as a chip: lit while worked, its control's name under it when it has one;
    /// dragged onto the picture to put it on a control.
    fn chip(&mut self, ui: &mut egui::Ui, input: Input) {
        let label = self.input_label(input);
        let mapped = self.mapped_to(input);
        let lit = self.is_lit(input);
        let visuals = ui.visuals().clone();
        let accent = style::accent(ui);
        let (fill, ink) = if lit {
            (accent, visuals.extreme_bg_color)
        } else if mapped.is_some() {
            (visuals.faint_bg_color, visuals.weak_text_color())
        } else {
            (
                visuals.widgets.inactive.bg_fill,
                visuals.strong_text_color(),
            )
        };
        let stroke = if mapped.is_some() {
            visuals.widgets.noninteractive.bg_stroke.color
        } else {
            accent
        };
        let painter = ui.painter();
        let title = painter.layout_no_wrap(label, FontId::proportional(13.5), ink);
        let note = mapped.map(|m| painter.layout_no_wrap(m, FontId::proportional(10.0), ink));
        let text = egui::vec2(
            title
                .size()
                .x
                .max(note.as_ref().map_or(0.0, |n| n.size().x)),
            title.size().y + note.as_ref().map_or(0.0, |n| n.size().y),
        );
        let padding = egui::vec2(10.0, 4.0);
        // One fixed-size piece, so the wrapped row breaks before it rather than squeezing it.
        let (rect, response) = ui.allocate_exact_size(text + padding * 2.0, Sense::drag());
        let draw = |painter: &egui::Painter, rect: Rect| {
            painter.rect(
                rect,
                style::RADIUS,
                fill,
                Stroke::new(1.0, stroke),
                egui::StrokeKind::Inside,
            );
            let mut at = rect.min + padding;
            painter.galley(at, title.clone(), ink);
            if let Some(note) = &note {
                at.y += title.size().y;
                painter.galley(at, note.clone(), ink);
            }
        };
        draw(ui.painter(), rect);
        if response.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), input);
        }
        if response.dragged()
            && let Some(p) = ui.ctx().pointer_latest_pos()
        {
            let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique(("chip", input)));
            let moved = Rect::from_center_size(p + egui::vec2(0.0, -rect.height()), rect.size());
            draw(&ui.ctx().layer_painter(layer), moved);
        }
        response
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(self.i18n.tr("strip-drag"));
    }
}
