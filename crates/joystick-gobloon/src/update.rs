//! Updates from GitHub releases. Only final releases count (GitHub's "latest release" is never a
//! pre-release), and only when newer than this build. An installed copy runs the new installer
//! silently, which starts the app again; a portable copy swaps its files and restarts itself.
//! Downloads are checked against the SHA-256 digest GitHub keeps for every release asset.

use crate::settings::UpdateChannel;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use eframe::egui::{self, RichText};
use gob_i18n::I18n;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::style::{self, icon};

const LATEST: &str = "https://api.github.com/repos/h0useRus/JoystickGobloon/releases/latest";
/// The newest releases, alpha builds included (newest first).
const RECENT: &str = "https://api.github.com/repos/h0useRus/JoystickGobloon/releases?per_page=30";
/// The restarted copy waits for the old process (`--wait-pid <pid>`) before the
/// single-instance check, which would otherwise hand over to the old window.
pub const WAIT_PID: &str = "--wait-pid";
/// Renamed running exe of a portable update, removed on the next start.
const OLD_SUFFIX: &str = "old";

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize, Clone)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    /// `sha256:<hex>`; missing on assets uploaded before GitHub kept digests.
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Clone)]
pub struct Release {
    pub version: Version,
    asset: GhAsset,
}

/// A downloaded update, ready to be applied when the app closes.
pub enum Prepared {
    /// The new installer (installed copies).
    Installer(PathBuf),
    /// The unpacked portable folder.
    Portable(PathBuf),
}

enum State {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading(Version),
    Failed(String),
}

enum Done {
    Checked(Result<Option<Release>, String>),
    Downloaded(Result<Prepared, String>),
}

pub struct Updater {
    state: State,
    job: Option<Receiver<Done>>,
    /// Download and apply a found update without asking (the check on start).
    auto: bool,
}

impl Default for Updater {
    fn default() -> Self {
        Self {
            state: State::Idle,
            job: None,
            auto: false,
        }
    }
}

/// Debug builds never update themselves (their exe is the developer's build).
pub fn enabled() -> bool {
    !cfg!(debug_assertions)
}

/// Managed by the system's installer (Windows: the setup's uninstaller is next to the exe).
fn installed() -> bool {
    exe_dir().is_some_and(|d| crate::platform::installed_copy(&d))
}

