//! The export dialog: the plugin's options, what a write changes, and the write.

use super::*;

pub(super) struct ExportDialog {
    pub(super) options: BTreeMap<String, OptionValue>,
    pub(super) preview: Option<Result<(Files, WriteResult), String>>,
    pub(super) busy: bool,
}

impl GameView {
    pub(super) fn open_export(&mut self, ctx: &Context) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let options: BTreeMap<String, OptionValue> = plugin
            .options
            .iter()
            .map(|o| (o.key.clone(), o.default.clone()))
            .collect();
        self.export = Some(ExportDialog {
            options,
            preview: None,
            busy: false,
        });
        self.preview(ctx);
    }

    pub(super) fn preview(&mut self, ctx: &Context) {
        let (Some(game), Some(found)) = (self.game_id(), self.found().cloned()) else {
            return;
        };
        let Some(d) = &mut self.export else {
            return;
        };
        d.busy = true;
        d.preview = None;
        let options = d.options.clone();
        let g = ctx.profile.game_bindings(&game);
        let binds = g
            .map(|g| {
                g.binds
                    .iter()
                    .map(|b| Bind {
                        action: b.action.clone(),
                        output: b.output.to_string(),
                        invert: b.invert,
                        default: false,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let devices = device_links(ctx, &game);
        let plugin = self.selected;
        self.send(Request::Preview {
            plugin,
            target: found.target,
            version: found.version,
            ours: our_devices(ctx),
            devices,
            binds,
            cleared: g.map(|g| g.cleared.clone()).unwrap_or_default(),
            profile: ctx.profile_name.to_string(),
            options,
        });
    }

    pub(super) fn export_dialog(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        ctx: &Context,
        _game: &str,
    ) {
        let Some(plugin) = self.plugin().cloned() else {
            return;
        };
        if self.export.is_none() {
            return;
        }
        let mut close = false;
        let mut write = None;
        let mut again = false;
        let title = i18n.trf(
            "game-export-title",
            &[("game", plugin.manifest.name.clone().into())],
        );
        style::modal(ui.ctx(), "game-export").show(ui.ctx(), |ui| {
            let Some(d) = &mut self.export else {
                return;
            };
            ui.set_width(560.0);
            style::dialog_title(ui, Some((icon::EXPORT, None)), &title);
            for o in &plugin.options {
                let value = d.options.entry(o.key.clone()).or_insert(o.default.clone());
                match (o.kind, value) {
                    (OptionKind::Bool, OptionValue::Bool(on)) => {
                        if ui.add(style::switch(on, &o.label)).changed() {
                            again = true;
                        }
                    }
                    (_, OptionValue::Text(text)) => {
                        ui.horizontal(|ui| {
                            ui.label(&o.label);
                            if ui
                                .add(style::text_field(text).desired_width(240.0))
                                .lost_focus()
                            {
                                again = true;
                            }
                        });
                    }
                    (_, OptionValue::Number(n)) => {
                        ui.horizontal(|ui| {
                            ui.label(&o.label);
                            if ui.add(egui::DragValue::new(n)).changed() {
                                again = true;
                            }
                        });
                    }
                    _ => {}
                }
            }
            ui.add_space(style::SPACE_M);
            match &d.preview {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(i18n.tr("game-export-busy"));
                    });
                }
                Some(Err(e)) => {
                    ui.label(
                        RichText::new(
                            i18n.trf("game-export-error", &[("error", e.clone().into())]),
                        )
                        .color(ui.visuals().error_fg_color),
                    );
                }
                Some(Ok((files, result))) => {
                    let catalog = plugin
                        .catalogs
                        .pick(self_version(&self.targets, self.selected, self.target).as_deref())
                        .map(|(c, _)| c.clone())
                        .unwrap_or_default();
                    ui.label(RichText::new(i18n.tr("game-export-changes")).strong());
                    if result.changes.is_empty() {
                        ui.label(RichText::new(i18n.tr("game-export-none")).weak());
                    }
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for c in &result.changes {
                                let (glyph, key) = match c.kind {
                                    ChangeKind::Added => (icon::PLUS, "game-added"),
                                    ChangeKind::Changed => (icon::PENCIL_SIMPLE, "game-changed"),
                                    ChangeKind::Removed => (icon::MINUS, "game-removed"),
                                };
                                let name = catalog
                                    .action(&c.action)
                                    .map_or(c.action.as_str(), |a| a.name.as_str());
                                let detail = match (&c.from, &c.to) {
                                    (Some(f), Some(t)) => format!("{f} → {t}"),
                                    (None, Some(t)) => t.clone(),
                                    (Some(f), None) => f.clone(),
                                    (None, None) => String::new(),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(glyph).on_hover_text(i18n.tr(key));
                                    ui.label(name);
                                    ui.label(RichText::new(detail).weak().monospace());
                                });
                            }
                        });
                    ui.add_space(style::SPACE_S);
                    ui.label(RichText::new(i18n.tr("game-export-files")).strong());
                    let found = self
                        .targets
                        .get(self.selected)
                        .and_then(|t| t.get(self.target));
                    for (key, text) in &result.files {
                        let same =
                            files.get(key).and_then(|f| f.text.as_deref()) == Some(text.as_str());
                        if !same && let Some(path) = found.and_then(|f| f.target.files.get(key)) {
                            ui.label(RichText::new(path).weak().monospace());
                        }
                    }
                    for new in &result.create {
                        if let Some(dir) = found.and_then(|f| f.target.dirs.get(&new.dir)) {
                            ui.label(
                                RichText::new(format!("{dir}/{}", new.name))
                                    .weak()
                                    .monospace(),
                            );
                        }
                    }
                    ui.label(RichText::new(i18n.tr("game-export-backup")).weak());
                }
            }
            let ready = matches!(d.preview, Some(Ok(_)));
            let buttons = [
                style::DialogButton::primary(i18n.tr("game-export-write")).enabled(ready),
                style::DialogButton::secondary(i18n.tr("cancel")),
            ];
            match style::dialog_buttons(ui, &buttons) {
                Some(0) => {
                    if let Some(Ok((files, result))) = d.preview.take() {
                        write = Some((files, result));
                    }
                }
                Some(_) => close = true,
                None => {}
            }
        });
        if close {
            self.export = None;
        } else if let Some((files, result)) = write {
            if let Some(found) = self.found().cloned() {
                if let Some(d) = &mut self.export {
                    d.busy = true;
                }
                self.send(Request::Write {
                    target: found.target,
                    files,
                    result,
                });
            }
        } else if again {
            self.preview(ctx);
        }
    }
}

fn self_version(targets: &[Vec<FoundTarget>], plugin: usize, target: usize) -> Option<String> {
    targets.get(plugin)?.get(target)?.version.clone()
}
