//! The game's actions as a table, like the game's own binding screen.

use super::*;

impl GameView {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn list(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        game: &str,
        graph: &Graph,
        live: &Live,
        now: f64,
        ops: &mut Vec<Op>,
    ) {
        let in_game: HashMap<String, Vec<String>> = match &self.in_game {
            Some((p, read)) if *p == self.selected => {
                let mut m: HashMap<String, Vec<String>> = HashMap::new();
                for b in &read.binds {
                    m.entry(b.action.clone())
                        .or_default()
                        .push(b.output.clone());
                }
                m
            }
            _ => HashMap::new(),
        };
        let accent = style::accent(ui);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Grid::new("game-list")
                    .striped(true)
                    .num_columns(4)
                    .spacing([style::SPACE_L, style::SPACE_S])
                    .show(ui, |ui| {
                        for key in [
                            "game-col-actions",
                            "game-in-game",
                            "game-in-profile",
                            "game-from",
                        ] {
                            ui.label(RichText::new(i18n.tr(key)).weak());
                        }
                        ui.end_row();
                        let mut last_category = "";
                        for (r, a) in graph.actions.iter().enumerate() {
                            if a.category != last_category {
                                ui.label(RichText::new(&a.category).strong());
                                ui.end_row();
                                last_category = &a.category;
                            }
                            let lit = live
                                .actions
                                .get(r)
                                .copied()
                                .unwrap_or(0.0)
                                .max(self.fade(&a.id, now));
                            let name = RichText::new(&a.name);
                            let name = if lit > 0.0 {
                                name.color(accent).strong()
                            } else if a.bound {
                                name
                            } else {
                                name.weak()
                            };
                            let resp = ui.label(name).on_hover_text(&a.id);
                            // Scrolled to when it lights up, not while it stays lit.
                            if live.actions.get(r).copied().unwrap_or(0.0) > 0.0
                                && self.lit.insert(a.id.clone())
                            {
                                resp.scroll_to_me(None);
                            }
                            let game_side = in_game
                                .get(a.id.as_str())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|o| GameOutput::parse(o))
                                        .map(|o| output_name(i18n, &o))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default();
                            ui.label(RichText::new(game_side).weak());
                            let outs: Vec<usize> = graph
                                .action_edges
                                .iter()
                                .filter(|(_, x)| *x == r)
                                .map(|(m, _)| *m)
                                .collect();
                            ui.horizontal(|ui| {
                                ui.label(
                                    outs.iter()
                                        .map(|m| output_name(i18n, &graph.outputs[*m]))
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                );
                                // Link: a picked output at once, else the output of the
                                // control pressed next.
                                let waiting = self.linking.as_deref() == Some(a.id.as_str());
                                let tip = if self.picked.is_some() {
                                    "game-click-to-link"
                                } else {
                                    "game-link-press"
                                };
                                if style::icon_toggle(ui, waiting, icon::LINK, &i18n.tr(tip))
                                    .clicked()
                                {
                                    if let Some(o) = self.picked.take() {
                                        ops.push(Op::GameBind {
                                            game: game.to_string(),
                                            action: a.id.clone(),
                                            output: o,
                                        });
                                    } else if waiting {
                                        self.listening = false;
                                    } else {
                                        self.linking = Some(a.id.clone());
                                        self.listening = true;
                                        self.held = None;
                                    }
                                }
                                if a.bound
                                    && style::icon_button(
                                        ui,
                                        true,
                                        icon::TRASH,
                                        &i18n.tr("game-unbind"),
                                    )
                                    .clicked()
                                {
                                    ops.push(Op::GameUnbind {
                                        game: game.to_string(),
                                        action: a.id.clone(),
                                        output: None,
                                    });
                                }
                            });
                            let sources: BTreeSet<&str> = outs
                                .iter()
                                .flat_map(|m| {
                                    graph
                                        .input_edges
                                        .iter()
                                        .filter(move |(_, x)| x == m)
                                        .map(|(l, _)| graph.inputs[*l].label.as_str())
                                })
                                .collect();
                            ui.label(
                                RichText::new(sources.into_iter().collect::<Vec<_>>().join(", "))
                                    .weak(),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
}