/// Where a portable update moves the running program file (it can be renamed, not overwritten).
fn old_exe(exe: &Path) -> PathBuf {
    let name = exe.file_name().unwrap_or_default().to_string_lossy();
    exe.with_file_name(format!("{name}.{OLD_SUFFIX}"))
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

impl Updater {
    /// Asks GitHub for the newest release of `channel` in the background. `auto`: install it
    /// right away.
    pub fn check(&mut self, ctx: &egui::Context, auto: bool, which: UpdateChannel) {
        if self.job.is_some() {
            return;
        }
        self.auto = auto;
        self.state = State::Checking;
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Done::Checked(latest(which)));
            ctx.request_repaint();
        });
        self.job = Some(rx);
    }

    /// The channel changed: what the last check found (an update, or none) was for the other
    /// channel, and a check still running for it is dropped. A running download goes on.
    pub fn forget(&mut self) {
        if matches!(self.state, State::Downloading(_)) {
            return;
        }
        self.job = None;
        self.auto = false;
        self.state = State::Idle;
    }

    fn download(&mut self, ctx: &egui::Context) {
        let State::Available(release) = &self.state else {
            return;
        };
        let release = release.clone();
        log::info!("update: downloading {}", release.asset.name);
        self.state = State::Downloading(release.version.clone());
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Done::Downloaded(prepare(&release)));
            ctx.request_repaint();
        });
        self.job = Some(rx);
    }

    /// Collects background work; returns an update to apply (the app closes for it).
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<Prepared> {
        let done = self.job.as_ref()?.try_recv().ok()?;
        self.job = None;
        match done {
            Done::Checked(Ok(Some(release))) => {
                log::info!("update: version {} is available", release.version);
                self.state = State::Available(release);
                if self.auto && enabled() {
                    self.download(ctx);
                }
                None
            }
            Done::Checked(Ok(None)) => {
                self.state = State::UpToDate;
                None
            }
            Done::Downloaded(Ok(prepared)) => {
                self.state = State::Idle;
                Some(prepared)
            }
            Done::Checked(Err(e)) | Done::Downloaded(Err(e)) => {
                log::warn!("update: {e}");
                self.state = State::Failed(e);
                None
            }
        }
    }

    /// The update block of the About page: a status line and a button, each centered by the
    /// caller's top-down layout.
    pub fn ui(&mut self, ui: &mut egui::Ui, i18n: &I18n, channel: UpdateChannel) {
        let busy = self.job.is_some();
        let weak = |t: String| RichText::new(t).weak();
        let status = match &self.state {
            State::Idle => None,
            State::Checking => Some(weak(format!(
                "{}  {}",
                icon::HOURGLASS,
                i18n.tr("update-checking")
            ))),
            State::Downloading(v) => Some(weak(format!(
                "{}  {}",
                icon::HOURGLASS,
                i18n.trf("update-downloading", &[("version", v.to_string().into())])
            ))),
            State::UpToDate => Some(
                RichText::new(format!(
                    "{}  {}",
                    icon::CHECK_CIRCLE,
                    i18n.tr("update-latest")
                ))
                .color(style::accent(ui)),
            ),
            State::Available(r) => Some(
                RichText::new(format!(
                    "{}  {}",
                    icon::GIFT,
                    i18n.trf(
                        "update-available",
                        &[("version", r.version.to_string().into())]
                    )
                ))
                .strong(),
            ),
            State::Failed(_) => Some(
                RichText::new(format!("{}  {}", icon::WARNING, i18n.tr("update-error")))
                    .color(ui.visuals().warn_fg_color),
            ),
        };
        if let Some(text) = status {
            let r = ui.label(text);
            if let State::Failed(e) = &self.state {
                r.on_hover_text(e);
            }
        }
        if busy {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
            return;
        }
        if matches!(self.state, State::Available(_)) {
            let install = ui
                .add_enabled(
                    enabled(),
                    egui::Button::new(format!(
                        "{}  {}",
                        icon::DOWNLOAD_SIMPLE,
                        i18n.tr("update-install")
                    ))
                    .corner_radius(style::RADIUS),
                )
                .on_disabled_hover_text(i18n.tr("update-debug"));
            if install.clicked() {
                self.auto = false;
                self.download(ui.ctx());
            }
        } else if ui
            .add(
                egui::Button::new(format!(
                    "{}  {}",
                    icon::ARROWS_CLOCKWISE,
                    i18n.tr("update-check")
                ))
                .corner_radius(style::RADIUS),
            )
            .clicked()
        {
            self.check(ui.ctx(), false, channel);
        }
    }
}

fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .user_agent(format!("JoystickGobloon/{}", crate::VERSION))
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .new_agent()
}

/// The newest release of `channel` when it is newer than this build and has the asset this
/// copy updates from (installer, portable zip or AppImage).
fn latest(channel: UpdateChannel) -> Result<Option<Release>, String> {
    let Some(suffix) = crate::platform::update_asset(installed()) else {
        return Ok(None);
    };
    let current = Version::parse(crate::VERSION).map_err(|e| format!("own version: {e}"))?;
    let get = |url: &str| {
        agent()
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| e.to_string())
    };
    let releases: Vec<GhRelease> = match channel {
        UpdateChannel::Release => vec![
            get(LATEST)?
                .body_mut()
                .read_json()
                .map_err(|e| e.to_string())?,
        ],
        UpdateChannel::Alpha => get(RECENT)?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())?,
    };
    newest(releases, &current, suffix, channel)
}

