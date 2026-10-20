//! Recording keyboard keys as Gremlin stores them (scan code + extended flag).
//!
//! While recording, the real key states are polled from the system where it allows that
//! (left and right modifiers apart, exact scan codes). Elsewhere the window's key events are
//! used: egui's physical keys become scan codes, and Ctrl, Shift and Alt count as the left
//! ones (egui does not tell them apart).

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
    /// Keys the window saw go down and not up yet (without the system's key state).
    down: Vec<Key>,
}

/// Set 1 scan code of an egui key on a US layout (egui's physical keys are named that way).
fn scan_code(key: egui::Key) -> Option<Key> {
    use egui::Key as K;
    let plain = |scan_code| {
        Some(Key {
            scan_code,
            extended: false,
        })
    };
    let extended = |scan_code| {
        Some(Key {
            scan_code,
            extended: true,
        })
    };
    let letters = [
        (K::A, 0x1e),
        (K::B, 0x30),
        (K::C, 0x2e),
        (K::D, 0x20),
        (K::E, 0x12),
        (K::F, 0x21),
        (K::G, 0x22),
        (K::H, 0x23),
        (K::I, 0x17),
        (K::J, 0x24),
        (K::K, 0x25),
        (K::L, 0x26),
        (K::M, 0x32),
        (K::N, 0x31),
        (K::O, 0x18),
        (K::P, 0x19),
        (K::Q, 0x10),
        (K::R, 0x13),
        (K::S, 0x1f),
        (K::T, 0x14),
        (K::U, 0x16),
        (K::V, 0x2f),
        (K::W, 0x11),
        (K::X, 0x2d),
        (K::Y, 0x15),
        (K::Z, 0x2c),
    ];
    if let Some(&(_, code)) = letters.iter().find(|(k, _)| *k == key) {
        return plain(code);
    }
    let digits = [
        K::Num1,
        K::Num2,
        K::Num3,
        K::Num4,
        K::Num5,
        K::Num6,
        K::Num7,
        K::Num8,
        K::Num9,
        K::Num0,
    ];
    if let Some(i) = digits.iter().position(|k| *k == key) {
        return plain(0x02 + i as u16);
    }
    let f_keys = [
        K::F1,
        K::F2,
        K::F3,
        K::F4,
        K::F5,
        K::F6,
        K::F7,
        K::F8,
        K::F9,
        K::F10,
    ];
    if let Some(i) = f_keys.iter().position(|k| *k == key) {
        return plain(0x3b + i as u16);
    }
    match key {
        K::Escape => plain(0x01),
        K::Minus => plain(0x0c),
        K::Equals | K::Plus => plain(0x0d),
        K::Backspace => plain(0x0e),
        K::Tab => plain(0x0f),
        K::OpenBracket => plain(0x1a),
        K::CloseBracket => plain(0x1b),
        K::Enter => plain(0x1c),
        K::Semicolon => plain(0x27),
        K::Quote => plain(0x28),
        K::Backtick => plain(0x29),
        K::Backslash => plain(0x2b),
        K::Comma => plain(0x33),
        K::Period => plain(0x34),
        K::Slash => plain(0x35),
        K::Space => plain(0x39),
        K::F11 => plain(0x57),
        K::F12 => plain(0x58),
        K::Home => extended(0x47),
        K::ArrowUp => extended(0x48),
        K::PageUp => extended(0x49),
        K::ArrowLeft => extended(0x4b),
        K::ArrowRight => extended(0x4d),
        K::End => extended(0x4f),
        K::ArrowDown => extended(0x50),
        K::PageDown => extended(0x51),
        K::Insert => extended(0x52),
        K::Delete => extended(0x53),
        _ => None,
    }
}

/// Keys held according to the window's events since recording started (`rec.down` keeps
/// them between frames); Ctrl, Shift and Alt from the modifiers, as the left keys.
fn window_keys(ui: &egui::Ui, rec: &mut Recording) -> Vec<Key> {
    ui.input(|i| {
        for event in &i.events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat: false,
                ..
            } = event
                && let Some(k) = scan_code(physical_key.unwrap_or(*key))
            {
                rec.down.retain(|d| *d != k);
                if *pressed {
                    rec.down.push(k);
                }
            }
        }
        let modifiers = [
            (i.modifiers.ctrl, 0x1d),
            (i.modifiers.shift, 0x2a),
            (i.modifiers.alt, 0x38),
        ];
        let mut held: Vec<Key> = modifiers
            .iter()
            .filter(|(on, _)| *on)
            .map(|&(_, scan_code)| Key {
                scan_code,
                extended: false,
            })
            .collect();
        held.extend(rec.down.iter().copied());
        held
    })
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
                let held =
                    crate::platform::held_keys().unwrap_or_else(|| window_keys(ui, &mut rec));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egui_keys_as_scan_codes() {
        let key = |scan_code, extended| {
            Some(Key {
                scan_code,
                extended,
            })
        };
        assert_eq!(scan_code(egui::Key::A), key(0x1e, false));
        assert_eq!(scan_code(egui::Key::Q), key(0x10, false));
        assert_eq!(scan_code(egui::Key::Num0), key(0x0b, false));
        assert_eq!(scan_code(egui::Key::F10), key(0x44, false));
        assert_eq!(scan_code(egui::Key::F12), key(0x58, false));
        assert_eq!(scan_code(egui::Key::ArrowUp), key(0x48, true));
        assert_eq!(scan_code(egui::Key::Copy), None);
    }
}
