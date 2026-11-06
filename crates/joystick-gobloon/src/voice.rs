//! Voice commands in the program: where libvosk and the speech models live (libvosk next to the
//! exe or in `<data folder>/voice`, models downloaded into `<data folder>/voice` by gob-voice's
//! `models`), and the Voice tab of the settings.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use gob_engine::VoiceConfig;
use gob_i18n::I18n;
use gob_voice::ModelInfo;

use crate::settings::Settings;
use crate::settings_view::section_title;
use crate::style::{self, icon};

/// The folder of the speech models (and of libvosk when it is not next to the exe).
pub fn dir() -> Option<PathBuf> {
    crate::settings::data_dir().map(|d| d.join("voice"))
}

/// Folders libvosk is looked for in.
fn library_dirs() -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from));
    exe_dir.into_iter().chain(dir()).collect()
}

/// The language voice commands are spoken in: the chosen one, else the program's.
fn language(settings: &Settings) -> &str {
    if settings.voice_language.is_empty() {
        &settings.language
    } else {
        &settings.voice_language
    }
}

/// How voice commands run with these settings; `None` while the model of their language is not
/// installed (voice-input actions then do nothing).
pub fn config(settings: &Settings) -> Option<VoiceConfig> {
    let dir = dir()?;
    let language = language(settings);
    let Some(model) = gob_voice::models::for_language(language) else {
        log::info!("voice commands: no speech model for {language}");
        return None;
    };
    let Some(path) = gob_voice::models::installed(&dir, model) else {
        log::info!(
            "voice commands: speech model {} not installed in {}",
            model.name,
            dir.display()
        );
        return None;
    };
    Some(VoiceConfig {
        library_dirs: library_dirs(),
        model: path,
        gain: settings.voice_gain,
        min_confidence: settings.voice_min_confidence,
        keep_open: settings.voice_keep_open,
    })
}

/// The microphones, listed on a thread of its own (listing them can take a while with Bluetooth
/// or USB headsets): `None` until the first list is there. `fresh` asks for a new list when the
/// last one is older than a few seconds; it comes with a repaint.
pub fn microphones(ctx: &egui::Context, fresh: bool) -> Option<Arc<Vec<gob_voice::Microphone>>> {
    const MAX_AGE: Duration = Duration::from_secs(3);
    struct List {
        microphones: Option<Arc<Vec<gob_voice::Microphone>>>,
        listed: Option<Instant>,
        listing: bool,
    }
    static LIST: Mutex<List> = Mutex::new(List {
        microphones: None,
        listed: None,
        listing: false,
    });
    let lock = || LIST.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = lock();
    let stale = list
        .listed
        .is_none_or(|at| fresh && at.elapsed() >= MAX_AGE);
    if stale && !list.listing {
        list.listing = true;
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("microphones".into())
            .spawn(move || {
                let found = Arc::new(gob_voice::microphones());
                let mut list = lock();
                list.microphones = Some(found);
                list.listed = Some(Instant::now());
                list.listing = false;
                ctx.request_repaint();
            });
        if spawned.is_err() {
            list.listing = false;
        }
    }
    list.microphones.clone()
}

/// Whether libvosk loads, checked once per run.
fn library_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| match gob_voice::Vosk::load(&library_dirs()) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("voice commands: {e}");
            false
        }
    })
}

/// A speech model download running on its own thread.
struct Download {
    model: &'static ModelInfo,
    /// Bytes so far, total.
    progress: Arc<Mutex<(u64, u64)>>,
    cancel: Arc<AtomicBool>,
    done: Receiver<Result<(), String>>,
}

/// The microphone open for the level meter.
struct MicTest {
    capture: gob_voice::Capture,
    /// Microphone id; empty = the default one.
    microphone: String,
    /// The meter falls back slowly from a peak.
    shown: f32,
}

/// The level meter's microphone being opened on a thread of its own (it can take a while).
struct Opening {
    microphone: String,
    done: Receiver<Result<gob_voice::Capture, String>>,
}