/// The newest of `releases` that `channel` takes, newer than `current`, with its asset. Final
/// releases only on the release channel; on the alpha channel alpha builds too (semver puts
/// `1.0.1-alpha.5` after `1.0.0` and before `1.0.1`). A release without the asset (still
/// uploading, or a build that failed) is passed over for the next newest that has it.
fn newest(
    releases: Vec<GhRelease>,
    current: &Version,
    suffix: &str,
    channel: UpdateChannel,
) -> Result<Option<Release>, String> {
    let best = releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.trim_start_matches('v')).ok()?;
            let alpha = r.prerelease || !version.pre.is_empty();
            if (alpha && channel != UpdateChannel::Alpha) || version <= *current {
                return None;
            }
            let asset = r.assets.into_iter().find(|a| a.name.ends_with(suffix));
            if asset.is_none() {
                log::debug!("update: release {version} has no *{suffix} (yet)");
            }
            Some((version, asset?))
        })
        .max_by(|a, b| a.0.cmp(&b.0));
    Ok(best.map(|(version, asset)| Release { version, asset }))
}

/// Downloads (and for portable copies unpacks) the update into a temporary folder.
fn prepare(release: &Release) -> Result<Prepared, String> {
    let dir = std::env::temp_dir().join(format!("JoystickGobloon-update-{}", release.version));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let bytes = agent()
        .get(&release.asset.browser_download_url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(200 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    if let Some(digest) = release
        .asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        let got = sha256_hex(&bytes);
        if !got.eq_ignore_ascii_case(digest) {
            return Err(format!(
                "{}: SHA-256 {got}, expected {digest}",
                release.asset.name
            ));
        }
    }
    let file = dir.join(&release.asset.name);
    std::fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    if release.asset.name.ends_with(".zip") {
        let out = dir.join("unpacked");
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        unpack_zip(&file, &out)?;
        // The zip holds a JoystickGobloon folder.
        let root = out.join("JoystickGobloon");
        let root = if root.is_dir() { root } else { out };
        check_portable(&root)?;
        Ok(Prepared::Portable(root))
    } else {
        Ok(Prepared::Installer(file))
    }
}

/// An unpacked portable version is complete: it has the exe, lists it and has every file it lists.
fn check_portable(root: &Path) -> Result<(), String> {
    let exe = crate::platform::exe_name();
    if !root.join(exe).exists() {
        return Err(format!("the portable zip has no {exe}"));
    }
    let files = new_files(root)?;
    if !files.iter().any(|n| same_name(n, Path::new(exe))) {
        return Err(format!("the portable zip's {MANIFEST} has no {exe}"));
    }
    if let Some(missing) = files.iter().find(|n| !root.join(n).is_file()) {
        return Err(format!("the portable zip has no {}", missing.display()));
    }
    Ok(())
}

/// Unpacks a zip into `to` (the same way speech models are unpacked), refusing entries that
/// would land outside it.
fn unpack_zip(zip: &Path, to: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip: {e}"))?;
        let Some(name) = entry.enclosed_name() else {
            return Err(format!(
                "zip: unsafe path {}",
                String::from_utf8_lossy(entry.name_raw())
            ));
        };
        let path = to.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out =
            std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Starts the update; the app closes right after. Installed: the new installer runs silently
/// (UAC prompt) and starts the app again. Portable: the files are swapped and the new exe starts.
pub fn apply(prepared: &Prepared) -> Result<(), String> {
    match prepared {
        Prepared::Installer(setup) => {
            log::info!("update: running {}", setup.display());
            crate::platform::run_installer(setup)
        }
        Prepared::Portable(new) => {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let dir = exe.parent().ok_or("no exe folder")?.to_path_buf();
            let old = old_exe(&exe);
            let _ = std::fs::remove_file(&old);
            // A running exe can be renamed, not overwritten.
            std::fs::rename(&exe, &old).map_err(|e| format!("rename {}: {e}", exe.display()))?;
            let copied = copy_portable(new, &dir);
            if let Err(e) = copied {
                let _ = std::fs::rename(&old, &exe);
                return Err(e);
            }
            log::info!("update: portable files replaced, restarting");
            std::process::Command::new(&exe)
                .arg(WAIT_PID)
                .arg(std::process::id().to_string())
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }
}

/// The list of a portable version's files (paths relative to the exe's folder, `/` between
/// folders), next to the exe: what an update replaces, and what it removes when the new
/// version no longer has it. Files the user added (their own languages) are never in it.
/// release.yml writes it into the portable zip.
const MANIFEST: &str = "files.gob";

#[derive(Deserialize, Serialize)]
struct Manifest {
    files: Vec<String>,
}

/// What the last portable update renamed aside (paths of the `.old` files), so [`cleanup`]
/// finds them in any folder, also in folders the new version no longer has.
const ASIDE: &str = "update-aside.gob";

/// The paths a list file (`files = [...]`) names; `None` when there is no such file.
fn read_list(file: &Path) -> Result<Option<Vec<PathBuf>>, String> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Ok(None);
    };
    let what = file.file_name().unwrap_or_default().to_string_lossy();
    let list: Manifest = toml::from_str(&text).map_err(|e| format!("{what}: {e}"))?;
    let mut names: Vec<PathBuf> = Vec::new();
    for name in &list.files {
        // Only plain relative paths below the folder, the same on every system: nothing outside
        // it is ever touched (no "..", roots, drives or `\`).
        if !name.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.contains([':', '\\'])
        }) {
            return Err(format!("{what}: bad path {name:?}"));
        }
        let path = PathBuf::from(name);
        if !names.iter().any(|n| same_name(n, &path)) {
            names.push(path);
        }
    }
    Ok(Some(names))
}

