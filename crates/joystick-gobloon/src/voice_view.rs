//! The Voice tab's parts on the Devices page: the banner while no input listens, and a command's
//! phrases and hold time above its actions.

use eframe::egui::{self, RichText};
use gob_core::VoiceCommand;
use gob_i18n::I18n;

use crate::action_editor::Op;
use crate::style::{self, icon};

/// Width of a phrase field.
const PHRASE_WIDTH: f32 = 260.0;

/// Shown while no input has a voice-input action: commands are heard only while one is held.
pub fn no_button_banner(ui: &mut egui::Ui, i18n: &I18n) {
    let banner = egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(8)
        .inner_margin(8);
    banner.show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(icon::MICROPHONE_SLASH).size(style::ICON_SIZE));
            ui.label(i18n.tr("voice-no-button"));
        });
    });
    ui.add_space(4.0);
}

/// A command's title with its delete button, its phrases (edited in place, an empty field adds
/// one) and how long it holds its button.
pub fn command_editor(ui: &mut egui::Ui, i18n: &I18n, command: &VoiceCommand, ops: &mut Vec<Op>) {
    let id = command.id;
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::MICROPHONE).size(18.0));
        ui.heading(i18n.trf("voice-command-n", &[("n", id.into())]));
        if style::icon_button(ui, true, icon::TRASH, &i18n.tr("voice-remove")).clicked() {
            ops.push(Op::RemoveVoiceCommand(id));
        }
    });
    ui.label(RichText::new(i18n.tr("voice-phrases")).weak());
    let set = |ops: &mut Vec<Op>, phrases: Vec<String>| {
        ops.push(Op::SetVoicePhrases { id, phrases });
    };
    for (i, phrase) in command.phrases.iter().enumerate() {
        ui.horizontal(|ui| {
            if let Some(text) = phrase_field(ui, ("voice-phrase", id, i), phrase, "")
                && text.trim() != phrase
            {
                let mut phrases = command.phrases.clone();
                phrases[i] = text;
                set(ops, phrases);
            }
            let tip = i18n.tr("voice-phrase-remove");
            if style::icon_button(ui, true, icon::X, &tip).clicked() {
                let mut phrases = command.phrases.clone();
                phrases.remove(i);
                set(ops, phrases);
            }
        });
    }
    let hint = i18n.tr("voice-phrase-add");
    // A new field for every count, so it is empty again after a phrase was added.
    let key = ("voice-phrase-new", id, command.phrases.len());
    if let Some(text) = phrase_field(ui, key, "", &hint)
        && !text.trim().is_empty()
    {
        let mut phrases = command.phrases.clone();
        phrases.push(text);
        set(ops, phrases);
    }
    if command.phrases.is_empty() {
        ui.label(RichText::new(i18n.tr("voice-no-phrases")).weak());
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::CLOCK_COUNTDOWN).size(style::ICON_SIZE))
            .on_hover_text(i18n.tr("voice-hold"));
        let mut hold = command.hold;
        let spin = style::Spin::new(&mut hold)
            .range(0.01..=10.0)
            .step(0.05)
            .suffix(" s");
        if ui.add(spin).on_hover_text(i18n.tr("voice-hold")).changed() {
            ops.push(Op::SetVoiceHold { id, hold });
        }
    });
    ui.add_space(4.0);
}

/// A phrase field whose edit lives in egui memory until Enter or focus loss; returns the text
/// then.
fn phrase_field(
    ui: &mut egui::Ui,
    key: impl std::hash::Hash + std::fmt::Debug,
    text: &str,
    hint: &str,
) -> Option<String> {
    let buf_id = egui::Id::unique(key);
    let mut edited: String = ui
        .data(|d| d.get_temp(buf_id))
        .unwrap_or_else(|| text.to_string());
    let field = style::text_field(&mut edited).hint_text(hint);
    let r = ui.add_sized([PHRASE_WIDTH, 28.0], field);
    if r.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(buf_id));
        return Some(edited);
    }
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(buf_id, edited));
    }
    None
}
