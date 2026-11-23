//! OSC: the OSC tab of the Devices page (its inputs: add, listen, address, kind, range) and the
//! send-osc action's editor.

use std::collections::HashMap;

use eframe::egui::{self, RichText};
use gob_core::{Action, OscArg, OscArgKind, OscInput, OscKind, Profile};
use gob_i18n::I18n;

use crate::action_editor::{Editor, Op};
use crate::style::{self, icon};

/// The most addresses whose last number is kept.
const MAX_ADDRESSES: usize = 1024;

/// What the OSC tab keeps between frames.
#[derive(Default)]
pub struct OscTab {
    /// The next message adds its address as an input.
    pub listening: bool,
    /// A message that arrived while listening: its address and number.
    heard: Option<(String, Option<f64>)>,
    /// The address and number of the last message, for following it to its input (when inputs
    /// are followed).
    pub used: Option<(String, Option<f64>)>,
    /// The last number of each address heard (`None`: none came with it).
    values: HashMap<String, Option<f64>>,
    /// The address typed for a new input.
    new_address: String,
    new_kind: OscKind,
    /// Whether OSC messages are received, filled by the app.
    pub receiving: gob_input::OscStatus,
    /// The Settings button of the "OSC is off" banner was clicked: the app opens the Panels
    /// settings.
    pub open_settings: bool,
}

impl OscTab {
    /// An OSC message arrived (`follow`: the page follows used inputs).
    pub fn heard(&mut self, address: String, value: Option<f64>, follow: bool) {
        // Any sender on the network may make up addresses: the list stays bounded.
        if self.values.len() >= MAX_ADDRESSES && !self.values.contains_key(&address) {
            self.values.clear();
        }
        self.values.insert(address.clone(), value);
        if self.listening {
            self.listening = false;
            self.heard = Some((address, value));
        } else if follow {
            self.used = Some((address, value));
        }
    }

    /// The last number heard for `address`: `None` = nothing heard yet, `Some(None)` = it came
    /// without one.
    pub fn value(&self, address: &str) -> Option<Option<f64>> {
        self.values.get(address).copied()
    }

