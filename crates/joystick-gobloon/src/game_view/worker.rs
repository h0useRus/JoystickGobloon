//! The page's worker thread: plugins load and run on it, the window never waits.

use super::*;

pub(super) enum Request {
    Scan {
        env: gob_export::Env,
    },
    Read {
        plugin: usize,
        target: Target,
        version: Option<String>,
        ours: Vec<OurDevice>,
        devices: Vec<DeviceLink>,
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
        cleared: Vec<String>,
        profile: String,
        options: BTreeMap<String, OptionValue>,
    },
    Write {
        target: Target,
        files: Files,
        result: WriteResult,
    },
}

pub(super) enum Reply {
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
pub(super) fn plugin_dirs() -> Vec<(PathBuf, Origin)> {
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

pub(super) fn spawn_worker(ctx: egui::Context) -> (Sender<Request>, Receiver<Reply>) {
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
                        devices,
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
                                devices: &devices,
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
                        cleared,
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
                                    cleared: &cleared,
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

pub(super) fn pick(catalogs: &Catalogs, version: Option<&str>) -> Catalog {
    catalogs
        .pick(version)
        .map(|(c, _)| c.clone())
        .unwrap_or_default()
}
