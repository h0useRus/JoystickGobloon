//! The Game page: which game action each output of the profile drives, as a wiring schematic
//! (controllers -> profile outputs -> game actions, lit live while inputs move) and as a table
//! like the game's own binding screen; reading the bindings already in the game and exporting
//! the profile's into the game's files through an export plugin (gob-export).
//!
//! Plugins load and run on a worker thread: a plugin hook may take up to two seconds and the
//! window never waits for it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use gob_core::{GameDevice, GameOutput, HatDirection, InputType, Profile, VirtualOutput};
use gob_export::{
    Bind, Catalog, Catalogs, ChangeKind, DeviceLink, Files, Fit, Manifest, OptionDecl, OptionKind,
    OptionValue, Origin, OurDevice, ReadContext, ReadResult, Target, WriteContext, WriteResult,
    host,
};
use gob_i18n::I18n;
use gob_input::DeviceState;

use crate::action_editor::Op;
use crate::style::{self, icon};

/// Height of one node in the wiring schematic.
const ROW: f32 = 28.0;
/// How long a game action stays lit after its input lets go, in seconds.
const FLASH: f64 = 1.0;
/// Catalog actions listed for a search.
const SEARCH_LIMIT: usize = 60;

// ---- worker ------------------------------------------------------------------------------------

/// What the GUI keeps of a plugin.
#[derive(Clone)]
struct PluginInfo {
    manifest: Manifest,
    catalogs: Catalogs,
    options: Vec<OptionDecl>,
}

#[derive(Clone)]
struct FoundTarget {
    target: Target,
    version: Option<String>,
}

enum Request {
    Scan {
        env: gob_export::Env,
    },
    Read {
        plugin: usize,
        target: Target,
        version: Option<String>,
        ours: Vec<OurDevice>,
        /// Take what is read into the profile.
        import: bool,
    },
    Preview {
        plugin: usize,
        target: Target,
        version: Option<String>,
        ours: Vec<OurDevice>,
        devices: Vec<DeviceLink>,
        binds: Vec<Bind>,
        profile: String,
        options: BTreeMap<String, OptionValue>,
    },
    Write {
        target: Target,
        files: Files,
        result: WriteResult,
    },
}

enum Reply {
    Loaded {
        plugins: Vec<PluginInfo>,
        errors: Vec<String>,
    },
    Scanned(Vec<Vec<FoundTarget>>),
    Read {
        plugin: usize,
        import: bool,
        result: Result<ReadResult, String>,
    },
    Preview(Result<(Files, WriteResult), String>),
    Written(Result<host::Written, String>),
}

/// Folders plugins load from: the program's `export` folder, then the user's; debug builds
/// take the repository's plugins first.
fn plugin_dirs() -> Vec<(PathBuf, Origin)> {
    let mut dirs = Vec::new();
    if cfg!(debug_assertions) {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export");
        dirs.push((repo, Origin::Bundled));
    }
    if let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("export")))
    {
        dirs.push((exe, Origin::Bundled));
    }
    if let Some(data) = crate::settings::data_dir() {
        dirs.push((data.join("export"), Origin::User));
    }
    dirs
}

fn spawn_worker(ctx: egui::Context) -> (Sender<Request>, Receiver<Reply>) {
    let (req_tx, req_rx) = channel::<Request>();
    let (rep_tx, rep_rx) = channel::<Reply>();
    std::thread::Builder::new()
        .name("game-export".into())
        .spawn(move || {
            let mut plugins = Vec::new();
            let mut infos = Vec::new();
            let mut errors = Vec::new();
            for found in gob_export::discover(&plugin_dirs()) {
                match found.plugin {
                    Ok(p) => {
                        let options = p.options().unwrap_or_else(|e| {
                            errors.push(e.to_string());
                            Vec::new()
                        });
                        infos.push(PluginInfo {
                            manifest: p.manifest.clone(),
                            catalogs: p.catalogs.clone(),
                            options,
                        });
                        plugins.push(p);
                    }
                    Err(e) => errors.push(e.to_string()),
                }
            }
            log::info!("game export: {} plugin(s) loaded", plugins.len());
            let send = |r: Reply| {
                let _ = rep_tx.send(r);
                ctx.request_repaint();
            };
            send(Reply::Loaded {
                plugins: infos,
                errors,
            });
            for req in req_rx {
                match req {
                    Request::Scan { env } => {
                        let all = plugins
                            .iter()
                            .map(|p| {
                                let targets = p.targets(&env).unwrap_or_else(|e| {
                                    log::warn!("game export: {e}");
                                    Vec::new()
                                });
                                targets
                                    .into_iter()
                                    .filter(host::installed)
                                    .map(|target| {
                                        let files = host::read_files(&target);
                                        let version = p.version(&target, &files).ok().flatten();
                                        FoundTarget { target, version }
                                    })
                                    .collect()
                            })
                            .collect();
                        send(Reply::Scanned(all));
                    }
                    Request::Read {
                        plugin,
                        target,
                        version,
                        ours,
                        import,
                    } => {
                        let result = (|| {
                            let p = plugins.get(plugin).ok_or("no plugin")?;
                            let catalog = pick(&p.catalogs, version.as_deref());
                            let files = host::read_files(&target);
                            p.read(&ReadContext {
                                target: &target,
                                files: &files,
                                catalog: &catalog,
                                ours: &ours,
                            })
                            .map_err(|e| e.to_string())
                        })();
                        send(Reply::Read {
                            plugin,
                            import,
                            result,
                        });
                    }
                    Request::Preview {
                        plugin,
                        target,
                        version,
                        ours,
                        devices,
                        binds,
                        profile,
                        options,
                    } => {
                        let result = (|| {
                            let p = plugins.get(plugin).ok_or("no plugin")?;
                            let catalog = pick(&p.catalogs, version.as_deref());
                            let files = host::read_files(&target);
                            let options = p.option_values(&options).map_err(|e| e.to_string())?;
                            let binds: Vec<Bind> = binds
                                .into_iter()
                                .map(|b| Bind {
                                    action: p
                                        .renames
                                        .resolve(&b.action, version.as_deref())
                                        .to_string(),
                                    ..b
                                })
                                .collect();
                            let result = p
                                .write(&WriteContext {
                                    target: &target,
                                    files: &files,
                                    catalog: &catalog,
                                    ours: &ours,
                                    devices: &devices,
                                    binds: &binds,
                                    profile: &profile,
                                    options: &options,
                                })
                                .map_err(|e| e.to_string())?;
                            Ok::<_, String>((files, result))
                        })();
                        send(Reply::Preview(result));
                    }
                    Request::Write {
                        target,
                        files,
                        result,
                    } => send(Reply::Written(host::apply(&target, &files, &result))),
                }
            }
        })
        .expect("game export thread starts");
    (req_tx, rep_rx)
}

fn pick(catalogs: &Catalogs, version: Option<&str>) -> Catalog {
    catalogs
        .pick(version)
        .map(|(c, _)| c.clone())
        .unwrap_or_default()
}

// ---- view ----------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Wiring,
    List,
}