    /// Above the OSC tab's list: whether messages arrive, the field adding an input and the
    /// button listening for one. Returns the input to show (one just added).
    pub fn tools(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: &Profile,
        ops: &mut Vec<Op>,
    ) -> Option<(OscKind, u32)> {
        let banner = egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(8)
            .inner_margin(8);
        match &self.receiving {
            gob_input::OscStatus::Listening(_) => {}
            gob_input::OscStatus::Off => {
                banner.show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(icon::BROADCAST).size(style::ICON_SIZE));
                        ui.label(i18n.tr("osc-off"));
                        let text = format!("{}  {}", icon::GEAR, i18n.tr("osc-open-settings"));
                        if ui.button(text).clicked() {
                            self.open_settings = true;
                        }
                    });
                });
                ui.add_space(4.0);
            }
            gob_input::OscStatus::Failed(error) => {
                banner.show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(icon::WARNING)
                                .size(style::ICON_SIZE)
                                .color(ui.visuals().warn_fg_color),
                        );
                        ui.label(i18n.tr("osc-port-failed")).on_hover_text(error);
                        let text = format!("{}  {}", icon::GEAR, i18n.tr("osc-open-settings"));
                        if ui.button(text).clicked() {
                            self.open_settings = true;
                        }
                    });
                });
                ui.add_space(4.0);
            }
        }
        ui.label(RichText::new(i18n.tr("osc-head")).weak());
        let mut add = None;
        ui.horizontal(|ui| {
            let field = style::text_field(&mut self.new_address)
                .hint_text("/deck/fire")
                .desired_width(170.0);
            let r = ui.add(field).on_hover_text(i18n.tr("osc-address-tip"));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            kind_combo(ui, i18n, "osc-new-kind", &mut self.new_kind);
            let valid = gob_core::osc_address(&self.new_address).is_some();
            let clicked = style::icon_button(ui, valid, icon::PLUS, &i18n.tr("osc-add")).clicked();
            if valid && (clicked || enter) {
                add = Some((self.new_address.clone(), self.new_kind));
                self.new_address.clear();
            }
            ui.separator();
            let tip = i18n.tr("osc-listen-tip");
            if style::icon_toggle(ui, self.listening, icon::EAR, &tip).clicked() {
                self.listening = !self.listening;
                self.heard = None;
            }
            if self.listening {
                ui.label(RichText::new(i18n.tr("osc-listening")).weak());
            }
        });
        if let Some((address, value)) = self.heard.take() {
            add = Some((address, guess_kind(value)));
        }
        let shown = add.and_then(|(address, kind)| {
            let address = gob_core::osc_address(&address)?;
            let (kind, id) = match profile.osc_input_at(&address) {
                Some(i) => (i.kind, i.id),
                None => {
                    ops.push(Op::AddOscInput {
                        address: address.clone(),
                        kind,
                    });
                    (kind, profile.next_osc_input_id())
                }
            };
            log::debug!("OSC input {id}: {address}");
            Some((kind, id))
        });
        if profile.osc_inputs.is_empty() {
            ui.label(RichText::new(i18n.tr("osc-empty")).weak());
        }
        ui.separator();
        shown
    }

    /// The selected input's head: its address, kind and an axis's range, the last number heard
    /// and the button removing it. Returns a message for the status bar.
    pub fn input_editor(
        &self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        profile: &Profile,
        input: &OscInput,
        ops: &mut Vec<Op>,
    ) -> Option<String> {
        let mut status = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::BROADCAST).size(18.0));
            // Edited text lives in egui memory; it is applied on Enter / focus loss.
            let buf_id = egui::Id::unique(("osc-address", input.id));
            let mut edited: String = ui
                .data(|d| d.get_temp(buf_id))
                .unwrap_or_else(|| input.address.clone());
            let r = ui
                .add_sized([240.0, 28.0], style::text_field(&mut edited))
                .on_hover_text(i18n.tr("osc-address-tip"));
            if r.lost_focus() {
                ui.data_mut(|d| d.remove::<String>(buf_id));
                match gob_core::osc_address(&edited) {
                    Some(a) if a == input.address => {}
                    Some(a) if profile.osc_input_at(&a).is_some() => {
                        status = Some(i18n.trf("osc-address-taken", &[("address", a.into())]));
                    }
                    Some(address) => ops.push(Op::SetOscAddress {
                        id: input.id,
                        address,
                    }),
                    None => {}
                }
            } else if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(buf_id, edited));
            }
            let mut kind = input.kind;
            if kind_combo(ui, i18n, ("osc-kind", input.id), &mut kind) {
                ops.push(Op::SetOscKind { id: input.id, kind });
            }
            if style::icon_button(ui, true, icon::TRASH, &i18n.tr("osc-remove")).clicked() {
                ops.push(Op::RemoveOscInput(input.id));
            }
        });
        if input.kind == OscKind::Axis {
            ui.horizontal(|ui| {
                ui.label(i18n.tr("osc-range"))
                    .on_hover_text(i18n.tr("osc-range-tip"));
                let (mut low, mut high) = (input.low, input.high);
                let a = ui.add(
                    style::Spin::new(&mut low)
                        .range(-100_000.0..=100_000.0)
                        .decimals(2),
                );
                ui.label("…");
                let b = ui.add(
                    style::Spin::new(&mut high)
                        .range(-100_000.0..=100_000.0)
                        .decimals(2),
                );
                if (a.changed() || b.changed()) && low != high {
                    ops.push(Op::SetOscRange {
                        id: input.id,
                        low,
                        high,
                    });
                }
            });
        }
        let last = match self.value(&input.address) {
            None => i18n.tr("osc-not-heard"),
            Some(None) => i18n.tr("osc-heard-press"),
            Some(Some(v)) => i18n.trf("osc-heard", &[("value", format!("{v:.3}").into())]),
        };
        ui.label(RichText::new(last).weak());
        status
    }
}

/// What an address heard while listening is: a fader moving between the ends is an axis; a
/// message without a number, or with exactly 0 or 1, a button.
fn guess_kind(value: Option<f64>) -> OscKind {
    match value {
        Some(v) if v != 0.0 && v != 1.0 => OscKind::Axis,
        _ => OscKind::Button,
    }
}

