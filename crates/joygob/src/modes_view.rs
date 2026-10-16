//! Modes window: add, rename, re-parent, remove modes and pick the startup mode.

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
    SetStartup(String),
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
        ModeOp::SetStartup(name) => p.set_startup_mode(&name),
    }
}

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
        let startup = profile.startup_mode();
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
                let star_w = h + 4.0;
                let parent_w = 240.0;
                let rest = star_w + parent_w + h + gap * 3.0;
                // Room for 100 characters of a mode name, within the screen.
                let char_w =
                    ui.fonts(|f| f.glyph_width(&egui::TextStyle::Body.resolve(ui.style()), 'n'));
                let name_w = (char_w * 100.0)
                    .min(ctx.screen_rect().width() * 0.9 - rest - 32.0)
                    .max(240.0);
                ui.set_width(name_w + rest);
                let left = egui::Layout::left_to_right(egui::Align::Center);

                ui.horizontal(|ui| {
                    ui.add_space(star_w + gap);
                    ui.allocate_ui_with_layout(egui::vec2(name_w, h), left, |ui| {
                        ui.label(RichText::new(i18n.tr("modes-name")).weak());
                    });
                    ui.allocate_ui_with_layout(egui::vec2(parent_w, h), left, |ui| {
                        ui.label(RichText::new(i18n.tr("modes-parent")).weak());
                    });
                });
                for mode in &profile.modes {
                    ui.horizontal(|ui| {
                        let is_startup = startup.as_deref() == Some(mode.name.as_str());
                        if style::icon_toggle(ui, is_startup, icon::STAR, &i18n.tr("modes-startup"))
                            .clicked()
                        {
                            ops.push(ModeOp::SetStartup(mode.name.clone()));
                        }
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
                    ui.add_space(star_w + gap);
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
