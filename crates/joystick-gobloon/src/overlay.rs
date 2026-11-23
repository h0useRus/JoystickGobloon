//! The game overlay: a small window above the game (borderless or windowed; exclusive full
//! screen draws over every window) showing the active mode and layer, the description of the
//! last pressed binding, the latency, the recognized voice command and the profile's voice
//! commands. It never takes clicks or focus; while it is being placed (Settings, Overlay,
//! Calibrate) it can be dragged. A key combination (Ctrl+Alt+F unless changed) shows and hides
//! it, also while a game has the focus.
//!
//! The main window builds the overlay's lines in `logic` (which runs while it is in the tray
//! too); the overlay window, a viewport of its own, only draws them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};
use gob_core::{Key, Profile};
use gob_engine::RunnerStatus;
use gob_i18n::I18n;

use crate::settings::Settings;
use crate::style::icon;

/// Where the overlay starts before it is placed: near the top left of the main screen.
const DEFAULT_POS: [f32; 2] = [40.0, 40.0];

/// How long after the key thread could not start it is tried again.
const KEY_RETRY: Duration = Duration::from_secs(5);

/// How often the key combination is looked for.
const KEY_POLL: Duration = Duration::from_millis(25);

/// Ctrl+Alt+F (left Ctrl, left Alt, F as set 1 scan codes).
pub const DEFAULT_KEYS: [u16; 3] = [0x1d, 0x38, 0x21];

fn viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("game-overlay")
}

/// A key as settings store it: the scan code, with 0x100 for the E0 prefix.
pub fn key_from_code(code: u16) -> Key {
    Key {
        scan_code: code & 0xff,
        extended: code & 0x100 != 0,
    }
}

pub fn key_code(key: Key) -> u16 {
    key.scan_code | if key.extended { 0x100 } else { 0 }
}

/// Whether every key of `combo` is among `held`.
#[cfg(test)]
fn combo_held(combo: &[Key], held: &[Key]) -> bool {
    combo_matches(combo, |k| held.contains(&k))
}

/// The key and its other side's twin (left and right Ctrl, Alt and Shift count as one).
fn both_sides(k: Key) -> [Key; 2] {
    let key = |scan_code, extended| Key {
        scan_code,
        extended,
    };
    match (k.scan_code, k.extended) {
        (0x1d, _) => [key(0x1d, false), key(0x1d, true)],
        (0x38, _) => [key(0x38, false), key(0x38, true)],
        (0x2a, false) | (0x36, false) => [key(0x2a, false), key(0x36, false)],
        _ => [k, k],
    }
}

/// Whether every key of `combo` is held (either side of Ctrl, Alt and Shift), by `is_held`.
fn combo_matches(combo: &[Key], is_held: impl Fn(Key) -> bool) -> bool {
    !combo.is_empty()
        && combo
            .iter()
            .all(|&k| both_sides(k).iter().any(|&side| is_held(side)))
}

/// Whether the combination is held now, asking the system only for its keys.
fn combo_down(combo: &[Key]) -> bool {
    combo_matches(combo, |k| crate::platform::key_held(k) == Some(true))
}

/// One line of the overlay.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    glyph: &'static str,
    text: String,
    /// Indent steps (voice commands in groups).
    depth: usize,
    /// A voice group's name.
    strong: bool,
}

/// What the overlay window draws.
#[derive(Clone, Debug, Default, PartialEq)]
struct Frame {
    lines: Vec<Line>,
    text: Color32,
    background: Color32,
    font_size: f32,
    /// Being placed: draggable, with a border.
    calibrating: bool,
}

/// Shared between the main window, the overlay window and the key thread.
#[derive(Default)]
struct Shared {
    frame: Mutex<Frame>,
    /// Where the overlay window is now (its outer top left), told by the overlay window.
    position: Mutex<Option<egui::Pos2>>,
    /// The key combination, and whether it is looked for.
    keys: Mutex<Vec<Key>>,
    listening: AtomicBool,
    /// Flipped by the key combination.
    hidden: AtomicBool,
}