struct ExportDialog {
    options: BTreeMap<String, OptionValue>,
    preview: Option<Result<(Files, WriteResult), String>>,
    busy: bool,
}

#[derive(Default)]
pub struct GameView {
    worker: Option<(Sender<Request>, Receiver<Reply>)>,
    plugins: Option<Vec<PluginInfo>>,
    errors: Vec<String>,
    selected: usize,
    targets: Vec<Vec<FoundTarget>>,
    target: usize,
    scanning: bool,
    tab: Tab,
    search: String,
    picked: Option<GameOutput>,
    listening: bool,
    /// When each game action was last lit.
    flash: HashMap<String, f64>,
    /// Game actions lit in the last frame.
    lit: std::collections::HashSet<String>,
    /// The bindings last read from the game: (plugin, result).
    in_game: Option<(usize, ReadResult)>,
    importing: bool,
    /// The game or target changed: read the game again.
    reread: bool,
    export: Option<ExportDialog>,
    status: Option<String>,
}

/// What the page asks of the app.
#[derive(Default)]
pub struct Output {
    pub toggle_window: bool,
    pub status: Option<String>,
}

/// Everything the page reads from the app.
pub struct Context<'a> {
    pub profile: &'a Profile,
    pub profile_name: &'a str,
    pub devices: &'a [DeviceState],
    pub vjoy: &'a [gob_virtual::DeviceInfo],
}

impl GameView {
    fn send(&mut self, req: Request) {
        if let Some((tx, _)) = &self.worker {
            let _ = tx.send(req);
        }
    }

    fn plugin(&self) -> Option<&PluginInfo> {
        self.plugins.as_ref()?.get(self.selected)
    }

    fn found(&self) -> Option<&FoundTarget> {
        self.targets.get(self.selected)?.get(self.target)
    }

    fn game_id(&self) -> Option<String> {
        self.plugin().map(|p| p.manifest.id.clone())
    }

    fn scan(&mut self) {
        self.scanning = true;
        let env = gob_export::Env {
            platform: if cfg!(windows) { "windows" } else { "linux" }.into(),
            roots: crate::platform::game_roots(),
        };
        self.send(Request::Scan { env });
    }

    /// The catalog for the selected target (or the profile's last export), and its fit.
    fn catalog(&self, profile: &Profile) -> Option<(Catalog, Fit)> {
        let plugin = self.plugin()?;
        let version = self.found().and_then(|f| f.version.clone()).or_else(|| {
            profile
                .game_bindings(&plugin.manifest.id)
                .and_then(|g| g.version.clone())
        });
        plugin
            .catalogs
            .pick(version.as_deref())
            .map(|(c, f)| (c.clone(), f))
    }

