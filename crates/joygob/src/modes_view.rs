//! Modes window: add, rename, re-parent and remove modes (like Gremlin's mode editor).

use std::collections::HashMap;

use eframe::egui::{self, RichText};
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

/// Width of the name field and the parent dropdown.
const FIELD_WIDTH: f32 = 100.0;
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
        egui::Window::new(i18n.tr("modes-title"))
            .open(open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                // Fixed columns instead of a Grid: a Grid limits text fields to last frame's
                // column width, so they never grow.
                let h = ui.spacing().interact_size.y;
                let gap = ui.spacing().item_spacing.x;
                let (name_w, parent_w) = (FIELD_WIDTH, FIELD_WIDTH);
                ui.set_width(name_w + gap + parent_w + gap + h);
                let left = egui::Layout::left_to_right(egui::Align::Center);

                // Captions start where the text inside the field / dropdown starts.
                let header = |ui: &mut egui::Ui, width: f32, inset: f32, key: &str| {
                    ui.allocate_ui_with_layout(egui::vec2(width, h), left, |ui| {
                        ui.add_space(inset);
                        ui.label(RichText::new(i18n.tr(key)).weak());
                    });
                };
                ui.horizontal(|ui| {
                    header(ui, name_w, TEXT_INSET, "modes-name");
                    header(ui, parent_w, ui.spacing().button_padding.x, "modes-parent");
                });
                for mode in &profile.modes {
                    ui.horizontal(|ui| {
                        let text = self
                            .names
                            .entry(mode.name.clone())
                            .or_insert_with(|| mode.name.clone());
                        let r = ui.add_sized([name_w, h], egui::TextEdit::singleline(text));
                        if r.lost_focus() && *text != mode.name {
                            ops.push(ModeOp::Rename {
                                old: mode.name.clone(),
                                new: text.clone(),
                            });
                            self.names.clear();
                        }
                        let parent = mode.parent.clone().unwrap_or_else(|| "—".into());
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
                        let removable = profile.modes.len() > 1;
                        if style::icon_button(ui, removable, icon::TRASH, &i18n.tr("modes-remove"))
                            .clicked()
                        {
                            ops.push(ModeOp::Remove(mode.name.clone()));
                            self.names.clear();
                        }
                    });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let edit = egui::TextEdit::singleline(&mut self.new_name)
                        .hint_text(i18n.tr("modes-new"));
                    let r = ui.add_sized([name_w, h], edit);
                    let valid = !self.new_name.trim().is_empty()
                        && !names.iter().any(|n| n == self.new_name.trim());
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let add =
                        style::icon_button(ui, valid, icon::PLUS_CIRCLE, &i18n.tr("modes-add"))
                            .clicked();
                    if valid && (add || enter) {
                        ops.push(ModeOp::Add(std::mem::take(&mut self.new_name)));
                    }
                });
            });
        ops
    }
}
