//! Binding by pressing: the wait for a press, what the pressed control is in the profile, the
//! warning when other actions already use it, and mapping a control the profile does not map.

use gob_core::{Combo, ComboPart};

use super::capture::{Capture, Captured, Pressed};
use super::state::{Mark, State};
use super::*;

/// What a press is for.
#[derive(Clone, Default)]
pub(super) struct Aim {
    /// The game action to bind; `None` only looks the control up.
    pub(super) action: Option<String>,
    /// The binding of the action the press replaces.
    pub(super) replace: Option<GameOutput>,
}

pub(super) enum Dialog {
    /// Waiting for a press.
    Capture(Aim, Capture),
    /// The pressed input writes several outputs: which one.
    Choose {
        aim: Aim,
        input: String,
        outputs: Vec<GameOutput>,
        /// Per output: how the input reaches it ([`route_of`]).
        routes: Vec<String>,
    },
    /// Other actions use the control already.
    Conflict {
        action: String,
        replace: Option<GameOutput>,
        output: GameOutput,
        /// (action id, the game's own default).
        users: Vec<(String, bool)>,
    },
    /// The pressed input has no output in the profile: map it to `output` first.
    Unmapped {
        action: String,
        replace: Option<GameOutput>,
        pressed: Pressed,
        /// The profile's id of the device.
        device: String,
        output: VirtualOutput,
        bind: GameOutput,
    },
}

fn kind_name(kind: InputType) -> &'static str {
    match kind {
        InputType::Axis => "axis",
        InputType::Hat => "hat",
        _ => "button",
    }
}

fn pressed_name(i18n: &I18n, p: &Pressed) -> String {
    let input = crate::output_marks::input_name(i18n, kind_name(p.kind), &p.id.to_string());
    match p.hat {
        Some(d) => format!("{} · {input} {}", p.device_name, arrow(d)),
        None => format!("{} · {input}", p.device_name),
    }
}

/// A whole hat (or d-pad): games bind its directions.
fn is_hat(v: &VirtualOutput) -> bool {
    match v {
        VirtualOutput::Vjoy { input_type, .. } | VirtualOutput::Logical { input_type, .. } => {
            *input_type == InputType::Hat
        }
        VirtualOutput::Gamepad { control, .. } => control == "dpad",
    }
}

/// The profile's id of the device `guid` (a profile made on another computer names it by its
/// old id), and the bindings of the pressed input.
pub(super) fn bindings_of(ctx: &Context, p: &Pressed) -> (String, Vec<usize>) {
    let aliases = crate::substitute_view::auto_aliases(ctx.profile, ctx.devices);
    let mut device = p.device.clone();
    let mut found = Vec::new();
    for (n, b) in ctx.profile.inputs.iter().enumerate() {
        let id = b.device_id.trim().to_ascii_lowercase();
        let here = aliases.get(&id).map_or(id.as_str(), String::as_str);
        if !here.eq_ignore_ascii_case(&p.device) {
            continue;
        }
        device = b.device_id.trim().to_string();
        if b.input_type.trim() == kind_name(p.kind)
            && b.input_id.trim().parse::<u32>().ok() == Some(p.id)
        {
            found.push(n);
        }
    }
    if found.is_empty()
        && let Some((old, _)) = aliases
            .iter()
            .find(|(_, now)| now.eq_ignore_ascii_case(&p.device))
    {
        device = old.clone();
    }
    (device, found)
}