    fn poll(&mut self, ctx: &Context, ops: &mut Vec<Op>) {
        let Some((_, rx)) = &self.worker else {
            return;
        };
        let replies: Vec<Reply> = rx.try_iter().collect();
        for reply in replies {
            match reply {
                Reply::Loaded { plugins, errors } => {
                    for e in &errors {
                        log::warn!("game export plugin: {e}");
                    }
                    // The profile's game first, when it has one.
                    if let Some(i) = ctx
                        .profile
                        .game_bindings
                        .first()
                        .and_then(|g| plugins.iter().position(|p| p.manifest.id == g.game))
                    {
                        self.selected = i;
                    }
                    self.plugins = Some(plugins);
                    self.errors = errors;
                    self.scan();
                }
                Reply::Scanned(all) => {
                    self.scanning = false;
                    self.targets = all;
                    // The target last exported to, when it is still there.
                    let last = self
                        .game_id()
                        .and_then(|g| ctx.profile.game_bindings(&g).map(|b| b.target.clone()));
                    self.target = self
                        .targets
                        .get(self.selected)
                        .and_then(|t| t.iter().position(|f| Some(&f.target.id) == last.as_ref()))
                        .unwrap_or(0);
                    self.read_game(ctx, false);
                }
                Reply::Read {
                    plugin,
                    import,
                    result,
                } => {
                    self.importing = false;
                    match result {
                        Ok(read) => {
                            let game = self
                                .plugins
                                .as_ref()
                                .and_then(|p| p.get(plugin))
                                .map(|p| p.manifest.id.clone());
                            // Debug builds can take the game's bindings at start, for checks.
                            let auto = cfg!(debug_assertions)
                                && std::env::var_os("JOYSTICK_GOBLOON_GAME_IMPORT").is_some()
                                && game
                                    .as_ref()
                                    .is_some_and(|g| ctx.profile.game_bindings(g).is_none());
                            if (import || auto)
                                && let Some(game) = game
                            {
                                let n = import_ops(ctx.profile, &game, &read, ops);
                                self.status = Some(n.to_string());
                            }
                            self.in_game = Some((plugin, read));
                        }
                        Err(e) => {
                            log::warn!("game export: reading the game failed: {e}");
                            self.status = Some(format!("error:{e}"));
                        }
                    }
                }
                Reply::Preview(result) => {
                    if let Some(d) = &mut self.export {
                        d.busy = false;
                        d.preview = Some(result);
                    }
                }
                Reply::Written(result) => {
                    self.export = None;
                    match result {
                        Ok(w) => {
                            log::info!("game export: wrote {:?}, copies {:?}", w.files, w.backups);
                            if let (Some(game), Some(found)) = (self.game_id(), self.found()) {
                                ops.push(Op::GameExported {
                                    game,
                                    target: found.target.id.clone(),
                                    version: found.version.clone(),
                                });
                            }
                            self.status = Some(format!("written:{}", w.files.len()));
                        }
                        Err(e) => {
                            log::error!("game export failed: {e}");
                            self.status = Some(format!("error:{e}"));
                        }
                    }
                }
            }
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        ctx: &Context,
        ops: &mut Vec<Op>,
        detached: bool,
    ) -> Output {
        let mut out = Output::default();
        if self.worker.is_none() {
            self.worker = Some(spawn_worker(ui.ctx().clone()));
            // Debug builds open the table with JOYSTICK_GOBLOON_TAB=game-list, for checks.
            if cfg!(debug_assertions)
                && std::env::var("JOYSTICK_GOBLOON_TAB").is_ok_and(|t| t == "game-list")
            {
                self.tab = Tab::List;
            }
        }
        self.poll(ctx, ops);
        if let Some(s) = self.status.take() {
            out.status = Some(status_text(i18n, &s, self.plugin()));
        }
        let Some(plugins) = self.plugins.clone() else {
            crate::app::empty_state(ui, icon::HOURGLASS, &i18n.tr("game-loading"));
            return out;
        };
        if plugins.is_empty() {
            crate::app::empty_state(ui, icon::PLUGS, &i18n.tr("game-no-plugins"));
            for e in &self.errors {
                ui.label(RichText::new(e).weak());
            }
            return out;
        }
        out.toggle_window = self.header(ui, i18n, ctx, &plugins, detached);
        if std::mem::take(&mut self.reread) {
            self.read_game(ctx, false);
        }
        self.import_banner(ui, i18n, ctx);
        ui.add_space(style::SPACE_S);
        self.device_links(ui, i18n, ctx, ops);
        ui.add_space(style::SPACE_S);
        ui.horizontal(|ui| {
            if style::tab(
                ui,
                self.tab == Tab::Wiring,
                icon::FLOW_ARROW,
                &i18n.tr("game-tab-wiring"),
            )
            .clicked()
            {
                self.tab = Tab::Wiring;
            }
            if style::tab(
                ui,
                self.tab == Tab::List,
                icon::LIST_BULLETS,
                &i18n.tr("game-tab-list"),
            )
            .clicked()
            {
                self.tab = Tab::List;
            }
            ui.add_space(style::SPACE_L);
            ui.label(icon::MAGNIFYING_GLASS);
            ui.add(
                style::text_field(&mut self.search)
                    .hint_text(i18n.tr("game-search"))
                    .desired_width(220.0),
            );
            ui.add_space(style::SPACE_M);
            let press =
                style::icon_toggle(ui, self.listening, icon::HAND_TAP, &i18n.tr("game-press"));
            if press.clicked() {
                self.listening = !self.listening;
            }
        });
        if self.listening {
            ui.label(RichText::new(i18n.tr("game-press-hint")).color(style::accent(ui)));
        } else if self.picked.is_some() {
            ui.label(RichText::new(i18n.tr("game-pick-action-hint")).color(style::accent(ui)));
        }
        ui.add_space(style::SPACE_S);
        let Some(game) = self.game_id() else {
            return out;
        };
        let catalog = self
            .catalog(ctx.profile)
            .map(|(c, _)| c)
            .unwrap_or_default();
        let graph = Graph::build(ctx, i18n, &game, &catalog, &self.search);
        let live = Live::read(ctx, &graph);
        if self.listening
            && let Some(o) = graph
                .inputs
                .iter()
                .enumerate()
                .find(|(i, _)| live.inputs.get(*i).copied().unwrap_or(0.0) > 0.5)
                .and_then(|(i, _)| graph.input_edges.iter().find(|(l, _)| *l == i))
                .map(|(_, m)| graph.outputs[*m].clone())
        {
            self.picked = Some(o);
            self.listening = false;
        }
        let now = ui.input(|i| i.time);
        self.lit.retain(|id| {
            graph
                .actions
                .iter()
                .position(|a| &a.id == id)
                .is_some_and(|r| live.actions.get(r).copied().unwrap_or(0.0) > 0.0)
        });
        for (r, a) in graph.actions.iter().enumerate() {
            if live.actions.get(r).copied().unwrap_or(0.0) > 0.0 {
                self.flash.insert(a.id.clone(), now);
            }
        }
        if live.any || self.flash.values().any(|t| now - t < FLASH) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        match self.tab {
            Tab::Wiring => self.wiring(ui, i18n, &game, &graph, &live, now, ops),
            Tab::List => self.list(ui, i18n, &game, &graph, &live, now, ops),
        }
        self.export_dialog(ui, i18n, ctx, &game);
        out
    }

    /// Game and install pickers, the game version, and the page's buttons; true to detach.
    fn header(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        ctx: &Context,
        plugins: &[PluginInfo],
        detached: bool,
    ) -> bool {
        let mut toggle = false;
        ui.horizontal(|ui| {
            let name = plugins
                .get(self.selected)
                .map_or(String::new(), |p| p.manifest.name.clone());
            egui::ComboBox::from_id_salt("game-plugin")
                .selected_text(format!("{}  {name}", icon::GAME_CONTROLLER))
                .show_ui(ui, |ui| {
                    for (i, p) in plugins.iter().enumerate() {
                        if ui
                            .selectable_label(self.selected == i, &p.manifest.name)
                            .clicked()
                        {
                            self.selected = i;
                            self.target = 0;
                            self.picked = None;
                            self.in_game = None;
                            self.reread = true;
                        }
                    }
                });
            let targets = self.targets.get(self.selected).cloned().unwrap_or_default();
            if self.scanning {
                ui.spinner();
                ui.label(RichText::new(i18n.tr("game-scanning")).weak());
            } else if targets.is_empty() {
                ui.label(
                    RichText::new(i18n.trf("game-not-found", &[("game", name.clone().into())]))
                        .weak(),
                );
            } else {
                let text = |f: &FoundTarget| format!("{}  ·  {}", f.target.name, f.target.id);
                egui::ComboBox::from_id_salt("game-target")
                    .selected_text(targets.get(self.target).map(text).unwrap_or_default())
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for (i, f) in targets.iter().enumerate() {
                            if ui.selectable_label(self.target == i, text(f)).clicked() {
                                self.target = i;
                                self.in_game = None;
                                self.reread = true;
                            }
                        }
                    });
                if let Some((catalog, fit)) = self.catalog(ctx.profile) {
                    let version = targets
                        .get(self.target)
                        .and_then(|f| f.version.clone())
                        .unwrap_or_else(|| "?".into());
                    let warn = match fit {
                        Fit::Exact => None,
                        Fit::GameNewer => Some("game-catalog-older"),
                        Fit::GameOlder => Some("game-catalog-newer"),
                        Fit::Unknown => Some("game-catalog-unknown"),
                    };
                    let label = ui.label(
                        RichText::new(i18n.trf(
                            "game-version",
                            &[
                                ("version", version.into()),
                                ("catalog", catalog.version.clone().into()),
                            ],
                        ))
                        .weak(),
                    );
                    if let Some(key) = warn {
                        ui.label(RichText::new(icon::WARNING).color(ui.visuals().warn_fg_color))
                            .on_hover_text(i18n.tr(key));
                    } else {
                        label.on_hover_text(i18n.tr("game-catalog-exact"));
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let glyph = if detached {
                    icon::ARROW_SQUARE_IN
                } else {
                    icon::ARROW_SQUARE_OUT
                };
                let tip = if detached {
                    "viewer-attach"
                } else {
                    "game-window"
                };
                if style::icon_button(ui, true, glyph, &i18n.tr(tip)).clicked() {
                    toggle = true;
                }
                let has_target = !targets.is_empty();
                if style::icon_button(
                    ui,
                    has_target && self.export.is_none(),
                    icon::EXPORT,
                    &i18n.tr("game-export"),
                )
                .clicked()
                {
                    self.open_export(ctx);
                }
                if self.importing {
                    ui.spinner();
                } else if style::icon_button(
                    ui,
                    has_target,
                    icon::DOWNLOAD_SIMPLE,
                    &i18n.tr("game-import"),
                )
                .clicked()
                {
                    self.read_game(ctx, true);
                }
                if style::icon_button(
                    ui,
                    !self.scanning,
                    icon::ARROWS_CLOCKWISE,
                    &i18n.tr("game-rescan"),
                )
                .clicked()
                {
                    self.scan();
                }
            });
        });
        toggle
    }