/// Button / axis list; true when changed.
fn kind_combo(
    ui: &mut egui::Ui,
    i18n: &I18n,
    id: impl std::hash::Hash + std::fmt::Debug,
    kind: &mut OscKind,
) -> bool {
    let name = |k: OscKind| match k {
        OscKind::Button => format!("{}  {}", icon::RADIO_BUTTON, i18n.tr("osc-kind-button")),
        OscKind::Axis => format!("{}  {}", icon::SLIDERS_HORIZONTAL, i18n.tr("osc-kind-axis")),
    };
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(name(*kind))
        .show_ui(ui, |ui| {
            for k in [OscKind::Button, OscKind::Axis] {
                if ui.selectable_label(*kind == k, name(k)).clicked() && *kind != k {
                    *kind = k;
                    changed = true;
                }
            }
        })
        .response
        .on_hover_text(i18n.tr("osc-kind-tip"));
    changed
}

impl Editor<'_> {
    /// send-osc: where to, the address, the arguments and when.
    pub(crate) fn send_osc(
        &mut self,
        ui: &mut egui::Ui,
        action: &Action,
        host: &str,
        port: u16,
        address: &str,
        args: &[OscArg],
    ) {
        let i18n = self.i18n;
        let id = action.id.clone();
        let mut new_args: Option<Vec<OscArg>> = None;
        egui::Grid::new(("send-osc", &id))
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(i18n.tr("osc-send-to"))
                    .on_hover_text(i18n.tr("osc-send-to-tip"));
                ui.horizontal(|ui| {
                    let mut h = host.to_string();
                    let r = ui
                        .add(style::text_field(&mut h).desired_width(150.0))
                        .on_hover_text(i18n.tr("osc-send-to-tip"));
                    if r.changed() {
                        self.set(&id, "host", "string", h);
                    }
                    ui.label(":");
                    let mut p = port;
                    if ui
                        .add(style::Spin::new(&mut p).range(1..=65535))
                        .on_hover_text(i18n.tr("osc-send-port-tip"))
                        .changed()
                    {
                        self.set(&id, "port", "int", p.to_string());
                    }
                });
                ui.end_row();
                ui.label(i18n.tr("osc-address"));
                let mut a = address.to_string();
                let r = ui
                    .add(
                        style::text_field(&mut a)
                            .hint_text("/location/1/0/0/style/text")
                            .desired_width(320.0),
                    )
                    .on_hover_text(i18n.tr("osc-send-address-tip"));
                if r.changed() {
                    self.set(&id, "address", "string", a);
                }
                ui.end_row();
                ui.label(i18n.tr("osc-arguments"))
                    .on_hover_text(i18n.tr("osc-arguments-tip"));
                ui.vertical(|ui| {
                    for (i, arg) in args.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let mut kind = arg.kind;
                            egui::ComboBox::from_id_salt(("osc-arg", &id, i))
                                .selected_text(arg_kind_name(i18n, kind))
                                .show_ui(ui, |ui| {
                                    for k in OscArgKind::ALL {
                                        ui.selectable_value(&mut kind, k, arg_kind_name(i18n, k));
                                    }
                                });
                            let mut value = arg.value.clone();
                            let typed = kind.has_value()
                                && ui
                                    .add(style::text_field(&mut value).desired_width(160.0))
                                    .changed();
                            if kind != arg.kind || typed {
                                let mut all = args.to_vec();
                                all[i] = OscArg::new(kind, value);
                                new_args = Some(all);
                            }
                            let tip = i18n.tr("osc-argument-remove");
                            if style::icon_button(ui, true, icon::X, &tip).clicked() {
                                let mut all = args.to_vec();
                                all.remove(i);
                                new_args = Some(all);
                            }
                        });
                    }
                    let tip = i18n.tr("osc-argument-add");
                    if style::icon_button(ui, true, icon::PLUS, &tip).clicked() {
                        let mut all = args.to_vec();
                        all.push(OscArg::new(OscArgKind::Text, ""));
                        new_args = Some(all);
                    }
                });
                ui.end_row();
            });
        if let Some(args) = new_args {
            self.ops.push(Op::SetOscArgs { action: id, args });
        }
        ui.horizontal(|ui| self.activation(ui, action));
    }
}

fn arg_kind_name(i18n: &I18n, kind: OscArgKind) -> String {
    i18n.tr(&format!("osc-arg-{}", kind.name()))
}