/// What a game can bind of what the pressed input sends: virtual joystick and gamepad outputs,
/// keys and mouse buttons. What the running profile sends right now comes first.
pub(super) fn outputs_of(ctx: &Context, p: &Pressed, bindings: &[usize]) -> Vec<GameOutput> {
    let mine = |uses: &[gob_core::OutputUse]| uses.iter().any(|u| bindings.contains(&u.binding));
    let mut out: Vec<GameOutput> = Vec::new();
    for (v, uses) in ctx.profile.output_usage(None) {
        if !mine(&uses) || matches!(v, VirtualOutput::Logical { .. }) {
            continue;
        }
        if !is_hat(&v) {
            out.push(GameOutput::Virtual(v));
        } else if let Some(d) = p.hat {
            out.push(GameOutput::HatDirection(v, d));
        } else {
            out.extend(
                HatDirection::ALL
                    .iter()
                    .map(|d| GameOutput::HatDirection(v.clone(), *d)),
            );
        }
    }
    for (combo, uses) in ctx.profile.key_usage(None) {
        if mine(&uses) {
            out.push(GameOutput::Keys(combo));
        }
    }
    let now = std::time::Instant::now();
    let lit = |o: &GameOutput| {
        ctx.sent
            .as_ref()
            .and_then(|sent| sent.read(|map| super::graph::sent_lit(map, o, now)))
            .is_some_and(|v| v > 0.0)
    };
    out.sort_by_key(|o| (!lit(o), super::graph::sort_key(o)));
    out
}

/// How the pressed input's `bindings` reach output `o` ("Long press"), empty for a plain
/// mapping: tells apart several outputs of one control.
pub(super) fn route_of(ctx: &Context, i18n: &I18n, bindings: &[usize], o: &GameOutput) -> String {
    let uses: Vec<gob_core::OutputUse> = match o {
        GameOutput::Virtual(v) | GameOutput::HatDirection(v, _) => {
            ctx.profile.output_usage(None).remove(v).unwrap_or_default()
        }
        GameOutput::Keys(c) => ctx
            .profile
            .key_usage(None)
            .into_iter()
            .find(|(k, _)| k == c)
            .map(|(_, u)| u)
            .unwrap_or_default(),
    };
    let mut labels: Vec<String> = Vec::new();
    for u in uses.iter().filter(|u| bindings.contains(&u.binding)) {
        let l = super::graph::route_label(i18n, ctx.profile, u.binding, &u.action);
        if !l.is_empty() && !labels.contains(&l) {
            labels.push(l);
        }
    }
    labels.join(", ")
}

/// The first output of the input's kind that no input of the profile writes and no game action
/// is bound to, on the virtual joysticks in their order.
fn free_output(ctx: &Context, game: &str, p: &Pressed) -> Option<VirtualOutput> {
    let mut busy: std::collections::HashSet<VirtualOutput> =
        ctx.profile.output_usage(None).into_keys().collect();
    if let Some(g) = ctx.profile.game_bindings(game) {
        busy.extend(
            g.binds
                .iter()
                .filter_map(|b| b.output.virtual_output().cloned()),
        );
    }
    let mut devices: Vec<&gob_virtual::DeviceInfo> = ctx.vjoy.iter().collect();
    devices.sort_by_key(|d| d.id);
    for d in devices {
        let at = |id: u32| VirtualOutput::Vjoy {
            device: d.id,
            input_type: p.kind,
            id,
        };
        let ids: Vec<u32> = match p.kind {
            // The same axis where the virtual joystick has it, else its first free one.
            InputType::Axis => {
                let mut ids: Vec<u32> = d.axes.iter().map(|a| u32::from(*a)).collect();
                ids.sort_by_key(|a| *a != p.id);
                ids
            }
            InputType::Hat => (1..=d.hats()).collect(),
            _ => (1..=d.buttons).collect(),
        };
        if let Some(id) = ids.into_iter().find(|id| !busy.contains(&at(*id))) {
            return Some(at(id));
        }
    }
    None
}