    /// How the game numbers the virtual joysticks: one small field per device.
    fn device_links(&mut self, ui: &mut egui::Ui, i18n: &I18n, ctx: &Context, ops: &mut Vec<Op>) {
        let Some(game) = self.game_id() else {
            return;
        };
        let ours = our_devices(ctx);
        if ours.is_empty() {
            return;
        }
        let current: Vec<GameDevice> = ctx
            .profile
            .game_bindings(&game)
            .map(|g| g.devices.clone())
            .unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(icon::LINK).weak())
                .on_hover_text(i18n.tr("game-devices"));
            let mut changed = None;
            for d in &ours {
                let n: u32 = d.output.trim_start_matches("vjoy:").parse().unwrap_or(0);
                ui.label(i18n.trf("game-joystick-n", &[("n", n.into())]));
                ui.label(RichText::new(icon::ARROW_RIGHT).weak());
                let mut value = current
                    .iter()
                    .find(|g| g.output == d.output)
                    .map(|g| g.game_device.clone())
                    .unwrap_or_default();
                let r = ui.add(
                    style::text_field(&mut value)
                        .desired_width(56.0)
                        .hint_text("?"),
                );
                r.on_hover_text(i18n.tr("game-devices"));
                if value
                    != current
                        .iter()
                        .find(|g| g.output == d.output)
                        .map(|g| g.game_device.clone())
                        .unwrap_or_default()
                {
                    changed = Some((d.output.clone(), value));
                }
                ui.add_space(style::SPACE_M);
            }
            if let Some((output, value)) = changed {
                let mut devices: Vec<GameDevice> =
                    current.into_iter().filter(|g| g.output != output).collect();
                let value = value.trim().to_string();
                if !value.is_empty() {
                    devices.push(GameDevice {
                        output,
                        game_device: value,
                    });
                }
                devices.sort_by(|a, b| a.output.cmp(&b.output));
                ops.push(Op::GameDevices { game, devices });
            }
        });
    }

    /// Reads the bindings in the game (shown as "In the game"); `import` takes them into the
    /// profile too.
    fn read_game(&mut self, ctx: &Context, import: bool) {
        let Some(found) = self.found().cloned() else {
            return;
        };
        self.importing = import;
        let plugin = self.selected;
        self.send(Request::Read {
            plugin,
            target: found.target,
            version: found.version,
            ours: our_devices(ctx),
            import,
        });
    }

    /// Offers the game's bindings when the profile has none for the game yet.
    fn import_banner(&mut self, ui: &mut egui::Ui, i18n: &I18n, ctx: &Context) {
        let Some(game) = self.game_id() else {
            return;
        };
        let n = match &self.in_game {
            Some((p, read)) if *p == self.selected => read.binds.len(),
            _ => 0,
        };
        if n == 0
            || ctx
                .profile
                .game_bindings(&game)
                .is_some_and(|g| !g.binds.is_empty())
        {
            return;
        }
        ui.add_space(style::SPACE_S);
        egui::Frame::new()
            .fill(style::accent(ui).gamma_multiply(0.15))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::DOWNLOAD_SIMPLE).color(style::accent(ui)));
                    ui.label(i18n.trf("game-import-offer", &[("n", (n as i64).into())]));
                    if ui.button(i18n.tr("game-import-take")).clicked() {
                        self.read_game(ctx, true);
                    }
                });
            });
    }

    fn open_export(&mut self, ctx: &Context) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        let options: BTreeMap<String, OptionValue> = plugin
            .options
            .iter()
            .map(|o| (o.key.clone(), o.default.clone()))
            .collect();
        self.export = Some(ExportDialog {
            options,
            preview: None,
            busy: false,
        });
        self.preview(ctx);
    }

    fn preview(&mut self, ctx: &Context) {
        let (Some(game), Some(found)) = (self.game_id(), self.found().cloned()) else {
            return;
        };
        let Some(d) = &mut self.export else {
            return;
        };
        d.busy = true;
        d.preview = None;
        let options = d.options.clone();
        let g = ctx.profile.game_bindings(&game);
        let binds = g
            .map(|g| {
                g.binds
                    .iter()
                    .map(|b| Bind {
                        action: b.action.clone(),
                        output: b.output.to_string(),
                        invert: b.invert,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let devices = g
            .map(|g| {
                g.devices
                    .iter()
                    .map(|d| DeviceLink {
                        output: d.output.clone(),
                        game_device: d.game_device.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let plugin = self.selected;
        self.send(Request::Preview {
            plugin,
            target: found.target,
            version: found.version,
            ours: our_devices(ctx),
            devices,
            binds,
            profile: ctx.profile_name.to_string(),
            options,
        });
    }

    fn export_dialog(&mut self, ui: &mut egui::Ui, i18n: &I18n, ctx: &Context, _game: &str) {
        let Some(plugin) = self.plugin().cloned() else {
            return;
        };
        if self.export.is_none() {
            return;
        }
        let mut close = false;
        let mut write = None;
        let mut again = false;
        let title = i18n.trf(
            "game-export-title",
            &[("game", plugin.manifest.name.clone().into())],
        );
        style::modal(ui.ctx(), "game-export").show(ui.ctx(), |ui| {
            let Some(d) = &mut self.export else {
                return;
            };
            ui.set_width(560.0);
            style::dialog_title(ui, Some((icon::EXPORT, None)), &title);
            for o in &plugin.options {
                let value = d.options.entry(o.key.clone()).or_insert(o.default.clone());
                match (o.kind, value) {
                    (OptionKind::Bool, OptionValue::Bool(on)) => {
                        if ui.add(style::switch(on, &o.label)).changed() {
                            again = true;
                        }
                    }
                    (_, OptionValue::Text(text)) => {
                        ui.horizontal(|ui| {
                            ui.label(&o.label);
                            if ui
                                .add(style::text_field(text).desired_width(240.0))
                                .lost_focus()
                            {
                                again = true;
                            }
                        });
                    }
                    (_, OptionValue::Number(n)) => {
                        ui.horizontal(|ui| {
                            ui.label(&o.label);
                            if ui.add(egui::DragValue::new(n)).changed() {
                                again = true;
                            }
                        });
                    }
                    _ => {}
                }
            }
            ui.add_space(style::SPACE_M);
            match &d.preview {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(i18n.tr("game-export-busy"));
                    });
                }
                Some(Err(e)) => {
                    ui.label(
                        RichText::new(
                            i18n.trf("game-export-error", &[("error", e.clone().into())]),
                        )
                        .color(ui.visuals().error_fg_color),
                    );
                }
                Some(Ok((files, result))) => {
                    let catalog = plugin
                        .catalogs
                        .pick(self_version(&self.targets, self.selected, self.target).as_deref())
                        .map(|(c, _)| c.clone())
                        .unwrap_or_default();
                    ui.label(RichText::new(i18n.tr("game-export-changes")).strong());
                    if result.changes.is_empty() {
                        ui.label(RichText::new(i18n.tr("game-export-none")).weak());
                    }
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for c in &result.changes {
                                let (glyph, key) = match c.kind {
                                    ChangeKind::Added => (icon::PLUS, "game-added"),
                                    ChangeKind::Changed => (icon::PENCIL_SIMPLE, "game-changed"),
                                    ChangeKind::Removed => (icon::MINUS, "game-removed"),
                                };
                                let name = catalog
                                    .action(&c.action)
                                    .map_or(c.action.as_str(), |a| a.name.as_str());
                                let detail = match (&c.from, &c.to) {
                                    (Some(f), Some(t)) => format!("{f} → {t}"),
                                    (None, Some(t)) => t.clone(),
                                    (Some(f), None) => f.clone(),
                                    (None, None) => String::new(),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(glyph).on_hover_text(i18n.tr(key));
                                    ui.label(name);
                                    ui.label(RichText::new(detail).weak().monospace());
                                });
                            }
                        });
                    ui.add_space(style::SPACE_S);
                    ui.label(RichText::new(i18n.tr("game-export-files")).strong());
                    let found = self
                        .targets
                        .get(self.selected)
                        .and_then(|t| t.get(self.target));
                    for (key, text) in &result.files {
                        let same =
                            files.get(key).and_then(|f| f.text.as_deref()) == Some(text.as_str());
                        if !same && let Some(path) = found.and_then(|f| f.target.files.get(key)) {
                            ui.label(RichText::new(path).weak().monospace());
                        }
                    }
                    for new in &result.create {
                        if let Some(dir) = found.and_then(|f| f.target.dirs.get(&new.dir)) {
                            ui.label(
                                RichText::new(format!("{dir}/{}", new.name))
                                    .weak()
                                    .monospace(),
                            );
                        }
                    }
                    ui.label(RichText::new(i18n.tr("game-export-backup")).weak());
                }
            }
            let ready = matches!(d.preview, Some(Ok(_)));
            let buttons = [
                style::DialogButton::primary(i18n.tr("game-export-write")).enabled(ready),
                style::DialogButton::secondary(i18n.tr("cancel")),
            ];
            match style::dialog_buttons(ui, &buttons) {
                Some(0) => {
                    if let Some(Ok((files, result))) = d.preview.take() {
                        write = Some((files, result));
                    }
                }
                Some(_) => close = true,
                None => {}
            }
        });
        if close {
            self.export = None;
        } else if let Some((files, result)) = write {
            if let Some(found) = self.found().cloned() {
                if let Some(d) = &mut self.export {
                    d.busy = true;
                }
                self.send(Request::Write {
                    target: found.target,
                    files,
                    result,
                });
            }
        } else if again {
            self.preview(ctx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wiring(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        game: &str,
        graph: &Graph,
        live: &Live,
        now: f64,
        ops: &mut Vec<Op>,
    ) {
        if graph.outputs.is_empty() {
            crate::app::empty_state(ui, icon::FLOW_ARROW, &i18n.tr("game-no-outputs"));
            return;
        }
        let accent = style::accent(ui);
        let weak = ui.visuals().weak_text_color();
        let text = ui.visuals().text_color();
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let fill = ui.visuals().faint_bg_color;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let width = ui.available_width();
                let rows = graph.rows + 1;
                let (rect, _) = ui
                    .allocate_exact_size(Vec2::new(width, rows as f32 * ROW + ROW), Sense::hover());
                let col_w = (width - 2.0 * 80.0) / 3.0;
                let xs = [
                    rect.left(),
                    rect.left() + col_w + 80.0,
                    rect.left() + 2.0 * (col_w + 80.0),
                ];
                let painter = ui.painter_at(rect);
                let font = FontId::proportional(13.0);
                for (x, key) in
                    xs.iter()
                        .zip(["game-col-inputs", "game-col-outputs", "game-col-actions"])
                {
                    painter.text(
                        Pos2::new(*x + 8.0, rect.top() + ROW / 2.0),
                        Align2::LEFT_CENTER,
                        i18n.tr(key),
                        FontId::proportional(12.0),
                        weak,
                    );
                }
                let node = |col: usize, row: usize| {
                    Rect::from_min_size(
                        Pos2::new(xs[col], rect.top() + ROW + row as f32 * ROW + 2.0),
                        Vec2::new(col_w, ROW - 4.0),
                    )
                };
                let glow = |v: f32| accent.gamma_multiply(v.clamp(0.0, 1.0));
                // Wires first, under the nodes.
                for (l, m) in &graph.input_edges {
                    let on = live
                        .inputs
                        .get(*l)
                        .copied()
                        .unwrap_or(0.0)
                        .min(live.outputs.get(*m).copied().unwrap_or(0.0));
                    wire(
                        &painter,
                        node(0, graph.input_rows[*l]).right_center(),
                        node(1, *m).left_center(),
                        if on > 0.0 {
                            Stroke::new(2.5, glow(0.4 + on))
                        } else {
                            Stroke::new(1.0, line)
                        },
                    );
                }
                for (m, r) in &graph.action_edges {
                    let lit = live.outputs.get(*m).copied().unwrap_or(0.0);
                    let fade = self.fade(&graph.actions[*r].id, now);
                    let on = lit.max(fade);
                    wire(
                        &painter,
                        node(1, *m).right_center(),
                        node(2, graph.action_rows[*r]).left_center(),
                        if on > 0.0 {
                            Stroke::new(2.5, glow(0.4 + on))
                        } else {
                            Stroke::new(1.0, line)
                        },
                    );
                }
                // Nodes.
                for (i, n) in graph.inputs.iter().enumerate() {
                    let v = live.inputs.get(i).copied().unwrap_or(0.0);
                    let r = node(0, graph.input_rows[i]);
                    draw_node(
                        &painter,
                        r,
                        &n.label,
                        v,
                        n.axis.then_some(v),
                        accent,
                        fill,
                        text,
                        &font,
                    );
                }
                for (i, o) in graph.outputs.iter().enumerate() {
                    let v = live.outputs.get(i).copied().unwrap_or(0.0);
                    let r = node(1, i);
                    let picked = self.picked.as_ref() == Some(o);
                    draw_node(
                        &painter,
                        r,
                        &output_name(i18n, o),
                        v,
                        None,
                        accent,
                        if picked {
                            accent.gamma_multiply(0.35)
                        } else {
                            fill
                        },
                        text,
                        &font,
                    );
                    let resp = ui.interact(r, ui.scope_id().with(("game-out", i)), Sense::click());
                    if resp.clicked() {
                        self.picked = if picked { None } else { Some(o.clone()) };
                    }
                    resp.on_hover_text(i18n.tr("game-pick-output-hint"));
                }
                for (i, a) in graph.actions.iter().enumerate() {
                    let v = live
                        .actions
                        .get(i)
                        .copied()
                        .unwrap_or(0.0)
                        .max(self.fade(&a.id, now));
                    let r = node(2, graph.action_rows[i]);
                    let label = if a.known {
                        a.name.clone()
                    } else {
                        format!("{}  {}", icon::WARNING, a.name)
                    };
                    draw_node(
                        &painter,
                        r,
                        &label,
                        v,
                        None,
                        accent,
                        fill,
                        if a.bound { text } else { weak },
                        &font,
                    );
                    let resp = ui.interact(r, ui.scope_id().with(("game-act", i)), Sense::click());
                    let mut tip = if a.category.is_empty() {
                        a.id.clone()
                    } else {
                        format!("{}\n{}", a.category, a.id)
                    };
                    if !a.known {
                        tip.push('\n');
                        tip.push_str(&i18n.tr("game-not-in-catalog"));
                    }
                    if let Some(o) = self.picked.clone() {
                        tip.push('\n');
                        tip.push_str(&i18n.tr("game-click-to-link"));
                        if resp.clicked() {
                            ops.push(Op::GameBind {
                                game: game.to_string(),
                                action: a.id.clone(),
                                output: o,
                            });
                            self.picked = None;
                        }
                    }
                    if a.bound {
                        let bin = Rect::from_center_size(
                            r.right_center() - Vec2::new(12.0, 0.0),
                            Vec2::splat(18.0),
                        );
                        if resp.hovered() || ui.rect_contains_pointer(bin) {
                            painter.text(
                                bin.center(),
                                Align2::CENTER_CENTER,
                                icon::TRASH,
                                FontId::proportional(13.0),
                                weak,
                            );
                            let b = ui.interact(
                                bin,
                                ui.scope_id().with(("game-unbind", i)),
                                Sense::click(),
                            );
                            if b.on_hover_text(i18n.tr("game-unbind")).clicked() {
                                ops.push(Op::GameUnbind {
                                    game: game.to_string(),
                                    action: a.id.clone(),
                                    output: None,
                                });
                            }
                        }
                    }
                    resp.on_hover_text(tip);
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        game: &str,
        graph: &Graph,
        live: &Live,
        now: f64,
        ops: &mut Vec<Op>,
    ) {
        let in_game: HashMap<String, Vec<String>> = match &self.in_game {
            Some((p, read)) if *p == self.selected => {
                let mut m: HashMap<String, Vec<String>> = HashMap::new();
                for b in &read.binds {
                    m.entry(b.action.clone())
                        .or_default()
                        .push(b.output.clone());
                }
                m
            }
            _ => HashMap::new(),
        };
        let accent = style::accent(ui);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Grid::new("game-list")
                    .striped(true)
                    .num_columns(4)
                    .spacing([style::SPACE_L, style::SPACE_S])
                    .show(ui, |ui| {
                        for key in [
                            "game-col-actions",
                            "game-in-game",
                            "game-in-profile",
                            "game-from",
                        ] {
                            ui.label(RichText::new(i18n.tr(key)).weak());
                        }
                        ui.end_row();
                        let mut last_category = "";
                        for (r, a) in graph.actions.iter().enumerate() {
                            if a.category != last_category {
                                ui.label(RichText::new(&a.category).strong());
                                ui.end_row();
                                last_category = &a.category;
                            }
                            let lit = live
                                .actions
                                .get(r)
                                .copied()
                                .unwrap_or(0.0)
                                .max(self.fade(&a.id, now));
                            let name = RichText::new(&a.name);
                            let name = if lit > 0.0 {
                                name.color(accent).strong()
                            } else {
                                name
                            };
                            let resp = ui.label(name).on_hover_text(&a.id);
                            // Scrolled to when it lights up, not while it stays lit.
                            if live.actions.get(r).copied().unwrap_or(0.0) > 0.0
                                && self.lit.insert(a.id.clone())
                            {
                                resp.scroll_to_me(None);
                            }
                            let game_side = in_game
                                .get(a.id.as_str())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|o| GameOutput::parse(o))
                                        .map(|o| output_name(i18n, &o))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default();
                            ui.label(RichText::new(game_side).weak());
                            let outs: Vec<usize> = graph
                                .action_edges
                                .iter()
                                .filter(|(_, x)| *x == r)
                                .map(|(m, _)| *m)
                                .collect();
                            ui.horizontal(|ui| {
                                ui.label(
                                    outs.iter()
                                        .map(|m| output_name(i18n, &graph.outputs[*m]))
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                );
                                if let Some(o) = self.picked.clone()
                                    && style::icon_button(
                                        ui,
                                        true,
                                        icon::LINK,
                                        &i18n.tr("game-click-to-link"),
                                    )
                                    .clicked()
                                {
                                    ops.push(Op::GameBind {
                                        game: game.to_string(),
                                        action: a.id.clone(),
                                        output: o,
                                    });
                                    self.picked = None;
                                }
                                if a.bound
                                    && style::icon_button(
                                        ui,
                                        true,
                                        icon::TRASH,
                                        &i18n.tr("game-unbind"),
                                    )
                                    .clicked()
                                {
                                    ops.push(Op::GameUnbind {
                                        game: game.to_string(),
                                        action: a.id.clone(),
                                        output: None,
                                    });
                                }
                            });
                            let sources: BTreeSet<&str> = outs
                                .iter()
                                .flat_map(|m| {
                                    graph
                                        .input_edges
                                        .iter()
                                        .filter(move |(_, x)| x == m)
                                        .map(|(l, _)| graph.inputs[*l].label.as_str())
                                })
                                .collect();
                            ui.label(
                                RichText::new(sources.into_iter().collect::<Vec<_>>().join(", "))
                                    .weak(),
                            );
                            ui.end_row();
                        }
                    });
            });
    }

    fn fade(&self, action: &str, now: f64) -> f32 {
        self.flash
            .get(action)
            .map_or(0.0, |t| (1.0 - (now - t) / FLASH).max(0.0) as f32)
    }
}

fn self_version(targets: &[Vec<FoundTarget>], plugin: usize, target: usize) -> Option<String> {
    targets.get(plugin)?.get(target)?.version.clone()
}

/// The status bar text for a status code the worker replies left.
fn status_text(i18n: &I18n, code: &str, plugin: Option<&PluginInfo>) -> String {
    let game = plugin.map_or(String::new(), |p| p.manifest.name.clone());
    if let Some(e) = code.strip_prefix("error:") {
        return i18n.trf("game-export-error", &[("error", e.to_string().into())]);
    }
    if let Some(n) = code.strip_prefix("written:") {
        let n: i64 = n.parse().unwrap_or(0);
        return i18n.trf("game-exported", &[("game", game.into()), ("n", n.into())]);
    }
    let n: i64 = code.parse().unwrap_or(0);
    i18n.trf("game-imported", &[("n", n.into())])
}

/// Edits that add what the game has and the profile lacks; the number of bindings added.
fn import_ops(profile: &Profile, game: &str, read: &ReadResult, ops: &mut Vec<Op>) -> usize {
    let have = profile.game_bindings(game);
    let mut n = 0;
    for b in &read.binds {
        let Some(output) = GameOutput::parse(&b.output) else {
            continue;
        };
        if have.is_some_and(|g| {
            g.binds
                .iter()
                .any(|x| x.action == b.action && x.output == output)
        }) {
            continue;
        }
        ops.push(Op::GameBind {
            game: game.to_string(),
            action: b.action.clone(),
            output,
        });
        n += 1;
    }
    if !read.devices.is_empty() {
        let mut devices: Vec<GameDevice> = have.map(|g| g.devices.clone()).unwrap_or_default();
        for d in &read.devices {
            devices.retain(|x| x.output != d.output);
            devices.push(GameDevice {
                output: d.output.clone(),
                game_device: d.game_device.clone(),
            });
        }
        devices.sort_by(|a, b| a.output.cmp(&b.output));
        ops.push(Op::GameDevices {
            game: game.to_string(),
            devices,
        });
    }
    n
}

/// The profile's virtual joysticks as games see them.
fn our_devices(ctx: &Context) -> Vec<OurDevice> {
    let mut ids: BTreeSet<u32> = ctx.vjoy.iter().map(|d| d.id).collect();
    ids.extend(ctx.profile.virtual_devices.joysticks.iter().map(|j| j.id));
    ids.into_iter()
        .map(|id| {
            let stored = ctx
                .profile
                .virtual_devices
                .joysticks
                .iter()
                .find(|j| j.id == id);
            let name = ctx
                .vjoy
                .iter()
                .find(|d| d.id == id)
                .and_then(|d| d.name.clone())
                .or_else(|| stored.and_then(|j| j.name.clone()))
                .unwrap_or_else(|| "vJoy Device".into());
            // DirectInput's product GUID: product id, vendor id, then "PIDVID".
            let product = stored.and_then(|j| j.product).unwrap_or(0xBEAD);
            OurDevice {
                output: format!("vjoy:{id}"),
                name,
                guid: format!("{{{product:04X}1234-0000-0000-0000-504944564944}}"),
            }
        })
        .collect()
}

/// An output's name as the page shows it.
fn output_name(i18n: &I18n, o: &GameOutput) -> String {
    let base = |v: &VirtualOutput| match v {
        VirtualOutput::Vjoy {
            device,
            input_type,
            id,
        } => {
            let dev = i18n.trf("game-joystick-n", &[("n", (*device).into())]);
            let what = match input_type {
                InputType::Axis => crate::devices_view::axis_name(i18n, *id),
                InputType::Hat => i18n.trf("vjoy-hat-n", &[("n", (*id).into())]),
                _ => i18n.trf("vjoy-button-n", &[("n", (*id).into())]),
            };
            format!("{dev} · {what}")
        }
        VirtualOutput::Gamepad { gamepad, control } => {
            format!("{} {gamepad} · {control}", i18n.tr("game-gamepad"))
        }
        VirtualOutput::Logical { input_type, id } => {
            format!("{} · {:?} {id}", i18n.tr("game-logical"), input_type)
        }
    };
    match o {
        GameOutput::Virtual(v) => base(v),
        GameOutput::HatDirection(v, d) => format!("{} {}", base(v), arrow(*d)),
        GameOutput::Key(k) => i18n.trf(
            "game-key",
            &[("key", crate::key_capture::name(*k, i18n).into())],
        ),
    }
}

fn arrow(d: HatDirection) -> &'static str {
    match d {
        HatDirection::Up => "↑",
        HatDirection::Right => "→",
        HatDirection::Down => "↓",
        HatDirection::Left => "←",
    }
}

// ---- the schematic's data ------------------------------------------------------------------------

struct InputNode {
    /// (profile device id, input type, input id).
    key: (String, String, String),
    label: String,
    axis: bool,
}

struct ActionNode {
    id: String,
    name: String,
    category: String,
    /// Bound to an output of the profile.
    bound: bool,
    /// In the catalog of the game version.
    known: bool,
}

/// Columns and wires of the schematic.
struct Graph {
    inputs: Vec<InputNode>,
    outputs: Vec<GameOutput>,
    actions: Vec<ActionNode>,
    /// (input, output) indices.
    input_edges: Vec<(usize, usize)>,
    /// (output, action) indices.
    action_edges: Vec<(usize, usize)>,
    /// The row of each input and action: next to the first output they connect to.
    input_rows: Vec<usize>,
    action_rows: Vec<usize>,
    rows: usize,
}

/// Rows for nodes wired to outputs: each as close as it gets to the first output it connects
/// to, in that order, never two in one row.
fn align(count: usize, edges: impl Iterator<Item = (usize, usize)>) -> Vec<usize> {
    let mut want = vec![usize::MAX; count];
    for (node, output) in edges {
        want[node] = want[node].min(output);
    }
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by_key(|&i| (want[i], i));
    let mut rows = vec![0; count];
    let mut next = 0;
    for i in order {
        let row = if want[i] == usize::MAX {
            next
        } else {
            want[i].max(next)
        };
        rows[i] = row;
        next = row + 1;
    }
    rows
}

impl Graph {
    fn build(ctx: &Context, i18n: &I18n, game: &str, catalog: &Catalog, search: &str) -> Self {
        let profile = ctx.profile;
        let usage = profile.output_usage(None);
        let binds = profile.game_bindings(game);
        // Outputs: what the profile writes (hats as their directions) and the keys bound.
        let mut outputs: Vec<GameOutput> = Vec::new();
        let mut sources: HashMap<GameOutput, Vec<usize>> = HashMap::new();
        for (v, uses) in &usage {
            let list: Vec<GameOutput> = match v {
                VirtualOutput::Vjoy {
                    input_type: InputType::Hat,
                    ..
                } => HatDirection::ALL
                    .iter()
                    .map(|d| GameOutput::HatDirection(v.clone(), *d))
                    .collect(),
                VirtualOutput::Gamepad { control, .. } if control == "dpad" => HatDirection::ALL
                    .iter()
                    .map(|d| GameOutput::HatDirection(v.clone(), *d))
                    .collect(),
                _ => vec![GameOutput::Virtual(v.clone())],
            };
            for o in list {
                sources.insert(o.clone(), uses.iter().map(|u| u.binding).collect());
                outputs.push(o);
            }
        }
        if let Some(g) = binds {
            for b in &g.binds {
                if !outputs.contains(&b.output) {
                    outputs.push(b.output.clone());
                }
            }
        }
        outputs.sort_by_key(sort_key);
        // Inputs: the bindings writing those outputs.
        let mut inputs: Vec<InputNode> = Vec::new();
        let mut input_edges = Vec::new();
        for (m, o) in outputs.iter().enumerate() {
            for &b in sources.get(o).into_iter().flatten() {
                let Some(binding) = profile.inputs.get(b) else {
                    continue;
                };
                let key = (
                    binding.device_id.clone(),
                    binding.input_type.clone(),
                    binding.input_id.clone(),
                );
                let l = match inputs.iter().position(|n| n.key == key) {
                    Some(l) => l,
                    None => {
                        inputs.push(InputNode {
                            label: input_label(i18n, profile, &key),
                            axis: binding.input_type() == InputType::Axis,
                            key,
                        });
                        inputs.len() - 1
                    }
                };
                if !input_edges.contains(&(l, m)) {
                    input_edges.push((l, m));
                }
            }
        }
        // Actions: those bound, and the catalog's matches of a search.
        let mut actions: Vec<ActionNode> = Vec::new();
        let mut action_edges = Vec::new();
        let node = |id: &str, bound: bool| {
            let a = catalog.action(id);
            ActionNode {
                id: id.to_string(),
                name: a.map_or_else(|| id.to_string(), |a| a.name.clone()),
                category: a.map_or_else(String::new, |a| a.category.clone()),
                bound,
                known: a.is_some(),
            }
        };
        if let Some(g) = binds {
            for b in &g.binds {
                if !actions.iter().any(|a| a.id == b.action) {
                    actions.push(node(&b.action, true));
                }
            }
        }
        let search = search.trim().to_lowercase();
        if !search.is_empty() {
            actions.retain(|a| matches(a, &search));
            let extra: Vec<ActionNode> = catalog
                .actions
                .iter()
                .filter(|a| !actions.iter().any(|x| x.id == a.id))
                .map(|a| node(&a.id, false))
                .filter(|a| matches(a, &search))
                .take(SEARCH_LIMIT)
                .collect();
            actions.extend(extra);
        }
        actions.sort_by(|a, b| (&a.category, &a.name).cmp(&(&b.category, &b.name)));
        if let Some(g) = binds {
            for b in &g.binds {
                if let (Some(m), Some(r)) = (
                    outputs.iter().position(|o| *o == b.output),
                    actions.iter().position(|a| a.id == b.action),
                ) {
                    action_edges.push((m, r));
                }
            }
        }
        let input_rows = align(inputs.len(), input_edges.iter().copied());
        let action_rows = align(actions.len(), action_edges.iter().map(|(m, r)| (*r, *m)));
        let rows = input_rows
            .iter()
            .chain(&action_rows)
            .map(|r| r + 1)
            .chain([outputs.len()])
            .max()
            .unwrap_or(0);
        Self {
            inputs,
            outputs,
            actions,
            input_edges,
            action_edges,
            input_rows,
            action_rows,
            rows,
        }
    }
}

fn matches(a: &ActionNode, search: &str) -> bool {
    a.name.to_lowercase().contains(search)
        || a.id.to_lowercase().contains(search)
        || a.category.to_lowercase().contains(search)
}

fn sort_key(o: &GameOutput) -> (u8, u32, u8, u32, u8, String) {
    let t = |t: &InputType| match t {
        InputType::Axis => 0,
        InputType::Button => 1,
        _ => 2,
    };
    match o {
        GameOutput::Virtual(VirtualOutput::Vjoy {
            device,
            input_type,
            id,
        }) => (0, *device, t(input_type), *id, 0, String::new()),
        GameOutput::HatDirection(VirtualOutput::Vjoy { device, id, .. }, d) => {
            (0, *device, 2, *id, *d as u8, String::new())
        }
        GameOutput::Key(k) => (3, 0, 0, gob_core::key_input_id(*k), 0, String::new()),
        other => (2, 0, 0, 0, 0, other.to_string()),
    }
}

fn input_label(
    i18n: &I18n,
    profile: &Profile,
    (device, kind, id): &(String, String, String),
) -> String {
    let name = crate::devices_view::device_name(i18n, profile, device);
    format!(
        "{name} · {}",
        crate::output_marks::input_name(i18n, kind, id)
    )
}

/// How lit each node is now, 0 to 1.
struct Live {
    inputs: Vec<f32>,
    outputs: Vec<f32>,
    actions: Vec<f32>,
    any: bool,
}

impl Live {
    fn read(ctx: &Context, graph: &Graph) -> Self {
        let aliases = crate::substitute_view::auto_aliases(ctx.profile, ctx.devices);
        let state = |device: &str| {
            let id = aliases.get(device).map_or(device, String::as_str);
            ctx.devices
                .iter()
                .find(|d| d.info.guid.eq_ignore_ascii_case(id))
        };
        // Per input: lit amount, and the hat angle for hat inputs.
        let mut hats: Vec<Option<u16>> = Vec::with_capacity(graph.inputs.len());
        let inputs: Vec<f32> = graph
            .inputs
            .iter()
            .map(|n| {
                let (device, kind, id) = &n.key;
                let index = id.trim().parse::<usize>().unwrap_or(0).wrapping_sub(1);
                let s = state(device);
                let mut hat = None;
                let v = match kind.as_str() {
                    "button" => s
                        .and_then(|s| s.buttons.get(index).copied())
                        .map_or(0.0, |b| if b { 1.0 } else { 0.0 }),
                    // Lit past a dead zone, so a resting axis a little off center stays dark.
                    "axis" => s
                        .and_then(|s| s.axes.get(index).copied())
                        .map_or(0.0, |v| ((v.abs() - 0.15) / 0.5).clamp(0.0, 1.0)),
                    "hat" => {
                        hat = s.and_then(|s| s.hats.get(index).copied()).flatten();
                        if hat.is_some() { 1.0 } else { 0.0 }
                    }
                    _ => 0.0,
                };
                hats.push(hat);
                v
            })
            .collect();
        let mut outputs = vec![0.0f32; graph.outputs.len()];
        for (l, m) in &graph.input_edges {
            let mut v = inputs[*l];
            if let GameOutput::HatDirection(_, d) = &graph.outputs[*m] {
                v = match hats[*l] {
                    Some(angle) if hat_has(angle, *d) => 1.0,
                    _ => 0.0,
                };
            }
            outputs[*m] = outputs[*m].max(v);
        }
        let mut actions = vec![0.0f32; graph.actions.len()];
        for (m, r) in &graph.action_edges {
            actions[*r] = actions[*r].max(outputs[*m]);
        }
        let any = inputs.iter().any(|v| *v > 0.0);
        Self {
            inputs,
            outputs,
            actions,
            any,
        }
    }
}

/// Whether a hat at `angle` (hundredths of a degree, 0 = up, clockwise) points `d`, diagonals
/// counting for both directions.
fn hat_has(angle: u16, d: HatDirection) -> bool {
    let a = f32::from(angle) / 100.0;
    let center = match d {
        HatDirection::Up => 0.0,
        HatDirection::Right => 90.0,
        HatDirection::Down => 180.0,
        HatDirection::Left => 270.0,
    };
    let diff = ((a - center + 540.0) % 360.0 - 180.0).abs();
    diff < 67.5
}

// ---- drawing -------------------------------------------------------------------------------------

fn wire(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let dx = ((to.x - from.x) / 2.0).max(20.0);
    let shape = egui::epaint::CubicBezierShape::from_points_stroke(
        [from, from + Vec2::new(dx, 0.0), to - Vec2::new(dx, 0.0), to],
        false,
        Color32::TRANSPARENT,
        stroke,
    );
    painter.add(shape);
}

#[allow(clippy::too_many_arguments)]
fn draw_node(
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    lit: f32,
    bar: Option<f32>,
    accent: Color32,
    fill: Color32,
    text: Color32,
    font: &FontId,
) {
    let radius = 6.0;
    painter.rect_filled(rect, radius, fill);
    if let Some(v) = bar {
        let w = rect.width() * v.clamp(0.0, 1.0);
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
            radius,
            accent.gamma_multiply(0.25),
        );
    }
    let stroke = if lit > 0.0 {
        Stroke::new(2.0, accent.gamma_multiply(0.4 + 0.6 * lit.min(1.0)))
    } else {
        Stroke::new(1.0, text.gamma_multiply(0.25))
    };
    painter.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    let galley = painter.layout(label.to_string(), font.clone(), text, rect.width() - 28.0);
    let pos = Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
    painter
        .with_clip_rect(rect.shrink(2.0))
        .galley(pos, galley, text);
}