pub struct Overlay {
    shared: Arc<Shared>,
    key_thread: bool,
    /// When the key thread could not start last (it is tried again a while later), and whether
    /// that was logged.
    key_tried: Option<Instant>,
    key_warned: bool,
    /// The key combination last handed to the key thread.
    keys: Vec<u16>,
    /// The voice command lines, made again only when the profile's commands change.
    commands: CommandLines,
    /// Being placed (Settings, Overlay, Calibrate).
    pub calibrating: bool,
    /// [`Self::calibrating`] in the last frame: placing ended when it goes off.
    was_calibrating: bool,
}

impl Overlay {
    pub fn new() -> Self {
        Self {
            shared: Arc::default(),
            key_thread: false,
            key_tried: None,
            key_warned: false,
            keys: Vec::new(),
            commands: CommandLines::default(),
            calibrating: false,
            was_calibrating: false,
        }
    }

    /// Shows, updates or closes the overlay window; called every frame from `logic`. Returns
    /// where it was dragged once placing ends (Done, or another tab). While it is dragged the
    /// window's position is left to the system: setting it from here too would fight the drag.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        i18n: &I18n,
        settings: &Settings,
        status: Option<&RunnerStatus>,
        profile: Option<&Profile>,
        usage: Option<crate::usage::Reading>,
        recording_keys: bool,
    ) -> Option<[f32; 2]> {
        if self.keys != settings.overlay_keys {
            self.keys.clone_from(&settings.overlay_keys);
            let keys: Vec<Key> = self.keys.iter().map(|&c| key_from_code(c)).collect();
            *self.shared.keys.lock().unwrap_or_else(|e| e.into_inner()) = keys;
        }
        // Not while the combination itself is being recorded in the settings.
        self.shared
            .listening
            .store(settings.overlay_on && !recording_keys, Ordering::Relaxed);
        // The key state may come later (Linux: once the program may read the keyboard).
        if settings.overlay_on
            && !self.key_thread
            && self.key_tried.is_none_or(|t| t.elapsed() >= KEY_RETRY)
        {
            match spawn_key_thread(ctx.clone(), self.shared.clone()) {
                Ok(()) => self.key_thread = true,
                Err(e) => {
                    if !self.key_warned {
                        log::warn!("overlay: {e}; its key combination works once it can");
                        self.key_warned = true;
                    }
                    self.key_tried = Some(Instant::now());
                }
            }
        }

        let position = || {
            self.shared
                .position
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
        };
        let placed = match (self.was_calibrating, self.calibrating) {
            (true, false) => position()
                .map(|p| [p.x.round(), p.y.round()])
                .filter(|p| Some(*p) != settings.overlay_pos),
            (false, true) => {
                // No position from an earlier placing.
                position();
                None
            }
            _ => None,
        };
        self.was_calibrating = self.calibrating;

        let hidden = self.shared.hidden.load(Ordering::Relaxed);
        let lines = if self.calibrating && status.is_none() {
            sample_lines(i18n, settings)
        } else {
            let commands = &mut self.commands;
            status.map_or_else(Vec::new, |s| lines(settings, s, profile, usage, commands))
        };
        let shown = self.calibrating || (settings.overlay_on && !hidden && !lines.is_empty());
        if !shown {
            return placed;
        }
        let opacity = settings.overlay_opacity.clamp(0.1, 1.0);
        let frame = Frame {
            lines,
            text: color(settings.overlay_text_color).gamma_multiply(opacity),
            background: color(settings.overlay_background).gamma_multiply(opacity),
            font_size: settings.overlay_font_size.clamp(8.0, 72.0),
            calibrating: self.calibrating,
        };
        let changed = {
            let mut f = self.shared.frame.lock().unwrap_or_else(|e| e.into_inner());
            let changed = *f != frame;
            *f = frame;
            changed
        };
        if changed {
            ctx.request_repaint_of(viewport());
        }

        let pos = settings.overlay_pos.unwrap_or(DEFAULT_POS);
        let builder = egui::ViewportBuilder::default()
            .with_title("Joystick Gobloon overlay")
            .with_position(egui::pos2(pos[0], pos[1]))
            .with_inner_size([10.0, 10.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_active(false)
            .with_mouse_passthrough(!self.calibrating);
        let shared = self.shared.clone();
        ctx.show_viewport_deferred(viewport(), builder, move |ui, _class| {
            draw(ui, &shared);
        });

        placed
    }
}

/// Settings, Overlay: on or off, its key combination, placing it, what it shows and how it
/// looks. True when a setting changed.
pub fn settings_tab(
    ui: &mut egui::Ui,
    i18n: &I18n,
    settings: &mut Settings,
    overlay: &mut Overlay,
) -> bool {
    use crate::settings_view::section_title;
    use crate::style;
    let mut changed = false;
    section_title(ui, icon::PICTURE_IN_PICTURE, &i18n.tr("settings-overlay"));
    ui.label(RichText::new(i18n.tr("settings-overlay-hint")).weak());
    ui.add_space(4.0);
    changed |= ui
        .add(style::switch(
            &mut settings.overlay_on,
            i18n.tr("settings-overlay-on"),
        ))
        .changed();
    ui.horizontal(|ui| {
        let keys: Vec<Key> = settings
            .overlay_keys
            .iter()
            .map(|&c| key_from_code(c))
            .collect();
        let id = ui.scope_id().with("overlay-keys");
        if let Some(new) = crate::key_capture::keys_field(ui, id, &keys, i18n) {
            settings.overlay_keys = new.into_iter().map(key_code).collect();
            changed = true;
        }
        ui.label(i18n.tr("settings-overlay-keys"));
    });
    ui.add_space(4.0);
    let (glyph, key) = if overlay.calibrating {
        (icon::CHECK, "settings-overlay-done")
    } else {
        (icon::ARROWS_OUT_CARDINAL, "settings-overlay-place")
    };
    let button = egui::Button::new(format!("{glyph}  {}", i18n.tr(key)));
    if ui
        .add(button)
        .on_hover_text(i18n.tr("settings-overlay-place-tip"))
        .clicked()
    {
        overlay.calibrating = !overlay.calibrating;
        // The place is kept when it is done.
        changed |= !overlay.calibrating;
    }
    if overlay.calibrating {
        // Here, not on the overlay: it keeps the size it has while playing.
        let hint = format!("{}  {}", icon::ARROWS_OUT_CARDINAL, i18n.tr("overlay-drag"));
        ui.label(RichText::new(hint).weak());
    }

    ui.add_space(18.0);
    section_title(ui, icon::LIST_CHECKS, &i18n.tr("settings-overlay-show"));
    for (value, key) in [
        (&mut settings.overlay_mode, "settings-overlay-mode"),
        (&mut settings.overlay_action, "settings-overlay-action"),
        (&mut settings.overlay_latency, "settings-overlay-latency"),
        (&mut settings.overlay_usage, "settings-overlay-usage"),
        (&mut settings.overlay_voice, "settings-overlay-voice"),
        (&mut settings.overlay_commands, "settings-overlay-commands"),
    ] {
        changed |= ui.add(style::switch(value, i18n.tr(key))).changed();
    }

    ui.add_space(18.0);
    section_title(ui, icon::PALETTE, &i18n.tr("settings-overlay-look"));
    for (rgba, key) in [
        (&mut settings.overlay_text_color, "settings-overlay-text"),
        (
            &mut settings.overlay_background,
            "settings-overlay-background",
        ),
    ] {
        ui.horizontal(|ui| {
            let mut c = color(*rgba);
            let r = egui::color_picker::color_edit_button_srgba(
                ui,
                &mut c,
                egui::color_picker::Alpha::OnlyBlend,
            );
            if r.changed() {
                *rgba = c.to_srgba_unmultiplied();
                changed = true;
            }
            ui.label(i18n.tr(key));
        });
    }
    ui.horizontal(|ui| {
        let mut size = settings.overlay_font_size.round().clamp(8.0, 72.0) as u16;
        if style::slider(ui, &mut size, 8..=72, 1.0, " pt").changed() {
            settings.overlay_font_size = f32::from(size);
            changed = true;
        }
        ui.label(i18n.tr("settings-overlay-size"));
    });
    ui.horizontal(|ui| {
        let mut percent = (settings.overlay_opacity.clamp(0.1, 1.0) * 100.0).round() as u16;
        if style::slider(ui, &mut percent, 10..=100, 1.0, " %").changed() {
            settings.overlay_opacity = f32::from(percent) / 100.0;
            changed = true;
        }
        ui.label(i18n.tr("settings-overlay-opacity"));
    });
    changed
}

fn color(rgba: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// The overlay's lines for a running profile.
fn lines(
    settings: &Settings,
    status: &RunnerStatus,
    profile: Option<&Profile>,
    usage: Option<crate::usage::Reading>,
    commands: &mut CommandLines,
) -> Vec<Line> {
    let line = |glyph, text: String| Line {
        glyph,
        text,
        depth: 0,
        strong: false,
    };
    let mut out = Vec::new();
    if settings.overlay_mode {
        let text = if status.layer.is_empty() {
            status.mode.clone()
        } else {
            format!("{}  ·  {}", status.mode, status.layer)
        };
        out.push(line(icon::STACK, text));
    }
    if settings.overlay_action
        && let Some(action) = &status.last_action
    {
        out.push(line(icon::LIGHTNING, action.clone()));
    }
    if settings.overlay_latency
        && let Some(latency) = status.latency
    {
        out.push(line(
            icon::TIMER,
            format!("{:.2} ms", latency.average.as_secs_f64() * 1e3),
        ));
    }
    if settings.overlay_usage
        && let Some(r) = usage
    {
        out.push(line(icon::MEMORY, r.text()));
    }
    if settings.overlay_voice
        && let Some(p) = profile
        && let Some(id) = status
            .voice
            .as_ref()
            .and_then(|v| v.heard.as_ref())
            .and_then(|h| h.command)
    {
        out.push(line(icon::MICROPHONE, p.voice_full_phrase(id)));
    }
    if settings.overlay_commands
        && let Some(p) = profile
    {
        out.extend_from_slice(commands.of(p));
    }
    out
}

/// Every voice command of a profile as overlay lines, indented by group; kept with the commands
/// they were made from (the tree is costly to walk every frame).
#[derive(Default)]
struct CommandLines {
    from: Vec<gob_core::VoiceCommand>,
    lines: Vec<Line>,
}

impl CommandLines {
    fn of(&mut self, p: &Profile) -> &[Line] {
        if self.from != p.voice_commands {
            self.from.clone_from(&p.voice_commands);
            self.lines = p
                .voice_tree()
                .into_iter()
                .filter_map(|(depth, c)| {
                    Some(Line {
                        glyph: if c.group {
                            icon::FOLDER_SIMPLE
                        } else {
                            icon::CHAT_CIRCLE
                        },
                        text: c.phrases.first()?.clone(),
                        depth,
                        strong: c.group,
                    })
                })
                .collect();
        }
        &self.lines
    }
}

/// What the chosen lines look like, while the overlay is placed with no profile running.
fn sample_lines(i18n: &I18n, settings: &Settings) -> Vec<Line> {
    let line = |glyph, text: String| Line {
        glyph,
        text,
        depth: 0,
        strong: false,
    };
    let mut out = Vec::new();
    if settings.overlay_mode {
        out.push(line(icon::STACK, i18n.tr("overlay-sample-mode")));
    }
    if settings.overlay_action {
        out.push(line(icon::LIGHTNING, i18n.tr("overlay-sample-action")));
    }
    if settings.overlay_latency {
        out.push(line(icon::TIMER, "0.42 ms".into()));
    }
    if settings.overlay_usage {
        let r = crate::usage::Reading {
            mb: 120,
            cpu: Some(0.3),
        };
        out.push(line(icon::MEMORY, r.text()));
    }
    if settings.overlay_voice || settings.overlay_commands {
        out.push(line(icon::MICROPHONE, i18n.tr("overlay-sample-voice")));
    }
    if out.is_empty() {
        out.push(line(icon::ARROWS_OUT_CARDINAL, i18n.tr("overlay-drag")));
    }
    out
}

/// The overlay window's drawing: the lines on the background, sized to them.
fn draw(root: &mut egui::Ui, shared: &Shared) {
    let ctx = root.ctx().clone();
    let frame = shared
        .frame
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let panel = egui::Frame::NONE.fill(Color32::TRANSPARENT);
    egui::CentralPanel::default().frame(panel).show(root, |ui| {
        // Nothing on it reacts to the mouse but the drag while it is placed.
        ui.style_mut().interaction.selectable_labels = false;
        // While placed: a border in the text color, visible on any background. Its width is
        // kept while playing (unseen), so the overlay has the same size in both.
        let stroke = if frame.calibrating {
            egui::Stroke::new(2.0, frame.text.gamma_multiply(0.8))
        } else {
            egui::Stroke::new(2.0, Color32::TRANSPARENT)
        };
        let inner = egui::Frame::new()
            .fill(frame.background)
            .stroke(stroke)
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = frame.font_size * 0.25;
                for l in &frame.lines {
                    ui.horizontal(|ui| {
                        ui.add_space(l.depth as f32 * frame.font_size);
                        let text = RichText::new(format!("{}  {}", l.glyph, l.text))
                            .size(frame.font_size)
                            .color(frame.text);
                        let text = if l.strong { text.strong() } else { text };
                        ui.add(egui::Label::new(text).extend().selectable(false));
                    });
                }
            });
        let rect = inner.response.rect;
        // The window follows its content.
        let want = rect.max - ui.min_rect().min + egui::vec2(1.0, 1.0);
        let have = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
        if have.is_none_or(|h| (h - want).length() > 1.0) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(want));
        }
        if frame.calibrating {
            let drag = ui.interact(rect, ui.scope_id().with("drag"), egui::Sense::drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            let pos = ctx.input(|i| i.viewport().outer_rect.map(|r| r.min));
            *shared.position.lock().unwrap_or_else(|e| e.into_inner()) = pos;
            // The position follows the drag.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    });
}

/// Watches the key combination on a thread of its own (the game has the focus, so the
/// window gets no key events); each press flips the overlay. An error when the system does not
/// tell which keys are held (yet) or the thread did not start.
fn spawn_key_thread(ctx: egui::Context, shared: Arc<Shared>) -> Result<(), String> {
    if crate::platform::held_keys().is_none() {
        return Err("the system does not tell which keys are held".into());
    }
    let spawned = std::thread::Builder::new()
        .name("overlay-keys".into())
        .spawn(move || {
            let mut down = false;
            loop {
                std::thread::sleep(KEY_POLL);
                if !shared.listening.load(Ordering::Relaxed) {
                    down = false;
                    continue;
                }
                let combo = shared
                    .keys
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let now = combo_down(&combo);
                if now && !down {
                    let hidden = !shared.hidden.load(Ordering::Relaxed);
                    shared.hidden.store(hidden, Ordering::Relaxed);
                    log::debug!("overlay {}", if hidden { "hidden" } else { "shown" });
                    ctx.request_repaint();
                }
                down = now;
            }
        });
    spawned
        .map(|_| ())
        .map_err(|e| format!("its key thread did not start: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_as_stored() {
        for k in [
            Key {
                scan_code: 0x21,
                extended: false,
            },
            Key {
                scan_code: 0x38,
                extended: true,
            },
        ] {
            assert_eq!(key_from_code(key_code(k)), k);
        }
    }

    #[test]
    fn either_ctrl_alt_or_shift_counts() {
        let k = |scan_code, extended| Key {
            scan_code,
            extended,
        };
        let combo: Vec<Key> = DEFAULT_KEYS.iter().map(|&c| key_from_code(c)).collect();
        assert!(combo_held(
            &combo,
            &[k(0x1d, false), k(0x38, false), k(0x21, false)]
        ));
        // Right Ctrl and right Alt (AltGr), and another key held too.
        assert!(combo_held(
            &combo,
            &[k(0x1d, true), k(0x38, true), k(0x21, false), k(0x1e, false)]
        ));
        assert!(!combo_held(&combo, &[k(0x1d, false), k(0x21, false)]));
        assert!(!combo_held(&[], &[k(0x21, false)]));
        // Right Shift for left Shift.
        assert!(combo_held(&[k(0x2a, false)], &[k(0x36, false)]));
    }
}
