//! Recording keyboard keys as Gremlin stores them (scan code + extended flag).
//!
//! egui only reports logical keys, so while recording the real key states are polled
//! from Windows: that tells left from right modifiers and gives the exact scan codes.

use eframe::egui::{self, RichText};
use gob_core::Key;
use gob_i18n::I18n;

use crate::style::{self, icon};

#[cfg(windows)]
mod os {
    use gob_core::Key;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetKeyNameTextW, MapVirtualKeyW, MAPVK_VK_TO_VSC_EX,
    };

    /// Keys held right now. Mouse buttons and the side-neutral Shift/Ctrl/Alt codes are
    /// skipped (their left/right versions are reported instead).
    pub fn held() -> Vec<Key> {
        let mut keys = Vec::new();
        for vk in 0x08..=0xFEi32 {
            if matches!(vk, 0x10..=0x12) {
                continue;
            }
            // SAFETY: plain Win32 state queries.
            let down = unsafe { GetAsyncKeyState(vk) } < 0;
            if !down {
                continue;
            }
            let code = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
            let scan_code = (code & 0xFF) as u16;
            if scan_code == 0 {
                continue;
            }
            let key = Key {
                scan_code,
                extended: code >> 8 == 0xE0,
            };
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys
    }

    /// Name in the current keyboard layout, e.g. "Right Ctrl".
    pub fn name(key: Key) -> String {
        let lparam = (i32::from(key.scan_code) << 16) | (i32::from(key.extended) << 24);
        let mut buf = [0u16; 64];
        // SAFETY: the buffer outlives the call; the length is passed with it.
        let n = unsafe { GetKeyNameTextW(lparam, &mut buf) };
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            fallback(key)
        }
    }

    fn fallback(key: Key) -> String {
        format!(
            "{}{:#04x}",
            if key.extended { "E0 " } else { "" },
            key.scan_code
        )
    }
}

#[cfg(not(windows))]
mod os {
    use gob_core::Key;

    pub fn held() -> Vec<Key> {
        Vec::new()
    }

    pub fn name(key: Key) -> String {
        format!(
            "{}{:#04x}",
            if key.extended { "E0 " } else { "" },
            key.scan_code
        )
    }
}

pub use os::name;

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
                let held = os::held();
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_names_keys_left_and_right() {
        let l_ctrl = name(Key {
            scan_code: 0x1D,
            extended: false,
        });
        let r_ctrl = name(Key {
            scan_code: 0x1D,
            extended: true,
        });
        println!("{l_ctrl} / {r_ctrl}");
        assert!(!l_ctrl.contains("0x") && !r_ctrl.contains("0x"));
        assert_ne!(l_ctrl, r_ctrl, "the E0 flag tells right from left");
    }
}