/// Edits that make `output` drive `action`, in place of `replace` when given.
pub(super) fn add_ops(
    game: &str,
    state: &State,
    action: &str,
    output: &GameOutput,
    replace: Option<&GameOutput>,
    ops: &mut Vec<Op>,
) {
    let replace = replace.filter(|r| *r != output);
    let old_keys = replace.and_then(GameOutput::combo);
    if old_keys.is_some() || output.combo().is_some() {
        // The profile takes the action's keyboard and mouse bindings over as they are now.
        let mut keys = state.keys(action);
        keys.retain(|k| Some(k) != old_keys);
        if let Some(new) = output.combo()
            && !keys.contains(new)
        {
            keys.push(new.clone());
        }
        ops.push(Op::GameKeys {
            game: game.to_string(),
            action: action.to_string(),
            keys: Some(keys),
        });
    }
    if let Some(old) = replace.filter(|r| r.combo().is_none()) {
        ops.push(Op::GameUnbind {
            game: game.to_string(),
            action: action.to_string(),
            output: Some(old.clone()),
        });
    }
    if output.combo().is_none() {
        ops.push(Op::GameBind {
            game: game.to_string(),
            action: action.to_string(),
            output: output.clone(),
        });
    }
}

/// Edits that take `output` away from `action`.
pub(super) fn remove_ops(
    game: &str,
    state: &State,
    action: &str,
    output: &GameOutput,
    ops: &mut Vec<Op>,
) {
    match output.combo() {
        Some(combo) => {
            let mut keys = state.keys(action);
            keys.retain(|k| k != combo);
            ops.push(Op::GameKeys {
                game: game.to_string(),
                action: action.to_string(),
                keys: Some(keys),
            });
        }
        None => ops.push(Op::GameUnbind {
            game: game.to_string(),
            action: action.to_string(),
            output: Some(output.clone()),
        }),
    }
}

/// What a dialog frame asks for.
enum Step {
    Stay,
    Close,
    Go(Box<Dialog>),
}

/// Everything the dialogs read.
pub(super) struct Scene<'a> {
    pub(super) i18n: &'a I18n,
    pub(super) ctx: &'a Context<'a>,
    pub(super) game: &'a str,
    pub(super) catalog: &'a Catalog,
    pub(super) state: &'a State<'a>,
}

impl Scene<'_> {
    fn action_name(&self, id: &str) -> String {
        self.catalog
            .action(id)
            .map_or_else(|| id.to_string(), |a| a.name.clone())
    }

    fn action_line(&self, id: &str) -> String {
        match self.catalog.action(id) {
            Some(a) if !a.category.is_empty() => format!("{} · {}", a.category, a.name),
            Some(a) => a.name.clone(),
            None => id.to_string(),
        }
    }
}

impl GameView {
    /// Starts the wait for a press.
    pub(super) fn press_for(&mut self, action: Option<String>, replace: Option<GameOutput>) {
        self.dialog = Some(Dialog::Capture(Aim { action, replace }, Capture::default()));
    }

    /// The control is known: looked up, bound, or held back for the warning.
    fn settle(&mut self, scene: &Scene, aim: Aim, output: GameOutput, ops: &mut Vec<Op>) -> Step {
        let Some(action) = aim.action else {
            if self.tab == Tab::Wiring {
                self.picked = Some(output.clone());
            }
            self.find = Some(output);
            return Step::Close;
        };
        let args = |output: &GameOutput| {
            [
                ("control", output_name(scene.i18n, output).into()),
                ("action", scene.action_name(&action).into()),
            ]
        };
        if scene.state.has(&action, &output) {
            self.note = Some(scene.i18n.trf("game-already", &args(&output)));
            return Step::Close;
        }
        let users: Vec<(String, bool)> = scene
            .state
            .others(&output, &action)
            .into_iter()
            .map(|(a, mark)| (a.to_string(), mark == Mark::Game { default: true }))
            .collect();
        if !users.is_empty() {
            return Step::Go(Box::new(Dialog::Conflict {
                action,
                replace: aim.replace,
                output,
                users,
            }));
        }
        add_ops(
            scene.game,
            scene.state,
            &action,
            &output,
            aim.replace.as_ref(),
            ops,
        );
        self.note = Some(scene.i18n.trf("game-bound", &args(&output)));
        Step::Close
    }