/// State of the Voice settings tab between frames.
#[derive(Default)]
pub struct VoicePanel {
    download: Option<Download>,
    /// The last download's error.
    error: Option<String>,
    /// The microphone test is on.
    testing: bool,
    test: Option<MicTest>,
    opening: Option<Opening>,
    /// Microphone the level meter listens to; empty = the default one.
    test_microphone: String,
}

impl VoicePanel {
    /// Closes the microphone test (the tab is not shown).
    pub fn hide(&mut self) {
        self.testing = false;
        self.test = None;
        self.opening = None;
    }

    /// The Voice tab; true when voice settings changed (or a model was installed or removed).
    /// A slider counts once it is let go, not on every step of a drag.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, settings: &mut Settings) -> bool {
        let changed = self.model_section(ui, i18n, settings);
        ui.add_space(18.0);
        self.microphone_section(ui, i18n, settings) || changed
    }

    fn model_section(&mut self, ui: &mut egui::Ui, i18n: &I18n, settings: &mut Settings) -> bool {
        section_title(ui, icon::TRANSLATE, &i18n.tr("settings-voice-model"));
        ui.label(RichText::new(i18n.tr("settings-voice-model-hint")).weak());
        ui.add_space(4.0);
        let mut changed = false;
        let languages = i18n.languages();
        let language_name = |code: &str| {
            languages
                .iter()
                .find(|(c, _)| c == code)
                .map_or_else(|| code.to_string(), |(_, name)| name.clone())
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::GLOBE).size(style::ICON_SIZE))
                .on_hover_text(i18n.tr("settings-voice-language"));
            let same = i18n.tr("settings-voice-language-program");
            let current = if settings.voice_language.is_empty() {
                same.clone()
            } else {
                language_name(&settings.voice_language)
            };
            egui::ComboBox::from_id_salt("voice-language")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    let mut pick = |ui: &mut egui::Ui, code: &str, name: String| {
                        if ui
                            .selectable_label(settings.voice_language == code, name)
                            .clicked()
                            && settings.voice_language != code
                        {
                            settings.voice_language = code.to_string();
                            changed = true;
                        }
                    };
                    pick(ui, "", same);
                    for m in gob_voice::models::MODELS {
                        pick(ui, m.language, language_name(m.language));
                    }
                });
        });

        let Some(dir) = dir() else { return changed };
        let language = language(settings).to_string();
        let Some(model) = gob_voice::models::for_language(&language) else {
            let text = i18n.trf(
                "settings-voice-no-model",
                &[("language", language_name(&language).into())],
            );
            ui.label(RichText::new(text).weak());
            return changed;
        };
        changed |= self.poll_download(ui.ctx());
        let megabytes = model.size.div_ceil(1_000_000);
        ui.horizontal(
            |ui| match (&self.download, gob_voice::models::installed(&dir, model)) {
                (Some(d), _) => {
                    let (done, total) = *d.progress.lock().unwrap_or_else(|e| e.into_inner());
                    let fraction = done as f32 / total.max(1) as f32;
                    let text = i18n.trf(
                        "settings-voice-downloading",
                        &[
                            ("done", (done / 1_000_000).into()),
                            ("total", megabytes.into()),
                        ],
                    );
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .desired_width(240.0)
                            .text(text),
                    );
                    let tip = i18n.tr("settings-voice-cancel");
                    if style::icon_button(ui, true, icon::X, &tip).clicked() {
                        d.cancel.store(true, Ordering::Relaxed);
                    }
                }
                (None, Some(_)) => {
                    let text = i18n.trf("settings-voice-installed", &[("size", megabytes.into())]);
                    ui.label(format!("{}  {text}", icon::CHECK_CIRCLE));
                    let tip = i18n.tr("settings-voice-remove");
                    if style::icon_button(ui, true, icon::TRASH, &tip).clicked() {
                        match gob_voice::models::remove(&dir, model) {
                            Ok(()) => changed = true,
                            Err(e) => log::warn!("removing {}: {e}", model.name),
                        }
                    }
                }
                (None, None) => {
                    let text = format!(
                        "{}  {}",
                        icon::DOWNLOAD_SIMPLE,
                        i18n.trf("settings-voice-download", &[("size", megabytes.into())])
                    );
                    if ui.button(text).clicked() {
                        self.start_download(ui.ctx(), dir.clone(), model);
                    }
                }
            },
        );
        if let Some(e) = &self.error {
            let text = format!(
                "{}  {}",
                icon::WARNING,
                i18n.trf(
                    "settings-voice-download-failed",
                    &[("error", e.clone().into())]
                )
            );
            ui.label(RichText::new(text).color(ui.visuals().warn_fg_color));
        }
        if !library_available() {
            let text = format!(
                "{}  {}",
                icon::WARNING,
                i18n.tr("settings-voice-no-library")
            );
            ui.label(RichText::new(text).color(ui.visuals().warn_fg_color));
        }
        changed
    }

    fn start_download(&mut self, ctx: &egui::Context, dir: PathBuf, model: &'static ModelInfo) {
        let progress = Arc::new(Mutex::new((0, model.size)));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, done) = std::sync::mpsc::channel();
        let (p, c, ctx) = (progress.clone(), cancel.clone(), ctx.clone());
        let spawned = std::thread::Builder::new()
            .name("voice-model".into())
            .spawn(move || {
                let mut report = |done, total| {
                    *p.lock().unwrap_or_else(|e| e.into_inner()) = (done, total);
                    ctx.request_repaint();
                };
                let result = gob_voice::models::install(&dir, model, &mut report, &c)
                    .map(|_| ())
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        if let Err(e) = spawned {
            self.error = Some(e.to_string());
            return;
        }
        self.error = None;
        self.download = Some(Download {
            model,
            progress,
            cancel,
            done,
        });
    }

    /// Takes a finished download; true when a model was installed.
    fn poll_download(&mut self, ctx: &egui::Context) -> bool {
        let Some(d) = &self.download else {
            return false;
        };
        let result = match d.done.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return false;
            }
            Err(TryRecvError::Disconnected) => Err("the download stopped".into()),
        };
        let cancelled = d.cancel.load(Ordering::Relaxed);
        let name = d.model.name;
        self.download = None;
        match result {
            Ok(()) => true,
            Err(_) if cancelled => false,
            Err(e) => {
                log::error!("speech model {name}: {e}");
                self.error = Some(e);
                false
            }
        }
    }

    fn microphone_section(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        settings: &mut Settings,
    ) -> bool {
        section_title(ui, icon::MICROPHONE, &i18n.tr("settings-voice-microphone"));
        let mut gain = (settings.voice_gain * 100.0).round() as u16;
        let max = (gob_voice::MAX_GAIN * 100.0) as u16;
        let r = ui
            .horizontal(|ui| {
                let r = style::slider(ui, &mut gain, 0..=max, 5.0, " %");
                ui.label(i18n.tr("settings-voice-gain"));
                r
            })
            .inner
            .on_hover_text(i18n.tr("settings-voice-gain-tip"));
        // The meter follows the drag; the setting is kept when it is let go.
        if r.changed() {
            settings.voice_gain = f32::from(gain) / 100.0;
            if let Some(t) = &self.test {
                t.capture.set_gain(settings.voice_gain);
            }
        }
        let gain_done = settled(&r);

        // Level meter: the chosen microphone open while the switch is on.
        ui.horizontal(|ui| {
            ui.add(style::switch(
                &mut self.testing,
                i18n.tr("settings-voice-test"),
            ));
            let listed = microphones(ui.ctx(), false);
            let mics = listed.as_deref().map_or(&[][..], Vec::as_slice);
            let name = |id: &str| {
                if id.is_empty() {
                    i18n.tr("voice-mic-default")
                } else {
                    mics.iter()
                        .find(|m| m.id == id)
                        .map_or_else(|| id.to_string(), |m| m.name.clone())
                }
            };
            let mut picked = None;
            egui::ComboBox::from_id_salt("voice-test-mic")
                .selected_text(name(&self.test_microphone))
                .width(240.0)
                .show_ui(ui, |ui| {
                    let fresh = microphones(ui.ctx(), true);
                    if ui
                        .selectable_label(self.test_microphone.is_empty(), name(""))
                        .clicked()
                    {
                        picked = Some(String::new());
                    }
                    for m in fresh.iter().flat_map(|l| l.iter()) {
                        if ui
                            .selectable_label(self.test_microphone == m.id, &m.name)
                            .clicked()
                        {
                            picked = Some(m.id.clone());
                        }
                    }
                });
            if let Some(id) = picked {
                self.test_microphone = id;
            }
        });
        self.update_test(ui.ctx(), settings.voice_gain);
        if let Some(t) = &mut self.test {
            t.shown = t.capture.level().max(t.shown * 0.9);
            let clip = t.shown >= 1.0;
            let text = if clip {
                i18n.tr("settings-voice-clipping")
            } else {
                format!("{:.0} %", t.shown * 100.0)
            };
            let mut bar = egui::ProgressBar::new(t.shown)
                .desired_width(320.0)
                .text(text);
            if clip {
                bar = bar.fill(ui.visuals().warn_fg_color);
            }
            ui.add(bar);
            ui.label(RichText::new(i18n.tr("settings-voice-test-hint")).weak());
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }

        ui.add_space(8.0);
        let keep_open = ui
            .add(style::switch(
                &mut settings.voice_keep_open,
                i18n.tr("settings-voice-keep-open"),
            ))
            .on_hover_text(i18n.tr("settings-voice-keep-open-tip"))
            .changed();
        let mut confidence = (settings.voice_min_confidence * 100.0).round() as u8;
        let r = ui
            .horizontal(|ui| {
                let r = style::slider(ui, &mut confidence, 0..=100, 5.0, " %");
                ui.label(i18n.tr("settings-voice-confidence"));
                r
            })
            .inner
            .on_hover_text(i18n.tr("settings-voice-confidence-tip"));
        if r.changed() {
            settings.voice_min_confidence = f32::from(confidence) / 100.0;
        }
        let confidence_done = settled(&r);
        ui.label(RichText::new(i18n.tr("settings-voice-next-start")).weak());
        gain_done || confidence_done || keep_open
    }

    /// Opens (on a thread), switches or closes the level meter's microphone to match the switch
    /// and the chosen microphone.
    fn update_test(&mut self, ctx: &egui::Context, gain: f32) {
        if !self.testing {
            self.test = None;
            self.opening = None;
            return;
        }
        if self
            .test
            .as_ref()
            .is_some_and(|t| t.microphone == self.test_microphone)
        {
            return;
        }
        match &self.opening {
            Some(o) if o.microphone == self.test_microphone => match o.done.try_recv() {
                Ok(Ok(capture)) => {
                    capture.set_gain(gain);
                    self.test = Some(MicTest {
                        capture,
                        microphone: self.test_microphone.clone(),
                        shown: 0.0,
                    });
                    self.opening = None;
                }
                Ok(Err(e)) => {
                    log::error!("microphone test: {e}");
                    self.testing = false;
                    self.opening = None;
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
                Err(TryRecvError::Disconnected) => {
                    self.testing = false;
                    self.opening = None;
                }
            },
            _ => {
                self.test = None;
                let microphone = self.test_microphone.clone();
                let (tx, done) = std::sync::mpsc::channel();
                let (id, ctx) = (microphone.clone(), ctx.clone());
                let spawned = std::thread::Builder::new()
                    .name("microphone-test".into())
                    .spawn(move || {
                        let id = (!id.is_empty()).then_some(id.as_str());
                        let opened =
                            gob_voice::Capture::open(id, gain, |_| {}).map_err(|e| e.to_string());
                        // Not wanted any more (switched off meanwhile): it closes here.
                        let _ = tx.send(opened);
                        ctx.request_repaint();
                    });
                match spawned {
                    Ok(_) => self.opening = Some(Opening { microphone, done }),
                    Err(e) => {
                        log::error!("microphone test: {e}");
                        self.testing = false;
                    }
                }
            }
        }
    }
}

/// A slider's value is final: changed without a drag (a click, the arrows, typing) or let go.
fn settled(r: &egui::Response) -> bool {
    (r.changed() && !r.dragged()) || r.drag_stopped()
}
