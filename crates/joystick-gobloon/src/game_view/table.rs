//! The game's actions as a table, like the game's own binding screen: groups that fold, per
//! action its joystick and its keyboard and mouse bindings as chips, and where they come from.

use super::bind::{add_ops, part_name};
use super::state::{Binding, Mark, State};
use super::*;

/// Height of a table row.
const LINE: f32 = 30.0;
/// Where the columns start, as parts of the width: action, joystick, keys, from.
const COLUMNS: [f32; 4] = [0.0, 0.30, 0.56, 0.82];

/// Which actions the table lists.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Filter {
    #[default]
    All,
    /// Those the profile binds.
    Profile,
    /// Those the next export changes in the game.
    Changed,
    /// Those sharing a control with another action.
    Shared,
}

impl Filter {
    pub(super) const ALL: [Self; 4] = [Self::All, Self::Profile, Self::Changed, Self::Shared];

    pub(super) fn key(self) -> &'static str {
        match self {
            Self::All => "game-filter-all",
            Self::Profile => "game-filter-profile",
            Self::Changed => "game-filter-changed",
            Self::Shared => "game-filter-shared",
        }
    }

    fn keeps(self, state: &State, bindings: &[Binding]) -> bool {
        match self {
            Self::All => true,
            Self::Profile => bindings.iter().any(|b| b.mark.profile()),
            Self::Changed => bindings
                .iter()
                .any(|b| matches!(b.mark, Mark::New | Mark::Removed)),
            Self::Shared => bindings
                .iter()
                .any(|b| b.mark.in_effect() && state.shared(b.output)),
        }
    }
}

enum Line<'a> {
    Group {
        name: &'a str,
        bound: usize,
        all: usize,
        open: bool,
    },
    /// Index into the graph's actions.
    Action(usize),
}

/// A binding's short name in a cell: the device is named for joysticks, keys speak for
/// themselves.
fn chip_text(i18n: &I18n, o: &GameOutput) -> String {
    match o {
        GameOutput::Keys(c) => c
            .parts()
            .iter()
            .map(|p| part_name(i18n, p))
            .collect::<Vec<_>>()
            .join(" + "),
        _ => output_name(i18n, o),
    }
}

