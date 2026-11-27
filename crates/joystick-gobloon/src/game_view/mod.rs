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

mod bind;
mod capture;
mod device;
mod export_dialog;
mod graph;
mod state;
mod table;
mod wiring;
mod worker;

use export_dialog::ExportDialog;
use graph::{Graph, Live};
use state::{InGame, State};
use worker::{Reply, Request, spawn_worker};

/// Height of one node in the wiring schematic.
const ROW: f32 = 28.0;
/// Space between the schematic's columns, where the wires run.
const GAP: f32 = 56.0;
/// How long a game action stays lit after its input lets go, in seconds.
const FLASH: f64 = 1.0;
/// Catalog actions listed for a search.
const SEARCH_LIMIT: usize = 60;

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

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    /// The game's actions as a table.
    #[default]
    List,
    Wiring,
    /// A connected controller's picture with the actions on its controls.
    Device,
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
    device: device::DeviceTab,
    search: String,
    /// The schematic's picked output: the next game action clicked gets it.
    picked: Option<GameOutput>,
    /// The table's selected game action: a pressed control goes to it.
    chosen: Option<String>,
    /// The control looked up by pressing: the table lists only the actions using it.
    find: Option<GameOutput>,
    filter: table::Filter,
    /// Groups of the table folded away.
    folded: std::collections::HashSet<String>,
    /// The wait for a press and the dialogs after it.
    dialog: Option<bind::Dialog>,
    /// Text for the status bar.
    note: Option<String>,
    /// The table was drawn already.
    table_shown: bool,
    /// Folders the user pointed at, searched for the game besides the usual places.
    folders: Vec<String>,
    /// The folder just chosen: its install is selected once the search is done.
    chosen_folder: Option<String>,
    /// A folder chosen this frame, for the app to keep in the settings.
    new_folder: Option<String>,
    /// The first search after loading may move to a game that was found: the profile names
    /// none and no game was picked yet.
    pick_found: bool,
    debug_done: bool,
    /// When each game action was last lit.
    flash: HashMap<String, f64>,
    /// Game actions lit in the last frame.
    lit: std::collections::HashSet<String>,
    /// The bindings last read from the game.
    in_game: Option<std::sync::Arc<InGame>>,
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
    /// A folder to search for games from now on (kept in the settings).
    pub add_folder: Option<String>,
}