/// The files `dir`'s manifest lists (the manifest itself included); `None` without a manifest.
fn manifest(dir: &Path) -> Result<Option<Vec<PathBuf>>, String> {
    let Some(mut names) = read_list(&dir.join(MANIFEST))? else {
        return Ok(None);
    };
    let suffix = format!(".{OLD_SUFFIX}");
    // Set-aside names belong to the update itself.
    if let Some(name) = names
        .iter()
        .find(|n| n.to_string_lossy().ends_with(&suffix))
    {
        return Err(format!("{MANIFEST}: bad path {:?}", name.display()));
    }
    if !names.iter().any(|n| same_name(n, Path::new(MANIFEST))) {
        names.push(PathBuf::from(MANIFEST));
    }
    Ok(Some(names))
}

/// File names compare without case, as Windows does.
fn same_name(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// The files of the new version: its manifest, or (zips made before it) every file next to the
/// exe and in `lang`.
fn new_files(from: &Path) -> Result<Vec<PathBuf>, String> {
    if let Some(names) = manifest(from)? {
        return Ok(names);
    }
    let mut names = Vec::new();
    for folder in [Path::new(""), Path::new("lang")] {
        let path = from.join(folder);
        if folder != Path::new("") && !path.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .flatten()
        {
            if entry.path().is_file() {
                names.push(folder.join(entry.file_name()));
            }
        }
    }
    Ok(names)
}

/// Copies the new version's files (the exe, libraries such as the speech library, languages)
/// over the old ones and removes the old version's files the new one no longer lists. Each old
/// file is renamed aside first (a loaded library can be renamed, not overwritten) and removed by
/// [`cleanup`] on the next start. All or nothing: when a file fails, every file and folder is put
/// back as it was.
fn copy_portable(from: &Path, to: &Path) -> Result<(), String> {
    copy_portable_with(from, to, |a, b| std::fs::copy(a, b).map(|_| ()))
}

fn copy_portable_with(
    from: &Path,
    to: &Path,
    copy: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let names = new_files(from)?;
    let exe = Path::new(crate::platform::exe_name());
    // A broken old manifest only means nothing is removed. The running exe is apply's to move.
    let gone: Vec<PathBuf> = manifest(to)
        .ok()
        .flatten()
        .unwrap_or_default()
        .into_iter()
        .filter(|n| {
            !same_name(n, exe) && !names.iter().any(|m| same_name(m, n)) && to.join(n).is_file()
        })
        .collect();
    let mut undo = Undo::default();
    let mut failed = None;
    for (name, replace) in names
        .iter()
        .map(|n| (n, true))
        .chain(gone.iter().map(|n| (n, false)))
    {
        let target = to.join(name);
        let step = undo.step(to, &target).and_then(|()| {
            if replace {
                copy(&from.join(name), &target)
            } else {
                Ok(())
            }
        });
        if let Err(e) = step {
            failed = Some(format!("{}: {e}", name.display()));
            break;
        }
    }
    if let Some(e) = failed {
        undo.run();
        return Err(e);
    }
    let aside: Vec<String> = undo
        .files
        .iter()
        .filter_map(|(_, a)| a.as_ref()?.strip_prefix(to).ok())
        .map(|a| a.to_string_lossy().replace('\\', "/"))
        .collect();
    // Without the list only leftovers outside the exe's folder and `lang` stay.
    let list = toml::to_string(&Manifest { files: aside }).unwrap_or_default();
    let _ = std::fs::write(to.join(ASIDE), list);
    Ok(())
}

/// What a portable update changed so far, to put it back when a file fails.
#[derive(Default)]
struct Undo {
    /// Files written or set aside, with where their old version went.
    files: Vec<(PathBuf, Option<PathBuf>)>,
    /// Folders made for new files, outer ones first.
    folders: Vec<PathBuf>,
}

impl Undo {
    /// Makes `target`'s missing folders below `to` and renames an existing `target` to its
    /// `.old` name, remembering both.
    fn step(&mut self, to: &Path, target: &Path) -> std::io::Result<()> {
        if let Some(parent) = target.parent() {
            let mut missing: Vec<&Path> = parent
                .ancestors()
                .take_while(|a| *a != to && !a.exists())
                .collect();
            missing.reverse();
            for folder in missing {
                std::fs::create_dir(folder)?;
                self.folders.push(folder.to_path_buf());
            }
        }
        let aside = if target.exists() {
            let aside = old_exe(target);
            let _ = std::fs::remove_file(&aside);
            std::fs::rename(target, &aside)?;
            Some(aside)
        } else {
            None
        };
        self.files.push((target.to_path_buf(), aside));
        Ok(())
    }

    fn run(self) {
        for (target, aside) in self.files.into_iter().rev() {
            let _ = std::fs::remove_file(&target);
            if let Some(aside) = aside {
                let _ = std::fs::rename(aside, &target);
            }
        }
        for folder in self.folders.into_iter().rev() {
            let _ = std::fs::remove_dir(folder);
        }
    }
}

/// Removes the files a portable update renamed aside, and the folders that leaves empty.
pub fn cleanup() {
    if let Some(dir) = exe_dir() {
        cleanup_in(&dir);
    }
}

fn cleanup_in(dir: &Path) {
    let suffix = format!(".{OLD_SUFFIX}");
    // Updates before the aside list: the exe's folder and `lang`.
    for folder in [dir.to_path_buf(), dir.join("lang")] {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            if entry.file_name().to_string_lossy().ends_with(&suffix) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let list = dir.join(ASIDE);
    for name in read_list(&list).ok().flatten().unwrap_or_default() {
        if !name.to_string_lossy().ends_with(&suffix) {
            continue;
        }
        let file = dir.join(&name);
        let _ = std::fs::remove_file(&file);
        // Only folders left empty go, never the exe's folder itself.
        for folder in file.ancestors().skip(1) {
            if folder == dir || !folder.starts_with(dir) || std::fs::remove_dir(folder).is_err() {
                break;
            }
        }
    }
    let _ = std::fs::remove_file(list);
}

/// `--wait-pid <pid>`: waits (at most 15 s) for the process this one replaces to exit.
pub fn wait_for_previous() {
    let args: Vec<String> = std::env::args().collect();
    let Some(pid) = args
        .iter()
        .position(|a| a == WAIT_PID)
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u32>().ok())
    else {
        return;
    };
    crate::platform::wait_for_process(pid, std::time::Duration::from_secs(15));
}

#[cfg(test)]
mod tests;