/// One binding as a chip; a click on it edits it.
fn chip(ui: &mut egui::Ui, i18n: &I18n, b: &Binding, lit: bool, shared: bool) -> egui::Response {
    let accent = style::accent(ui);
    let mut text = RichText::new(chip_text(i18n, b.output)).size(12.5);
    let mut tip = match b.mark {
        Mark::Same => i18n.tr("game-chip-edit"),
        Mark::New => format!(
            "{}\n{}",
            i18n.tr("game-chip-new"),
            i18n.tr("game-chip-edit")
        ),
        Mark::Removed => i18n.tr("game-chip-removed"),
        Mark::Game { default: true } => format!(
            "{}\n{}",
            i18n.tr("game-chip-default"),
            i18n.tr("game-chip-edit")
        ),
        Mark::Game { default: false } => format!(
            "{}\n{}",
            i18n.tr("game-chip-game"),
            i18n.tr("game-chip-edit")
        ),
    };
    if shared {
        tip.push('\n');
        tip.push_str(&i18n.tr("game-chip-shared"));
    }
    text = match b.mark {
        Mark::Removed => text.weak().strikethrough(),
        Mark::Game { default: true } => text.weak().italics(),
        Mark::Game { default: false } => text.weak(),
        _ if lit => text.color(accent).strong(),
        _ => text,
    };
    let stroke = if shared {
        Stroke::new(1.0, ui.visuals().warn_fg_color)
    } else if lit {
        Stroke::new(1.5, accent)
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    let resp = ui.add(
        egui::Button::new(text)
            .fill(ui.visuals().faint_bg_color)
            .stroke(stroke)
            .corner_radius(4.0)
            .min_size(Vec2::new(0.0, LINE - 10.0)),
    );
    if b.mark == Mark::New {
        // Not in the game yet.
        ui.painter()
            .circle_filled(resp.rect.right_top() + Vec2::new(-2.0, 2.0), 3.0, accent);
    }
    resp.on_hover_text(tip)
}

impl GameView {
    /// The rows above the table: column names.
    fn table_head(ui: &mut egui::Ui, i18n: &I18n) {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
        for (x, key) in COLUMNS.iter().zip([
            "game-col-actions",
            "game-col-joystick",
            "game-col-keys",
            "game-from",
        ]) {
            ui.painter().text(
                Pos2::new(rect.left() + rect.width() * x + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                i18n.tr(key),
                FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn table(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        ctx: &Context,
        game: &str,
        graph: &Graph,
        live: &Live,
        state: &State,
        now: f64,
        ops: &mut Vec<Op>,
    ) {
        // The actions listed, group by group (the graph has them sorted by group and name).
        let searching = !self.search.trim().is_empty() || self.find.is_some();
        let mut lines: Vec<Line> = Vec::new();
        let mut group: Option<usize> = None;
        for (r, a) in graph.actions.iter().enumerate() {
            let bindings = state.of(&a.id);
            if !self.filter.keeps(state, bindings) {
                continue;
            }
            if let Some(find) = &self.find
                && !state.has(&a.id, find)
            {
                continue;
            }
            let fresh = match group.map(|g| &lines[g]) {
                Some(Line::Group { name, .. }) => *name != a.category,
                _ => true,
            };
            if fresh {
                group = Some(lines.len());
                lines.push(Line::Group {
                    name: &a.category,
                    bound: 0,
                    all: 0,
                    // A search shows what it found, folded or not.
                    open: searching || !self.folded.contains(&a.category),
                });
            }
            let mut shown = true;
            if let Some(Line::Group {
                bound, all, open, ..
            }) = group.map(|g| &mut lines[g])
            {
                *all += 1;
                *bound += usize::from(bindings.iter().any(|b| b.mark.profile()));
                shown = *open;
            }
            if shown {
                lines.push(Line::Action(r));
            }
        }
        if let Some(find) = &self.find
            && lines.is_empty()
        {
            ui.add_space(style::SPACE_L);
            ui.label(
                RichText::new(i18n.trf(
                    "game-find-none",
                    &[("control", output_name(i18n, find).into())],
                ))
                .weak(),
            );
            return;
        }
        Self::table_head(ui, i18n);
        // An action that lights up comes into view: once, not while it stays lit.
        // What is lit when the table opens (an axis resting off center) moves nothing.
        let opened = !std::mem::replace(&mut self.table_shown, true);
        let spacing = ui.spacing().item_spacing.y;
        let mut jump = None;
        for (n, line) in lines.iter().enumerate() {
            if let Line::Action(r) = line
                && live.actions.get(*r).copied().unwrap_or(0.0) > 0.0
                && self.lit.insert(graph.actions[*r].id.clone())
                && self.dialog.is_none()
                && !opened
            {
                jump = Some(n);
            }
        }
        let where_from: HashMap<&GameOutput, usize> = graph
            .outputs
            .iter()
            .enumerate()
            .map(|(m, o)| (o, m))
            .collect();
        let profile = ctx.profile.game_bindings(game);
        let accent = style::accent(ui);
        let mut area = egui::ScrollArea::vertical().auto_shrink(false);
        if let Some(n) = jump {
            let top = n as f32 * (LINE + spacing) - ui.available_height() / 3.0;
            area = area.vertical_scroll_offset(top.max(0.0));
        }
        area.show_rows(ui, LINE, lines.len(), |ui, range| {
            for n in range {
                let (rect, row) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), LINE), Sense::click());
                let cell = |from: usize| {
                    let left = rect.left() + rect.width() * COLUMNS[from];
                    let right = COLUMNS
                        .get(from + 1)
                        .map_or(rect.right(), |x| rect.left() + rect.width() * x);
                    Rect::from_min_max(Pos2::new(left, rect.top()), Pos2::new(right, rect.bottom()))
                };
                match &lines[n] {
                    Line::Group {
                        name,
                        bound,
                        all,
                        open,
                    } => {
                        ui.painter()
                            .rect_filled(rect, 4.0, ui.visuals().faint_bg_color);
                        let caret = if *open {
                            icon::CARET_DOWN
                        } else {
                            icon::CARET_RIGHT
                        };
                        let title = if name.is_empty() {
                            i18n.tr("game-no-group")
                        } else {
                            (*name).to_string()
                        };
                        ui.painter().text(
                            rect.left_center() + Vec2::new(8.0, 0.0),
                            Align2::LEFT_CENTER,
                            format!("{caret}  {title}"),
                            FontId::proportional(13.5),
                            ui.visuals().strong_text_color(),
                        );
                        ui.painter().text(
                            rect.right_center() - Vec2::new(10.0, 0.0),
                            Align2::RIGHT_CENTER,
                            format!("{bound} / {all}"),
                            FontId::proportional(12.0),
                            ui.visuals().weak_text_color(),
                        );
                        if row.clicked() && !searching && !self.folded.remove(*name) {
                            self.folded.insert((*name).to_string());
                        }
                    }
                    Line::Action(r) => {
                        let a = &graph.actions[*r];
                        let bindings = state.of(&a.id);
                        let selected = self.chosen.as_deref() == Some(a.id.as_str());
                        let hot = ui.rect_contains_pointer(rect);
                        if selected {
                            ui.painter()
                                .rect_filled(rect, 4.0, accent.gamma_multiply(0.18));
                        } else if hot {
                            ui.painter().rect_filled(
                                rect,
                                4.0,
                                ui.visuals().faint_bg_color.gamma_multiply(0.6),
                            );
                        }
                        if row.clicked() {
                            self.chosen = if selected { None } else { Some(a.id.clone()) };
                        }
                        let lit = live
                            .actions
                            .get(*r)
                            .copied()
                            .unwrap_or(0.0)
                            .max(self.fade(&a.id, now));
                        // The action's name.
                        let name = RichText::new(&a.name);
                        let name = if lit > 0.0 {
                            name.color(accent).strong()
                        } else if bindings.iter().any(|b| b.mark.in_effect()) {
                            name
                        } else {
                            name.weak()
                        };
                        let mut at = |rect: Rect| {
                            let mut child = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(rect.shrink2(Vec2::new(8.0, 0.0)))
                                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            );
                            child.set_clip_rect(rect.intersect(ui.clip_rect()));
                            child
                        };
                        let mut ui_name = at(cell(0));
                        let label = if a.known {
                            name
                        } else {
                            RichText::new(format!("{}  {}", icon::WARNING, a.name)).weak()
                        };
                        let mut tip = a.id.clone();
                        if !a.known {
                            tip.push('\n');
                            tip.push_str(&i18n.tr("game-not-in-catalog"));
                        }
                        ui_name
                            .add(egui::Label::new(label).truncate().selectable(false))
                            .on_hover_text(tip);
                        // Its bindings: joystick, then keyboard and mouse.
                        for (column, keys) in [(1, false), (2, true)] {
                            let mut ui_cell = at(cell(column));
                            ui_cell.spacing_mut().item_spacing.x = 4.0;
                            for b in bindings
                                .iter()
                                .filter(|b| b.output.combo().is_some() == keys)
                            {
                                let shared = b.mark.in_effect() && state.shared(b.output);
                                let on = lit > 0.0
                                    && where_from.get(b.output).is_some_and(|m| {
                                        live.outputs.get(*m).copied().unwrap_or(0.0) > 0.0
                                    });
                                if chip(&mut ui_cell, i18n, b, on, shared).clicked() {
                                    self.chosen = Some(a.id.clone());
                                    if b.mark == Mark::Removed {
                                        // Kept: back into the profile.
                                        add_ops(game, state, &a.id, b.output, None, ops);
                                    } else {
                                        self.press_for(Some(a.id.clone()), Some(b.output.clone()));
                                    }
                                }
                            }
                            if (hot || selected)
                                && ui_cell
                                    .add(
                                        egui::Button::new(RichText::new(icon::PLUS).weak())
                                            .frame(false),
                                    )
                                    .on_hover_text(i18n.tr("game-add"))
                                    .clicked()
                            {
                                self.chosen = Some(a.id.clone());
                                self.press_for(Some(a.id.clone()), None);
                            }
                            // Held by the profile: the way back to the game's own.
                            if keys
                                && (hot || selected)
                                && profile.is_some_and(|p| p.keys_of(&a.id).is_some())
                                && ui_cell
                                    .add(
                                        egui::Button::new(
                                            RichText::new(icon::ARROW_COUNTER_CLOCKWISE).weak(),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text(i18n.tr("game-keys-reset"))
                                    .clicked()
                            {
                                ops.push(Op::GameKeys {
                                    game: game.to_string(),
                                    action: a.id.clone(),
                                    keys: None,
                                });
                            }
                        }
                        // Where they come from: the controller inputs writing those outputs.
                        let mut sources: BTreeSet<String> = BTreeSet::new();
                        for b in bindings.iter().filter(|b| b.mark.in_effect()) {
                            if let Some(m) = where_from.get(b.output) {
                                sources.extend(
                                    graph
                                        .input_edges
                                        .iter()
                                        .zip(&graph.input_routes)
                                        .filter(|((_, x), _)| x == m)
                                        .map(|((l, _), route)| {
                                            let label = &graph.inputs[*l].label;
                                            if route.is_empty() {
                                                label.clone()
                                            } else {
                                                format!("{label} ({route})")
                                            }
                                        }),
                                );
                            }
                        }
                        let from = sources.into_iter().collect::<Vec<_>>().join(", ");
                        if !from.is_empty() {
                            at(cell(3))
                                .add(
                                    egui::Label::new(RichText::new(&from).weak().size(12.5))
                                        .truncate()
                                        .selectable(false),
                                )
                                .on_hover_text(&from);
                        }
                    }
                }
            }
        });
    }
}
