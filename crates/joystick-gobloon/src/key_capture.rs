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

/// A key's name in the current keyboard layout, e.g. "Right Ctrl"; F13-F24 and the
/// [`NAMED_KEYS`] by their own names (layouts often have none for them).
pub fn name(key: gob_core::Key, i18n: &I18n) -> String {
    if let Some(n) = FUNCTION_KEYS.iter().position(|&k| k == key) {
        return format!("F{}", n + 13);
    }
    match NAMED_KEYS.iter().find(|(k, _)| *k == key) {
        Some((_, name)) => i18n.tr(name),
        None => crate::platform::key_name(key),
    }
}

const fn key(scan_code: u16, extended: bool) -> Key {
    Key {
        scan_code,
        extended,
    }
}

/// F13 to F24 (set 1 scan codes): many games take them, few keyboards have them.
const FUNCTION_KEYS: [Key; 12] = [
    key(0x64, false),
    key(0x65, false),
    key(0x66, false),
    key(0x67, false),
    key(0x68, false),
    key(0x69, false),
    key(0x6a, false),
    key(0x6b, false),
    key(0x6c, false),
    key(0x6d, false),
    key(0x6e, false),
    key(0x76, false),
];

/// Keys named by the program, not the keyboard layout (layouts often have no name for them),
/// with the i18n key of their name: media, browser, launch and power keys, the Japanese and
/// Korean layout keys and the numpad's = and ,.
const NAMED_KEYS: [(Key, &str); 30] = [
    (key(0x22, true), "key-play-pause"),
    (key(0x24, true), "key-media-stop"),
    (key(0x10, true), "key-previous-track"),
    (key(0x19, true), "key-next-track"),
    (key(0x20, true), "key-mute"),
    (key(0x2e, true), "key-volume-down"),
    (key(0x30, true), "key-volume-up"),
    (key(0x6a, true), "key-browser-back"),
    (key(0x69, true), "key-browser-forward"),
    (key(0x67, true), "key-browser-refresh"),
    (key(0x68, true), "key-browser-stop"),
    (key(0x32, true), "key-browser-home"),
    (key(0x65, true), "key-browser-search"),
    (key(0x66, true), "key-browser-favorites"),
    (key(0x6c, true), "key-mail"),
    (key(0x21, true), "key-calculator"),
    (key(0x6b, true), "key-my-computer"),
    (key(0x6d, true), "key-media-player"),
    (key(0x5f, true), "key-sleep"),
    (key(0x63, true), "key-wake"),
    (key(0x5e, true), "key-power"),
    (key(0x73, false), "key-ro"),
    (key(0x7d, false), "key-yen"),
    (key(0x7b, false), "key-muhenkan"),
    (key(0x79, false), "key-henkan"),
    (key(0x70, false), "key-kana"),
    (key(0x71, false), "key-hanja"),
    (key(0x72, false), "key-han-yeong"),
    (key(0x59, false), "key-num-equals"),
    (key(0x7e, false), "key-num-comma"),
];

fn scan_code_of(k: Key) -> style::ScanCode {
    style::ScanCode {
        code: k.scan_code,
        extended: k.extended,
    }
}

fn key_of(k: style::ScanCode) -> Key {
    Key {
        scan_code: k.code,
        extended: k.extended,
    }
}

/// A key cap legend from the current layout: only a single character fits ("A", "Ж", ";");
/// longer names keep the picker's own legend.
fn legend(k: Key) -> Option<String> {
    if FUNCTION_KEYS.contains(&k) || NAMED_KEYS.iter().any(|(m, _)| *m == k) {
        return None;
    }
    let name = crate::platform::key_name(k);
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_whitespace() => Some(c.to_uppercase().collect()),
        _ => None,
    }
}

/// A keyboard button opening a clickable full-size keyboard: a click adds a key to `keys`,
/// a second click removes it. The keyboard stays open until a click outside it.
fn keyboard(ui: &mut egui::Ui, keys: &[Key], i18n: &I18n) -> Option<Vec<Key>> {
    let button = style::icon_toggle(ui, false, icon::KEYBOARD, &i18n.tr("keys-keyboard"));
    let selected: Vec<style::ScanCode> = keys.iter().map(|&k| scan_code_of(k)).collect();
    let picked = egui::Popup::menu(&button)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            style::keyboard_picker(
                ui,
                "keys",
                &selected,
                |k| legend(key_of(k)),
                |k| name(key_of(k), i18n),
            )
        })
        .and_then(|r| r.inner)?;
    let k = key_of(picked);
    let mut keys = keys.to_vec();
    match keys.iter().position(|&x| x == k) {
        Some(i) => {
            keys.remove(i);
        }
        None => keys.push(k),
    }
    Some(keys)
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

/// A key that is down now and not one of `ignore`: from the system where it tells the held
/// keys, else from this frame's key events (a key held before makes none).
pub(crate) fn key_down(ui: &egui::Ui, ignore: &[Key]) -> Option<Key> {
    if let Some(held) = crate::platform::held_keys() {
        return held.into_iter().find(|k| !ignore.contains(k));
    }
    ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                repeat: false,
                ..
            } => scan_code(physical_key.unwrap_or(*key)),
            _ => None,
        })
    })
}

fn recording_id() -> egui::Id {
    egui::Id::unique("input-recording")
}

/// How long after the last recording frame inputs still do not move the Devices page: the
/// press that ends a recording may reach the event queue a few frames later.
const RECORDING_GRACE: f64 = 0.5;

/// An editor records an input this frame: pressing one must not move the Devices page
/// to it (the editor would go away mid-recording).
pub(crate) fn mark_recording(ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(recording_id(), now));
}

/// An editor recorded an input within [`RECORDING_GRACE`].
pub(crate) fn recording(ctx: &egui::Context) -> bool {
    let now = ctx.input(|i| i.time);
    ctx.data(|d| d.get_temp::<f64>(recording_id()))
        .is_some_and(|t| now - t < RECORDING_GRACE)
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
            chip.show(ui, |ui| ui.label(name(*k, i18n)));
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
                if let Some(keys) = keyboard(ui, keys, i18n) {
                    result = Some(keys);
                }
                if !keys.is_empty()
                    && style::icon_button(ui, true, icon::BACKSPACE, &i18n.tr("keys-clear"))
                        .clicked()
                {
                    result = Some(Vec::new());
                }
            }
            Some(mut rec) => {
                mark_recording(ui.ctx());
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
