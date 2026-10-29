//! Modes window: add, rename, re-parent and remove modes (like Gremlin's mode editor).

use std::collections::HashMap;

use eframe::egui;
use gob_core::Profile;
use gob_i18n::I18n;

use crate::style::{self, icon};

pub enum ModeOp {
    Add(String),
    Rename {
        old: String,
        new: String,
    },
    Remove(String),
    SetParent {
        name: String,
        parent: Option<String>,
    },
}

pub fn apply(p: &mut Profile, op: ModeOp) {
    match op {
        ModeOp::Add(name) => {
            p.add_mode(name.trim(), None);
        }
        ModeOp::Rename { old, new } => {
            p.rename_mode(&old, new.trim());
        }
        ModeOp::Remove(name) => {
            p.remove_mode(&name);
        }
        ModeOp::SetParent { name, parent } => {
            p.set_mode_parent(&name, parent.as_deref());
        }
    }
}

/// Narrowest the name field and the parent dropdown get; they grow with the window.
const MIN_FIELD_WIDTH: f32 = 150.0;
/// egui's default text field margin: where the text starts inside it.
const TEXT_INSET: f32 = 4.0;

/// Text being edited per mode, so renames apply on Enter / focus loss only.
#[derive(Default)]
pub struct ModesWindow {
    names: HashMap<String, String>,
    new_name: String,
}

impl ModesWindow {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &I18n,
        profile: &Profile,
        open: &mut bool,
    ) -> Vec<ModeOp> {
        let mut ops = Vec::new();
        let names: Vec<String> = profile.modes.iter().map(|m| m.name.clone()).collect();
        let style = ctx.style();
        let (h, gap) = (style.spacing.interact_size.y, style.spacing.item_spacing.x);
        let min_width = MIN_FIELD_WIDTH * 2.0 + h + gap * 2.0;
        let title = i18n.tr("modes-title");
        let window = egui::Window::new(&title)
            .collapsible(false)
            .resizable([true, false])
            .min_width(min_width)
            .default_width(min_width)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center());
        style::dialog_window(ctx, window, &title, open, |ui| {
            // Invisible grid: two equal columns (name, parent) share the window width,
            // the delete button keeps its size. Not egui's Grid: it limits text fields to
            // last frame's column width, so they would never grow.
            let field_w = ((ui.available_width() - h - gap * 2.0) / 2.0).max(MIN_FIELD_WIDTH);
            let (name_w, parent_w) = (field_w, field_w);

            // Captions are painted after the rows, where the first row's field and
            // dropdown text really starts.
            let (header, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
            let mut columns: Option<[f32; 2]> = None;
            for mode in &profile.modes {
                ui.horizontal(|ui| {
                    let text = self
                        .names
                        .entry(mode.name.clone())
                        .or_insert_with(|| mode.name.clone());
                    let r = ui.add_sized([name_w, h], style::text_field(text));
                    let name_x = r.rect.left() + TEXT_INSET;
                    if r.lost_focus() && *text != mode.name {
                        ops.push(ModeOp::Rename {
                            old: mode.name.clone(),
                            new: text.clone(),
                        });
                        self.names.clear();
                    }
                    let parent = mode.parent.clone().unwrap_or_else(|| "—".into());
                    let combo_x = ui.cursor().left() + ui.spacing().button_padding.x;
                    egui::ComboBox::from_id_salt(("parent", &mode.name))
                        .selected_text(parent)
                        .width(parent_w)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(mode.parent.is_none(), "—").clicked() {
                                ops.push(ModeOp::SetParent {
                                    name: mode.name.clone(),
                                    parent: None,
                                });
                            }
                            for n in names.iter().filter(|n| **n != mode.name) {
                                if ui
                                    .selectable_label(mode.parent.as_ref() == Some(n), n)
                                    .clicked()
                                {
                                    ops.push(ModeOp::SetParent {
                                        name: mode.name.clone(),
                                        parent: Some(n.clone()),
                                    });
                                }
                            }
                        });
                    columns.get_or_insert([name_x, combo_x]);
                    let removable = profile.modes.len() > 1;
                    if style::icon_button(ui, removable, icon::TRASH, &i18n.tr("modes-remove"))
                        .clicked()
                    {
                        ops.push(ModeOp::Remove(mode.name.clone()));
                        self.names.clear();
                    }
                });
            }
            if let Some(xs) = columns {
                let color = ui.visuals().weak_text_color();
                let font = egui::TextStyle::Body.resolve(ui.style());
                for (x, key) in xs.into_iter().zip(["modes-name", "modes-parent"]) {
                    ui.painter().text(
                        egui::pos2(x, header.center().y),
                        egui::Align2::LEFT_CENTER,
                        i18n.tr(key),
                        font.clone(),
                        color,
                    );
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                // Spans both columns; the add button sits in the delete column.
                let edit = style::text_field(&mut self.new_name).hint_text(i18n.tr("modes-new"));
                let r = ui.add_sized([name_w + gap + parent_w, h], edit);
                let valid = !self.new_name.trim().is_empty()
                    && !names.iter().any(|n| n == self.new_name.trim());
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let add = style::icon_button(ui, valid, icon::PLUS_CIRCLE, &i18n.tr("modes-add"))
                    .clicked();
                if valid && (add || enter) {
                    ops.push(ModeOp::Add(std::mem::take(&mut self.new_name)));
                }
            });
        });
        ops
    }
}