    /// A joystick input was pressed: its output in the profile, a choice of several, or the
    /// offer to map it.
    fn pressed(&mut self, scene: &Scene, aim: Aim, p: Pressed, ops: &mut Vec<Op>) -> Step {
        let (device, bindings) = bindings_of(scene.ctx, &p);
        let mut outputs = outputs_of(scene.ctx, &p, &bindings);
        let input = pressed_name(scene.i18n, &p);
        match outputs.len() {
            0 => {
                let Some(action) = aim.action else {
                    self.note = Some(
                        scene
                            .i18n
                            .trf("game-unmapped-find", &[("input", input.into())]),
                    );
                    return Step::Close;
                };
                let Some(output) = free_output(scene.ctx, scene.game, &p) else {
                    self.note = Some(
                        scene
                            .i18n
                            .trf("game-unmapped-full", &[("input", input.into())]),
                    );
                    return Step::Close;
                };
                let bind = match p.hat {
                    Some(d) => GameOutput::HatDirection(output.clone(), d),
                    None => GameOutput::Virtual(output.clone()),
                };
                Step::Go(Box::new(Dialog::Unmapped {
                    action,
                    replace: aim.replace,
                    pressed: p,
                    device,
                    output,
                    bind,
                }))
            }
            1 => self.settle(scene, aim, outputs.remove(0), ops),
            _ => {
                let routes = outputs
                    .iter()
                    .map(|o| route_of(scene.ctx, scene.i18n, &bindings, o))
                    .collect();
                Step::Go(Box::new(Dialog::Choose {
                    aim,
                    input,
                    outputs,
                    routes,
                }))
            }
        }
    }

    /// Binds `aim`'s action to a control known without a press (dropped on its picture): the
    /// same way as pressing it, with the same dialogs.
    pub(super) fn bind_pressed(&mut self, scene: &Scene, aim: Aim, p: Pressed, ops: &mut Vec<Op>) {
        if let Step::Go(next) = self.pressed(scene, aim, p, ops) {
            self.dialog = Some(*next);
        }
    }