/// Everything the page reads from the app.
pub struct Context<'a> {
    pub profile: &'a Profile,
    pub profile_name: &'a str,
    pub devices: &'a [DeviceState],
    pub vjoy: &'a [gob_virtual::DeviceInfo],
    /// What the profile sends while it runs.
    pub sent: Option<gob_engine::OutputTap>,
    /// The mode the editor shows: a control mapped from this page is mapped in it.
    pub mode: &'a str,
    /// Folders the user chose to search for games (from the settings).
    pub folders: &'a [String],
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

    /// Looks for the games: in the usual places, and in the folders the user chose. A chosen
    /// folder may be the game's own or the one holding it, so its parent is searched too.
    fn scan(&mut self, ctx: &Context) {
        self.scanning = true;
        for f in ctx.folders {
            if !self.folders.contains(f) {
                self.folders.push(f.clone());
            }
        }
        let mut roots = crate::platform::game_roots();
        for f in &self.folders {
            let folder = std::path::Path::new(f);
            for dir in [Some(folder), folder.parent()].into_iter().flatten() {
                let root = slashed(&dir.to_string_lossy());
                if !root.is_empty() && !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
        let documents = crate::platform::documents_dir()
            .map(|d| format!("{}/", slashed(&d.to_string_lossy())))
            .unwrap_or_default();
        let local_app_data = crate::platform::local_app_data_dir()
            .map(|d| format!("{}/", slashed(&d.to_string_lossy())))
            .unwrap_or_default();
        let env = gob_export::Env {
            platform: if cfg!(windows) { "windows" } else { "linux" }.into(),
            roots,
            documents,
            local_app_data,
        };
        self.send(Request::Scan { env });
    }

    /// Asks for the game's folder and searches it.
    fn choose_folder(&mut self, ctx: &Context) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let folder = dir.to_string_lossy().into_owned();
        if !self.folders.contains(&folder) {
            self.folders.push(folder.clone());
            self.new_folder = Some(folder.clone());
        }
        self.chosen_folder = Some(folder);
        self.scan(ctx);
    }

    /// The catalog for the selected target (or the profile's last export), and its fit, with
    /// the actions the game's own files list added. A plugin that takes every action from the
    /// game's files has no catalog: what the files list fits the game exactly.
    fn catalog(&self, profile: &Profile) -> Option<(Catalog, Fit)> {
        let plugin = self.plugin()?;
        let version = self.found().and_then(|f| f.version.clone()).or_else(|| {
            profile
                .game_bindings(&plugin.manifest.id)
                .and_then(|g| g.version.clone())
        });
        let listed = self
            .in_game
            .as_ref()
            .filter(|g| g.plugin == self.selected && !g.actions.is_empty())
            .map(|g| g.actions.as_slice());
        let (mut catalog, fit) = match plugin.catalogs.pick(version.as_deref()) {
            Some((c, f)) => (c.clone(), f),
            None => {
                listed?;
                let catalog = Catalog {
                    version: version.unwrap_or_else(|| "?".into()),
                    actions: Vec::new(),
                };
                (catalog, Fit::Exact)
            }
        };
        for a in listed.into_iter().flatten() {
            if catalog.action(&a.id).is_none() {
                catalog.actions.push(a.clone());
            }
        }
        Some((catalog, fit))
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
                    } else {
                        self.pick_found = true;
                    }
                    self.plugins = Some(plugins);
                    self.errors = errors;
                    self.scan(ctx);
                }
                Reply::Scanned(all) => {
                    self.scanning = false;
                    self.targets = all;
                    // The install in the folder just chosen, of whichever game it is.
                    let chosen = self.chosen_folder.take().and_then(|folder| {
                        let folder = slashed(&folder).to_lowercase();
                        let found = self.targets.iter().enumerate().find_map(|(p, list)| {
                            list.iter()
                                .position(|f| {
                                    f.target.files.values().any(|file| {
                                        slashed(file).to_lowercase().starts_with(&folder)
                                    })
                                })
                                .map(|t| (p, t))
                        });
                        if found.is_none() {
                            self.note = Some(format!("no-game:{folder}"));
                        }
                        found
                    });
                    // Without a game of its own, the profile opens on a game found here.
                    if std::mem::take(&mut self.pick_found)
                        && self.targets.get(self.selected).is_none_or(|t| t.is_empty())
                        && let Some(found) = self.targets.iter().position(|t| !t.is_empty())
                    {
                        self.selected = found;
                    }
                    if let Some((plugin, target)) = chosen {
                        self.selected = plugin;
                        self.target = target;
                        self.in_game = None;
                        self.read_game(ctx, false);
                        continue;
                    }
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
                            self.in_game = Some(std::sync::Arc::new(InGame::new(plugin, &read)));
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
            // Debug builds open a tab with JOYSTICK_GOBLOON_TAB=game-wiring | game-device, for
            // checks.
            if cfg!(debug_assertions) {
                match std::env::var("JOYSTICK_GOBLOON_TAB").as_deref() {
                    Ok("game-wiring") => self.tab = Tab::Wiring,
                    Ok("game-device") => self.tab = Tab::Device,
                    _ => {}
                }
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
                self.tab == Tab::List,
                icon::LIST_BULLETS,
                &i18n.tr("game-tab-list"),
            )
            .clicked()
            {
                self.tab = Tab::List;
            }
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
                self.tab == Tab::Device,
                icon::JOYSTICK,
                &i18n.tr("game-tab-device"),
            )
            .clicked()
            {
                self.tab = Tab::Device;
            }
            ui.add_space(style::SPACE_L);
            ui.label(icon::MAGNIFYING_GLASS);
            ui.add(
                style::text_field(&mut self.search)
                    .hint_text(i18n.tr("game-search"))
                    .desired_width(220.0),
            );
            ui.add_space(style::SPACE_M);
            // A pressed control goes to the table's selected action; with none it is looked up.
            let aim = if self.tab == Tab::List {
                self.chosen.clone()
            } else {
                None
            };
            let tip = if aim.is_some() {
                "game-press-bind"
            } else {
                "game-press-find"
            };
            if style::icon_toggle(ui, self.dialog.is_some(), icon::HAND_TAP, &i18n.tr(tip))
                .clicked()
            {
                self.press_for(aim, None);
            }
            if self.tab == Tab::List {
                egui::ComboBox::from_id_salt("game-filter")
                    .selected_text(format!("{}  {}", icon::FUNNEL, i18n.tr(self.filter.key())))
                    .show_ui(ui, |ui| {
                        for f in table::Filter::ALL {
                            ui.selectable_value(&mut self.filter, f, i18n.tr(f.key()));
                        }
                    });
                // Every group folded or unfolded at once.
                let (glyph, tip) = if self.folded.is_empty() {
                    (icon::ARROWS_IN_LINE_VERTICAL, "game-fold-all")
                } else {
                    (icon::ARROWS_OUT_LINE_VERTICAL, "game-unfold-all")
                };
                if style::icon_button(ui, true, glyph, &i18n.tr(tip)).clicked() {
                    if self.folded.is_empty() {
                        let catalog = self.catalog(ctx.profile).map(|(c, _)| c);
                        self.folded = catalog
                            .iter()
                            .flat_map(|c| c.actions.iter().map(|a| a.category.clone()))
                            .collect();
                    } else {
                        self.folded.clear();
                    }
                }
            }
            if let Some(find) = &self.find {
                ui.add_space(style::SPACE_M);
                ui.label(
                    RichText::new(i18n.trf(
                        "game-find-chip",
                        &[("control", output_name(i18n, find).into())],
                    ))
                    .color(style::accent(ui)),
                );
                if style::icon_button(ui, true, icon::X, &i18n.tr("game-find-clear")).clicked() {
                    self.find = None;
                    self.picked = None;
                }
            }
        });
        if self.tab == Tab::Wiring && self.picked.is_some() {
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
        let in_game = self.in_game.clone().filter(|g| g.plugin == self.selected);
        let state = State::build(
            ctx.profile.game_bindings(&game),
            in_game.as_ref().map(|g| g.binds.as_slice()),
        );
        // The table lists every action of the game, like the game's own screen; the schematic
        // only what is wired.
        let all = self.tab == Tab::List;
        let graph = Graph::build(ctx, i18n, &catalog, &self.search, all, &state);
        let live = Live::read(ctx, &graph);
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
            Tab::Wiring => self.wiring(ui, i18n, &game, &graph, &live, &state, now, ops),
            Tab::List => self.table(ui, i18n, ctx, &game, &graph, &live, &state, now, ops),
            Tab::Device => self.device_tab(ui, i18n, ctx, &catalog, &state, ops),
        }
        let presses: Vec<gob_core::Press> = self
            .plugin()
            .map(|p| {
                p.manifest
                    .presses
                    .iter()
                    .filter_map(|p| gob_core::Press::parse(p))
                    .collect()
            })
            .unwrap_or_default();
        let scene = bind::Scene {
            i18n,
            ctx,
            game: &game,
            catalog: &catalog,
            state: &state,
            presses: &presses,
        };
        self.debug_dialog(&scene);
        self.bind_dialog(ui, &scene, ops);
        self.export_dialog(ui, i18n, ctx, &game);
        if let Some(note) = self.note.take() {
            out.status = Some(match note.strip_prefix("no-game:") {
                Some(folder) => {
                    i18n.trf("game-folder-none", &[("folder", folder.to_string().into())])
                }
                None => note,
            });
        }
        out.add_folder = self.new_folder.take();
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
                            self.pick_found = false;
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
                    self.scan(ctx);
                }
                if style::icon_button(
                    ui,
                    !self.scanning,
                    icon::FOLDER_OPEN,
                    &i18n.tr("game-folder"),
                )
                .clicked()
                {
                    self.choose_folder(ctx);
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
                // Games name devices by a short number ("js1") or a whole GUID.
                let width = (value.chars().count() as f32 * 7.5).clamp(56.0, 300.0);
                let r = ui.add(
                    style::text_field(&mut value)
                        .desired_width(width)
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
        let devices = self
            .game_id()
            .map(|game| device_links(ctx, &game))
            .unwrap_or_default();
        self.send(Request::Read {
            plugin,
            target: found.target,
            version: found.version,
            ours: our_devices(ctx),
            devices,
            import,
        });
    }

    /// Offers the game's bindings when the profile has none for the game yet.
    fn import_banner(&mut self, ui: &mut egui::Ui, i18n: &I18n, ctx: &Context) {
        let Some(game) = self.game_id() else {
            return;
        };
        let n = match &self.in_game {
            Some(g) if g.plugin == self.selected => g.on_devices(),
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

    fn fade(&self, action: &str, now: f64) -> f32 {
        self.flash
            .get(action)
            .map_or(0.0, |t| (1.0 - (now - t) / FLASH).max(0.0) as f32)
    }
}

/// A path with forward slashes and no slash at its end, as plugins write paths.
fn slashed(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
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
    // Keyboard and mouse bindings stay the game's until they are changed here.
    for b in &read.binds {
        let Some(output) = GameOutput::parse(&b.output).filter(|o| o.combo().is_none()) else {
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

/// The device links the profile keeps for `game`.
fn device_links(ctx: &Context, game: &str) -> Vec<DeviceLink> {
    ctx.profile
        .game_bindings(game)
        .map(|g| {
            g.devices
                .iter()
                .map(|d| DeviceLink {
                    output: d.output.clone(),
                    game_device: d.game_device.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Axis slots 1..=8 by the names game export plugins use.
const AXIS_NAMES: [&str; 8] = ["x", "y", "z", "rx", "ry", "rz", "slider", "dial"];

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
            // The device of this computer decides; one only the profile knows goes by what
            // the profile keeps of it.
            let name = match ctx.vjoy.iter().find(|d| d.id == id) {
                Some(d) => d.name.as_deref(),
                None => stored.and_then(|j| j.name.as_deref()),
            };
            let known = gob_virtual::game_identity(id, name, stored.and_then(|j| j.product));
            // The attached input device that is this one, for games keeping instance GUIDs.
            let instance = ctx
                .devices
                .iter()
                .find(|d| {
                    d.info.is_own()
                        && gob_virtual::joystick_of_input(&d.info.hid_path, ctx.vjoy) == Some(id)
                })
                .map_or(String::new(), |d| {
                    format!("{{{}}}", d.info.guid.to_ascii_uppercase())
                });
            // Its controls: as on this computer, else as the profile keeps them.
            let (slots, buttons, hats) = match ctx.vjoy.iter().find(|d| d.id == id) {
                Some(d) => (d.axes.clone(), d.buttons, d.hats()),
                None => stored.map_or((Vec::new(), 0, 0), |j| (j.axes.clone(), j.buttons, j.hats)),
            };
            let mut slots = slots;
            slots.sort_unstable();
            OurDevice {
                output: format!("vjoy:{id}"),
                name: known.name,
                guid: known.product_guid,
                instance,
                axes: slots
                    .iter()
                    .filter_map(|s| AXIS_NAMES.get(usize::from(*s).wrapping_sub(1)))
                    .map(|n| (*n).to_string())
                    .collect(),
                buttons,
                hats,
            }
        })
        .collect()
}

/// An output's name as the page shows it.
fn output_name(i18n: &I18n, o: &GameOutput) -> String {
    let (device, what) = output_parts(i18n, o);
    format!("{device} · {what}")
}

/// An output's device and its name on that device: the schematic names the device once, above
/// its outputs.
fn output_parts(i18n: &I18n, o: &GameOutput) -> (String, String) {
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
            (dev, what)
        }
        VirtualOutput::Gamepad { gamepad, control } => (
            format!("{} {gamepad}", i18n.tr("game-gamepad")),
            control.clone(),
        ),
        VirtualOutput::Logical { input_type, id } => {
            (i18n.tr("game-logical"), format!("{input_type:?} {id}"))
        }
    };
    match o {
        GameOutput::Virtual(v) => base(v),
        GameOutput::HatDirection(v, d) => {
            let (device, what) = base(v);
            (device, format!("{what} {}", arrow(*d)))
        }
        GameOutput::Keys(c) => (i18n.tr("keyboard-device"), combo_name(i18n, c)),
        GameOutput::Pressed(o, p) => {
            let (device, what) = output_parts(i18n, o);
            (device, format!("{what} · {}", press_name(i18n, *p)))
        }
    }
}

/// "long press", "double tap".
pub(super) fn press_name(i18n: &I18n, p: gob_core::Press) -> String {
    i18n.tr(&format!("game-press-{}", p.name()))
}

/// Keys and mouse buttons pressed together, as "Right Alt + Y".
fn combo_name(i18n: &I18n, combo: &gob_core::Combo) -> String {
    combo
        .parts()
        .iter()
        .map(|p| match p {
            gob_core::ComboPart::Key(k) => crate::key_capture::name(*k, i18n),
            gob_core::ComboPart::Mouse(b) => i18n.tr(&format!(
                "mouse-{}",
                b.name().to_lowercase().replace(' ', "-")
            )),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn arrow(d: HatDirection) -> &'static str {
    match d {
        HatDirection::Up => "↑",
        HatDirection::Right => "→",
        HatDirection::Down => "↓",
        HatDirection::Left => "←",
    }
}
