//! Recording keyboard keys as Gremlin stores them (scan code + extended flag).
//!
//! egui only reports logical keys, so while recording the real key states are polled
//! from Windows: that tells left from right modifiers and gives the exact scan codes.

use eframe::egui::{self, RichText};
use gob_core::Key;
use gob_i18n::I18n;

use crate::style::{self, icon};

/// A key's name in the current keyboard layout, e.g. "Right Ctrl".
pub fn name(key: gob_core::Key) -> String {
    crate::platform::key_name(key)
}

/// Recording in progress: keys in the order they went down.
#[derive(Clone, Default)]
struct Recording {
    keys: Vec<Key>,
}

/// Keys as text chips plus a record button. Recording collects every key pressed until
/// all are released again; the result replaces the keys. Returns the new keys once.
pub fn keys_field(ui: &mut egui::Ui, id: egui::Id, keys: &[Key], i18n: &I18n) -> Option<Vec<Key>> {
    let recording: Option<Recording> = ui.data(|d| d.get_temp(id));
    let mut result = None;
    ui.horizontal(|ui| {
        let shown: Vec<Key> = recording.as_ref().map_or(keys.to_vec(), |r| r.keys.clone());
        if shown.is_empty() {
            let hint = if recording.is_some() {
                i18n.tr("keys-press")
            } else {
                i18n.tr("keys-none")
            };
            ui.label(RichText::new(hint).weak());
        }
        for (n, k) in shown.iter().enumerate() {
            if n > 0 {
                ui.label(RichText::new("+").weak());
            }
            let chip = egui::Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(6, 2));
            chip.show(ui, |ui| ui.label(name(*k)));
        }
        match recording {
            None => {
                if style::record_button(ui, &i18n.tr("keys-record")).clicked() {
                    // Typing must not reach a focused text field meanwhile.
                    ui.memory_mut(|m| {
                        if let Some(f) = m.focused() {
                            m.surrender_focus(f);
                        }
                    });
                    ui.data_mut(|d| d.insert_temp(id, Recording::default()));
                }
            }
            Some(mut rec) => {
                let held = crate::platform::held_keys();
                for k in &held {
                    if !rec.keys.contains(k) {
                        rec.keys.push(*k);
                    }
                }
                let cancel =
                    style::icon_button(ui, true, icon::X, &i18n.tr("keys-cancel")).clicked();
                if cancel {
                    ui.data_mut(|d| d.remove::<Recording>(id));
                } else if held.is_empty() && !rec.keys.is_empty() {
                    ui.data_mut(|d| d.remove::<Recording>(id));
                    result = Some(rec.keys);
                } else {
                    ui.data_mut(|d| d.insert_temp(id, rec));
                    ui.ctx().request_repaint();
                }
            }
        }
    });
    result
}