    /// Shows the open dialog and acts on it.
    pub(super) fn bind_dialog(&mut self, ui: &mut egui::Ui, scene: &Scene, ops: &mut Vec<Op>) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let (dialog, step) = match dialog {
            Dialog::Capture(aim, capture) => self.capture_dialog(ui, scene, aim, capture, ops),
            Dialog::Choose {
                aim,
                input,
                outputs,
                routes,
            } => self.choose_dialog(ui, scene, aim, input, outputs, routes, ops),
            Dialog::Conflict {
                action,
                replace,
                output,
                users,
            } => self.conflict_dialog(ui, scene, action, replace, output, users, ops),
            Dialog::Unmapped {
                action,
                replace,
                pressed,
                device,
                output,
                bind,
            } => self.unmapped_dialog(
                ui, scene, action, replace, pressed, device, output, bind, ops,
            ),
        };
        self.dialog = match step {
            Step::Stay => dialog,
            Step::Close => None,
            Step::Go(next) => Some(*next),
        };
    }

    fn capture_dialog(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Scene,
        aim: Aim,
        mut capture: Capture,
        ops: &mut Vec<Op>,
    ) -> (Option<Dialog>, Step) {
        let i18n = scene.i18n;
        let title = match &aim.action {
            Some(a) => scene.action_name(a),
            None => i18n.tr("game-capture-title"),
        };
        let mut captured = None;
        let mut picked = None;
        let mut clear = false;
        let mut cancel = false;
        let modal = style::modal(ui.ctx(), "game-capture").show(ui.ctx(), |ui| {
            ui.set_width(460.0);
            style::dialog_title(ui, Some((icon::HAND_TAP, None)), &title);
            if let Some(old) = &aim.replace {
                ui.label(
                    RichText::new(i18n.trf(
                        "game-capture-now",
                        &[("control", output_name(i18n, old).into())],
                    ))
                    .weak(),
                );
                ui.add_space(style::SPACE_S);
            }
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 84.0), Sense::click());
            let accent = style::accent(ui);
            ui.painter()
                .rect_filled(rect, 8.0, accent.gamma_multiply(0.10));
            ui.painter().rect_stroke(
                rect,
                8.0,
                Stroke::new(1.5, accent),
                egui::StrokeKind::Inside,
            );
            let (text, color) = if capture.parts().is_empty() {
                (i18n.tr("game-capture-hint"), ui.visuals().weak_text_color())
            } else {
                let parts: Vec<String> =
                    capture.parts().iter().map(|p| part_name(i18n, p)).collect();
                (parts.join(" + "), ui.visuals().strong_text_color())
            };
            let galley = ui.painter().layout(
                text,
                FontId::proportional(if capture.parts().is_empty() {
                    13.0
                } else {
                    18.0
                }),
                color,
                rect.width() - 24.0,
            );
            let pos = rect.center() - galley.size() / 2.0;
            ui.painter().galley(pos, galley, color);
            captured = capture.poll(ui, scene.ctx.devices, Some(rect));
            ui.add_space(style::SPACE_S);
            ui.horizontal(|ui| {
                // Keys that cannot be pressed here: Esc closes the dialog, some keyboards lack keys.
                picked = crate::key_capture::pick_key(ui, i18n);
            });
            let mut buttons = Vec::new();
            if aim.replace.is_some() {
                buttons.push(style::DialogButton::secondary(
                    i18n.tr("game-capture-clear"),
                ));
            }
            buttons.push(style::DialogButton::secondary(i18n.tr("cancel")));
            match style::dialog_buttons(ui, &buttons) {
                Some(0) if aim.replace.is_some() => clear = true,
                Some(_) => cancel = true,
                None => {}
            }
        });
        if cancel || modal.should_close() {
            return (None, Step::Close);
        }
        if clear {
            if let (Some(action), Some(old)) = (&aim.action, &aim.replace) {
                remove_ops(scene.game, scene.state, action, old, ops);
            }
            return (None, Step::Close);
        }
        if let Some(key) = picked {
            let output = GameOutput::Keys(Combo::key(key));
            let step = self.settle(scene, aim, output, ops);
            return (None, step);
        }
        match captured {
            Some(Captured::Keys(combo)) => {
                let step = self.settle(scene, aim, GameOutput::Keys(combo), ops);
                (None, step)
            }
            Some(Captured::Input(p)) => {
                let step = self.pressed(scene, aim, p, ops);
                (None, step)
            }
            None => (Some(Dialog::Capture(aim, capture)), Step::Stay),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn choose_dialog(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Scene,
        aim: Aim,
        input: String,
        outputs: Vec<GameOutput>,
        routes: Vec<String>,
        ops: &mut Vec<Op>,
    ) -> (Option<Dialog>, Step) {
        let i18n = scene.i18n;
        let mut chosen = None;
        let mut cancel = false;
        let modal = style::modal(ui.ctx(), "game-choose").show(ui.ctx(), |ui| {
            ui.set_width(460.0);
            style::dialog_title(
                ui,
                Some((icon::FLOW_ARROW, None)),
                &i18n.tr("game-choose-title"),
            );
            ui.label(i18n.trf("game-choose-text", &[("input", input.clone().into())]));
            ui.add_space(style::SPACE_S);
            for (n, o) in outputs.iter().enumerate() {
                let text = match routes.get(n).filter(|r| !r.is_empty()) {
                    Some(r) => format!("{}  ·  {r}", output_name(i18n, o)),
                    None => output_name(i18n, o),
                };
                if ui.button(text).clicked() {
                    chosen = Some(n);
                }
            }
            let buttons = [style::DialogButton::secondary(i18n.tr("cancel"))];
            if style::dialog_buttons(ui, &buttons).is_some() {
                cancel = true;
            }
        });
        if cancel || modal.should_close() {
            return (None, Step::Close);
        }
        match chosen {
            Some(n) => {
                let step = self.settle(scene, aim, outputs[n].clone(), ops);
                (None, step)
            }
            None => (
                Some(Dialog::Choose {
                    aim,
                    input,
                    outputs,
                    routes,
                }),
                Step::Stay,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn conflict_dialog(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Scene,
        action: String,
        replace: Option<GameOutput>,
        output: GameOutput,
        users: Vec<(String, bool)>,
        ops: &mut Vec<Op>,
    ) -> (Option<Dialog>, Step) {
        let i18n = scene.i18n;
        let mut choice = None;
        let modal = style::modal(ui.ctx(), "game-conflict").show(ui.ctx(), |ui| {
            ui.set_width(520.0);
            style::dialog_title(
                ui,
                Some((icon::WARNING, Some(ui.visuals().warn_fg_color))),
                &i18n.tr("game-conflict-title"),
            );
            ui.label(i18n.trf(
                "game-conflict-text",
                &[("control", output_name(i18n, &output).into())],
            ));
            ui.add_space(style::SPACE_S);
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (id, default) in &users {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon::ARROW_RIGHT).weak());
                            ui.label(scene.action_line(id)).on_hover_text(id);
                            if *default {
                                ui.label(RichText::new(i18n.tr("game-conflict-default")).weak());
                            }
                        });
                    }
                });
            ui.add_space(style::SPACE_S);
            ui.label(
                RichText::new(i18n.trf(
                    "game-conflict-question",
                    &[("action", scene.action_name(&action).into())],
                ))
                .weak(),
            );
            let buttons = [
                style::DialogButton::primary(i18n.tr("game-conflict-replace")),
                style::DialogButton::secondary(i18n.tr("game-conflict-add")),
                style::DialogButton::secondary(i18n.tr("cancel")),
            ];
            choice = style::dialog_buttons(ui, &buttons);
        });
        let choice = if modal.should_close() {
            Some(2)
        } else {
            choice
        };
        match choice {
            Some(n @ (0 | 1)) => {
                add_ops(
                    scene.game,
                    scene.state,
                    &action,
                    &output,
                    replace.as_ref(),
                    ops,
                );
                if n == 0 {
                    for (other, _) in &users {
                        remove_ops(scene.game, scene.state, other, &output, ops);
                    }
                }
                self.note = Some(i18n.trf(
                    "game-bound",
                    &[
                        ("control", output_name(i18n, &output).into()),
                        ("action", scene.action_name(&action).into()),
                    ],
                ));
                (None, Step::Close)
            }
            Some(_) => (None, Step::Close),
            None => (
                Some(Dialog::Conflict {
                    action,
                    replace,
                    output,
                    users,
                }),
                Step::Stay,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn unmapped_dialog(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Scene,
        action: String,
        replace: Option<GameOutput>,
        pressed: Pressed,
        device: String,
        output: VirtualOutput,
        bind: GameOutput,
        ops: &mut Vec<Op>,
    ) -> (Option<Dialog>, Step) {
        let i18n = scene.i18n;
        let mut choice = None;
        let modal = style::modal(ui.ctx(), "game-unmapped").show(ui.ctx(), |ui| {
            ui.set_width(480.0);
            style::dialog_title(
                ui,
                Some((icon::JOYSTICK, None)),
                &i18n.tr("game-unmapped-title"),
            );
            ui.label(i18n.trf(
                "game-unmapped-text",
                &[
                    ("input", pressed_name(i18n, &pressed).into()),
                    (
                        "output",
                        output_name(i18n, &GameOutput::Virtual(output.clone())).into(),
                    ),
                    ("action", scene.action_name(&action).into()),
                ],
            ));
            let buttons = [
                style::DialogButton::primary(i18n.tr("game-unmapped-go")),
                style::DialogButton::secondary(i18n.tr("cancel")),
            ];
            choice = style::dialog_buttons(ui, &buttons);
        });
        let choice = if modal.should_close() {
            Some(1)
        } else {
            choice
        };
        match choice {
            Some(0) => {
                // The other actions using a free output are none: no warning to show.
                ops.push(Op::GameMap {
                    device,
                    device_name: pressed.device_name.clone(),
                    input_type: pressed.kind,
                    input_id: pressed.id,
                    mode: scene.ctx.mode.to_string(),
                    output,
                });
                add_ops(
                    scene.game,
                    scene.state,
                    &action,
                    &bind,
                    replace.as_ref(),
                    ops,
                );
                self.note = Some(i18n.trf(
                    "game-bound",
                    &[
                        ("control", output_name(i18n, &bind).into()),
                        ("action", scene.action_name(&action).into()),
                    ],
                ));
                (None, Step::Close)
            }
            Some(_) => (None, Step::Close),
            None => (
                Some(Dialog::Unmapped {
                    action,
                    replace,
                    pressed,
                    device,
                    output,
                    bind,
                }),
                Step::Stay,
            ),
        }
    }

    /// Debug builds open a dialog at start with `JOYSTICK_GOBLOON_GAME_DIALOG` = capture |
    /// conflict | choose | unmapped, for checks.
    pub(super) fn debug_dialog(&mut self, scene: &Scene) {
        if !cfg!(debug_assertions) || std::mem::replace(&mut self.debug_done, true) {
            return;
        }
        let Ok(which) = std::env::var("JOYSTICK_GOBLOON_GAME_DIALOG") else {
            return;
        };
        let Some(first) = scene.catalog.actions.first() else {
            return;
        };
        let action = first.id.clone();
        let output = GameOutput::parse("vjoy:1:button:5").expect("an output id");
        let pressed = Pressed {
            device: "debug".into(),
            device_name: "Stick".into(),
            kind: InputType::Button,
            id: 3,
            hat: None,
        };
        self.dialog = match which.as_str() {
            "capture" => Some(Dialog::Capture(
                Aim {
                    action: Some(action),
                    replace: None,
                },
                Capture::default(),
            )),
            "conflict" => Some(Dialog::Conflict {
                action,
                replace: None,
                output,
                users: scene
                    .catalog
                    .actions
                    .iter()
                    .skip(1)
                    .take(3)
                    .enumerate()
                    .map(|(n, a)| (a.id.clone(), n == 2))
                    .collect(),
            }),
            "choose" => Some(Dialog::Choose {
                aim: Aim {
                    action: Some(action),
                    replace: None,
                },
                input: pressed_name(scene.i18n, &pressed),
                outputs: vec![
                    output,
                    GameOutput::parse("vjoy:1:button:6").expect("an output id"),
                ],
                routes: vec![scene.i18n.tr("edit-short"), scene.i18n.tr("edit-long")],
            }),
            "unmapped" => Some(Dialog::Unmapped {
                action,
                replace: None,
                pressed,
                device: "debug".into(),
                output: VirtualOutput::Vjoy {
                    device: 1,
                    input_type: InputType::Button,
                    id: 12,
                },
                bind: GameOutput::parse("vjoy:1:button:12").expect("an output id"),
            }),
            _ => None,
        };
    }
}

/// A key's or mouse button's name.
pub(super) fn part_name(i18n: &I18n, part: &ComboPart) -> String {
    match part {
        ComboPart::Key(k) => crate::key_capture::name(*k, i18n),
        ComboPart::Mouse(b) => i18n.tr(&format!(
            "mouse-{}",
            b.name().to_lowercase().replace(' ', "-")
        )),
    }
}
