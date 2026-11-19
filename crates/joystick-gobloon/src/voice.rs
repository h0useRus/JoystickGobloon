//! Voice commands in the program: where the speech libraries (libvosk, sherpa-onnx) and models
//! live (the libraries next to the exe or in `<data folder>/voice`, models of both engines
//! downloaded into `<data folder>/voice` by gob-voice's `models` and kept there), and the Voice
//! tab of the settings.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use gob_engine::VoiceConfig;
use gob_i18n::I18n;
use gob_voice::models::Size;
use gob_voice::tts::VoiceInfo;
use gob_voice::{Engine, ModelInfo};

use crate::settings::Settings;
use crate::settings_view::section_title;
use crate::style::{self, icon};

/// The folder of the speech models (and of sherpa-onnx when it is not next to the exe).
pub fn dir() -> Option<PathBuf> {
    crate::settings::data_dir().map(|d| d.join("voice"))
}

/// Folders the speech libraries are looked for in.
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

/// The speech recognizer chosen in the settings.
fn engine(settings: &Settings) -> Engine {
    Engine::parse(&settings.voice_engine)
}

/// The speech model size chosen in the settings.
fn size(settings: &Settings) -> Size {
    Size::parse(&settings.voice_model_size)
}

/// How voice commands run with these settings; `None` while the model of their language is not
/// installed (voice-input actions then do nothing).
pub fn config(settings: &Settings) -> Option<VoiceConfig> {
    // Called on start and after every voice setting: the natural voices follow too.
    apply_natural_voices(settings);
    let dir = dir()?;
    let engine = engine(settings);
    let language = language(settings);
    let Some(model) = gob_voice::models::pick(engine, language, size(settings)) else {
        log::info!(
            "voice commands: no {} speech model for {language}",
            engine.name()
        );
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
        engine,
        library_dirs: library_dirs(),
        model: path,
        language: language
            .split(['-', '_'])
            .next()
            .unwrap_or(language)
            .to_lowercase(),
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

/// Whether the speech library of `engine` loads, checked once per run.
fn library_available(engine: Engine) -> bool {
    static VOSK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static SHERPA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let check = |loaded: Result<(), gob_voice::Error>| match loaded {
        Ok(()) => true,
        Err(e) => {
            log::warn!("voice commands: {e}");
            false
        }
    };
    match engine {
        Engine::Vosk => {
            *VOSK.get_or_init(|| check(gob_voice::Vosk::load(&library_dirs()).map(|_| ())))
        }
        Engine::Sherpa => {
            *SHERPA.get_or_init(|| check(gob_voice::Sherpa::load(&library_dirs()).map(|_| ())))
        }
    }
}

/// Tells the engine which natural voices are installed and which one is the default.
pub fn apply_natural_voices(settings: &Settings) {
    let tts = dir().map(|d| gob_voice::tts::dir(&d));
    let library = tts.as_deref().and_then(gob_voice::tts::library_installed);
    let voices: Vec<(String, PathBuf)> = tts
        .as_deref()
        .map(|dir| {
            gob_voice::tts::VOICES
                .iter()
                .filter_map(|v| Some((v.display_name(), gob_voice::tts::installed(dir, v)?)))
                .collect()
        })
        .unwrap_or_default();
    let default = Some(settings.tts_voice.clone()).filter(|v| voices.iter().any(|(n, _)| n == v));
    gob_engine::set_natural_voices(gob_engine::NaturalVoices {
        library,
        voices,
        default,
    });
}

/// What is being downloaded.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Model,
    VoiceLibrary,
    Voice(&'static VoiceInfo),
}

/// A download (speech model, voice library or voice) running on its own thread.
struct Download {
    what: Target,
    /// For the log.
    name: &'static str,
    /// Bytes so far, total.
    progress: Arc<Mutex<(u64, u64)>>,
    cancel: Arc<AtomicBool>,
    done: Receiver<Result<(), String>>,
}

/// What a download thread runs: the install with its progress and cancel flag.
type Job = Box<
    dyn FnOnce(&mut dyn FnMut(u64, u64), &AtomicBool) -> Result<PathBuf, gob_voice::Error> + Send,
>;

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
    /// The last download's error, and what it was.
    error: Option<(Target, String)>,
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
        let mut changed = self.poll_download(ui.ctx());
        changed |= self.model_section(ui, i18n, settings);
        ui.add_space(18.0);
        changed |= self.microphone_section(ui, i18n, settings);
        ui.add_space(18.0);
        self.natural_section(ui, i18n, settings) || changed
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
        let engine_tip = |e: Engine| match e {
            Engine::Vosk => i18n.tr("settings-voice-engine-vosk"),
            Engine::Sherpa => i18n.tr("settings-voice-engine-sherpa"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::EAR).size(style::ICON_SIZE))
                .on_hover_text(i18n.tr("settings-voice-engine"));
            let current = engine(settings);
            egui::ComboBox::from_id_salt("voice-engine")
                .selected_text(current.name())
                .show_ui(ui, |ui| {
                    for e in Engine::ALL {
                        // Models of the other engine stay installed: switching back needs no
                        // download.
                        if ui
                            .selectable_label(e == current, e.name())
                            .on_hover_text(engine_tip(e))
                            .clicked()
                            && e != current
                        {
                            settings.voice_engine = e.key().to_string();
                            changed = true;
                        }
                    }
                })
                .response
                .on_hover_text(engine_tip(current));
        });
        let engine = engine(settings);
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
                    for code in gob_voice::models::languages(engine) {
                        pick(ui, code, language_name(code));
                    }
                });
        });
        let language = language(settings).to_string();
        let models = gob_voice::models::for_language(engine, &language);
        if !models.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::GAUGE).size(style::ICON_SIZE))
                    .on_hover_text(i18n.tr("settings-voice-size"));
                let text = |m: &ModelInfo| {
                    let key = match m.size {
                        Size::Small => "settings-voice-size-small",
                        Size::Large => "settings-voice-size-large",
                    };
                    i18n.trf(key, &[("memory", m.memory_mb.into())])
                };
                let chosen = gob_voice::models::pick(engine, &language, size(settings));
                let current = chosen.map(text).unwrap_or_default();
                if models.len() == 1 {
                    ui.label(current)
                        .on_hover_text(i18n.tr("settings-voice-size-only"));
                    return;
                }
                egui::ComboBox::from_id_salt("voice-size")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for m in &models {
                            let selected = chosen.is_some_and(|c| c.size == m.size);
                            if ui.selectable_label(selected, text(m)).clicked() && !selected {
                                settings.voice_model_size = m.size.key().to_string();
                                changed = true;
                            }
                        }
                    });
            });
        }

        let Some(dir) = dir() else { return changed };
        let Some(model) = gob_voice::models::pick(engine, &language, size(settings)) else {
            let text = i18n.trf(
                "settings-voice-no-model",
                &[("language", language_name(&language).into())],
            );
            ui.label(RichText::new(text).weak());
            return changed;
        };
        let megabytes = model.bytes.div_ceil(1_000_000);
        let busy = self.download.is_some();
        ui.horizontal(|ui| {
            match (
                self.downloading(Target::Model),
                gob_voice::models::installed(&dir, model),
            ) {
                (Some(d), _) => progress_bar(ui, i18n, d),
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
                    if ui.add_enabled(!busy, egui::Button::new(text)).clicked() {
                        let dir = dir.clone();
                        let job: Job = Box::new(move |progress, cancel| {
                            gob_voice::models::install(&dir, model, progress, cancel)
                        });
                        self.start_download(ui.ctx(), Target::Model, model.name, model.bytes, job);
                    }
                }
            }
        });
        if let Some((Target::Model, e)) = &self.error {
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
        if !library_available(engine) {
            let text = format!(
                "{}  {}",
                icon::WARNING,
                i18n.tr("settings-voice-no-library")
            );
            ui.label(RichText::new(text).color(ui.visuals().warn_fg_color));
        }
        changed
    }

    /// The download of `what`, when it runs.
    fn downloading(&self, what: Target) -> Option<&Download> {
        self.download.as_ref().filter(|d| d.what == what)
    }

    fn start_download(
        &mut self,
        ctx: &egui::Context,
        what: Target,
        name: &'static str,
        bytes: u64,
        job: Job,
    ) {
        let progress = Arc::new(Mutex::new((0, bytes)));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, done) = std::sync::mpsc::channel();
        let (p, c, ctx) = (progress.clone(), cancel.clone(), ctx.clone());
        let spawned = std::thread::Builder::new()
            .name("voice-download".into())
            .spawn(move || {
                let mut report = |done, total| {
                    *p.lock().unwrap_or_else(|e| e.into_inner()) = (done, total);
                    ctx.request_repaint();
                };
                let result = job(&mut report, &c).map(|_| ()).map_err(|e| e.to_string());
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        if let Err(e) = spawned {
            self.error = Some((what, e.to_string()));
            return;
        }
        self.error = None;
        self.download = Some(Download {
            what,
            name,
            progress,
            cancel,
            done,
        });
    }

    /// Takes a finished download; true when something was installed.
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
        let (what, name) = (d.what, d.name);
        self.download = None;
        match result {
            Ok(()) => true,
            Err(_) if cancelled => false,
            Err(e) => {
                log::error!("download {name}: {e}");
                self.error = Some((what, e));
                false
            }
        }
    }

    /// Natural voices: the voice library (downloaded on request: it carries GPL code), the
    /// voices, and which one speaks text to speech without a voice of its own.
    fn natural_section(&mut self, ui: &mut egui::Ui, i18n: &I18n, settings: &mut Settings) -> bool {
        section_title(ui, icon::USER_SOUND, &i18n.tr("settings-tts"));
        ui.label(RichText::new(i18n.tr("settings-tts-hint")).weak());
        ui.add_space(4.0);
        let Some(tts) = dir().map(|d| gob_voice::tts::dir(&d)) else {
            return false;
        };
        let mut changed = false;
        let busy = self.download.is_some();
        let library = gob_voice::tts::library_installed(&tts);
        let archive = gob_voice::tts::library();
        let megabytes = archive.bytes.div_ceil(1_000_000);
        ui.horizontal(
            |ui| match (self.downloading(Target::VoiceLibrary), &library) {
                (Some(d), _) => progress_bar(ui, i18n, d),
                (None, Some(_)) => {
                    ui.label(format!(
                        "{}  {}",
                        icon::CHECK_CIRCLE,
                        i18n.tr("settings-tts-library-installed")
                    ));
                    let tip = i18n.tr("settings-tts-library-remove");
                    if style::icon_button(ui, true, icon::TRASH, &tip).clicked() {
                        match gob_voice::tts::remove_library(&tts) {
                            Ok(()) => changed = true,
                            Err(e) => log::warn!("removing the voice library: {e}"),
                        }
                    }
                }
                (None, None) => {
                    let text = format!(
                        "{}  {}",
                        icon::DOWNLOAD_SIMPLE,
                        i18n.trf(
                            "settings-tts-library-download",
                            &[("size", megabytes.into())]
                        )
                    );
                    if ui.add_enabled(!busy, egui::Button::new(text)).clicked() {
                        let dir = tts.clone();
                        let job: Job = Box::new(move |progress, cancel| {
                            gob_voice::tts::install_library(&dir, progress, cancel)
                        });
                        self.start_download(
                            ui.ctx(),
                            Target::VoiceLibrary,
                            archive.name,
                            archive.bytes,
                            job,
                        );
                    }
                }
            },
        );
        ui.label(RichText::new(i18n.tr("settings-tts-gpl")).weak().small());
        if library.is_some() {
            ui.add_space(4.0);
            let language = language(settings)
                .split(['-', '_'])
                .next()
                .unwrap_or("")
                .to_string();
            let mut voices: Vec<&'static VoiceInfo> = gob_voice::tts::VOICES.iter().collect();
            // The voices of the language spoken first.
            voices.sort_by_key(|v| v.language != language);
            egui::Grid::new("tts-voices")
                .num_columns(2)
                .spacing([style::SPACE_M, style::SPACE_XS])
                .show(ui, |ui| {
                    for v in voices {
                        ui.label(format!("{}  ·  {}", v.display_name(), v.license));
                        let megabytes = v.bytes.div_ceil(1_000_000);
                        ui.horizontal(|ui| {
                            match (
                                self.downloading(Target::Voice(v)),
                                gob_voice::tts::installed(&tts, v),
                            ) {
                                (Some(d), _) => progress_bar(ui, i18n, d),
                                (None, Some(_)) => {
                                    ui.label(format!(
                                        "{}  {}",
                                        icon::CHECK_CIRCLE,
                                        i18n.tr("settings-tts-voice-installed")
                                    ));
                                    let tip = i18n.tr("settings-tts-voice-remove");
                                    if style::icon_button(ui, true, icon::TRASH, &tip).clicked() {
                                        match gob_voice::tts::remove(&tts, v) {
                                            Ok(()) => changed = true,
                                            Err(e) => {
                                                log::warn!("removing the voice {}: {e}", v.id)
                                            }
                                        }
                                    }
                                }
                                (None, None) => {
                                    let text = format!(
                                        "{}  {}",
                                        icon::DOWNLOAD_SIMPLE,
                                        i18n.trf(
                                            "settings-tts-voice-download",
                                            &[("size", megabytes.into())]
                                        )
                                    );
                                    if ui.add_enabled(!busy, egui::Button::new(text)).clicked() {
                                        let dir = tts.clone();
                                        let job: Job = Box::new(move |progress, cancel| {
                                            gob_voice::tts::install(&dir, v, progress, cancel)
                                        });
                                        let archive = v.archive();
                                        self.start_download(
                                            ui.ctx(),
                                            Target::Voice(v),
                                            archive.name,
                                            archive.bytes,
                                            job,
                                        );
                                    }
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            ui.add_space(4.0);
            let installed: Vec<String> = gob_voice::tts::VOICES
                .iter()
                .filter(|v| gob_voice::tts::installed(&tts, v).is_some())
                .map(|v| v.display_name())
                .collect();
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::USER_SOUND).size(style::ICON_SIZE))
                    .on_hover_text(i18n.tr("settings-tts-default"));
                let system = i18n.tr("settings-tts-default-system");
                let current = if installed.contains(&settings.tts_voice) {
                    settings.tts_voice.clone()
                } else {
                    system.clone()
                };
                egui::ComboBox::from_id_salt("tts-default")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(settings.tts_voice.is_empty(), &system)
                            .clicked()
                            && !settings.tts_voice.is_empty()
                        {
                            settings.tts_voice.clear();
                            changed = true;
                        }
                        for name in &installed {
                            if ui
                                .selectable_label(settings.tts_voice == *name, name)
                                .clicked()
                                && settings.tts_voice != *name
                            {
                                settings.tts_voice = name.clone();
                                changed = true;
                            }
                        }
                    });
            });
        }
        if let Some((what, e)) = &self.error
            && *what != Target::Model
        {
            let text = format!(
                "{}  {}",
                icon::WARNING,
                i18n.trf(
                    "settings-tts-download-failed",
                    &[("error", e.clone().into())]
                )
            );
            ui.label(RichText::new(text).color(ui.visuals().warn_fg_color));
        }
        changed
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
/// A download's progress bar with its cancel button.
fn progress_bar(ui: &mut egui::Ui, i18n: &I18n, d: &Download) {
    let (done, total) = *d.progress.lock().unwrap_or_else(|e| e.into_inner());
    let fraction = done as f32 / total.max(1) as f32;
    let text = i18n.trf(
        "settings-voice-downloading",
        &[
            ("done", (done / 1_000_000).into()),
            ("total", total.div_ceil(1_000_000).into()),
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

fn settled(r: &egui::Response) -> bool {
    (r.changed() && !r.dragged()) || r.drag_stopped()
}
