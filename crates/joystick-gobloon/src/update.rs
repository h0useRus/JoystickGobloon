//! Updates from GitHub releases. Only final releases count (GitHub's "latest release" is never a
//! pre-release), and only when newer than this build. An installed copy runs the new installer
//! silently, which starts the app again; a portable copy swaps its files and restarts itself.
//! Downloads are checked against the SHA-256 digest GitHub keeps for every release asset.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use eframe::egui::{self, RichText};
use gob_i18n::I18n;
use semver::Version;
use serde::Deserialize;

use crate::style::{self, icon};

const LATEST: &str = "https://api.github.com/repos/h0useRus/JoystickGobloon/releases/latest";
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

/// Copies installed by the setup have its uninstaller next to the exe.
fn installed() -> bool {
    exe_dir().is_some_and(|d| d.join("unins000.exe").exists())
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

impl Updater {
    /// Asks GitHub for the latest release in the background. `auto`: install it right away.
    pub fn check(&mut self, ctx: &egui::Context, auto: bool) {
        if self.job.is_some() {
            return;
        }
        self.auto = auto;
        self.state = State::Checking;
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Done::Checked(latest()));
            ctx.request_repaint();
        });
        self.job = Some(rx);
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
    pub fn ui(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
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
            self.check(ui.ctx(), false);
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

/// The latest final release, when it is newer than this build and has the asset this copy
/// updates from (installer or portable zip).
fn latest() -> Result<Option<Release>, String> {
    let current = Version::parse(crate::VERSION).map_err(|e| format!("own version: {e}"))?;
    let release: GhRelease = agent()
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = Version::parse(release.tag_name.trim_start_matches('v'))
        .map_err(|e| format!("release tag {}: {e}", release.tag_name))?;
    if !version.pre.is_empty() || version <= current {
        return Ok(None);
    }
    let suffix = if installed() {
        "-setup.exe"
    } else {
        "-portable.zip"
    };
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name.ends_with(suffix))
        .ok_or_else(|| format!("release {version} has no *{suffix}"))?;
    Ok(Some(Release { version, asset }))
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
        // tar.exe (Windows 10 1803+) unpacks zip files.
        let out = dir.join("unpacked");
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let status = hidden(std::process::Command::new("tar"))
            .arg("-xf")
            .arg(&file)
            .arg("-C")
            .arg(&out)
            .status()
            .map_err(|e| format!("tar: {e}"))?;
        if !status.success() {
            return Err(format!("tar: {status}"));
        }
        // The zip holds a JoystickGobloon folder.
        let root = out.join("JoystickGobloon");
        let root = if root.is_dir() { root } else { out };
        if !root.join("JoystickGobloon.exe").exists() {
            return Err("the portable zip has no JoystickGobloon.exe".into());
        }
        Ok(Prepared::Portable(root))
    } else {
        Ok(Prepared::Installer(file))
    }
}

fn hidden(mut cmd: std::process::Command) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
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
            shell_open(
                setup,
                "/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /UPDATE=1",
            )
        }
        Prepared::Portable(new) => {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let dir = exe.parent().ok_or("no exe folder")?.to_path_buf();
            let old = exe.with_extension(format!("exe.{OLD_SUFFIX}"));
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

fn copy_portable(from: &Path, to: &Path) -> Result<(), String> {
    let copy = |name: &Path| {
        std::fs::copy(from.join(name), to.join(name))
            .map(|_| ())
            .map_err(|e| format!("{}: {e}", name.display()))
    };
    copy(Path::new("JoystickGobloon.exe"))?;
    if from.join("LICENSE").exists() {
        copy(Path::new("LICENSE"))?;
    }
    let lang = from.join("lang");
    if lang.is_dir() {
        std::fs::create_dir_all(to.join("lang")).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(&lang)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            copy(&Path::new("lang").join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Removes the exe a portable update left behind.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension(format!("exe.{OLD_SUFFIX}")));
    }
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
    #[cfg(windows)]
    // SAFETY: the handle is closed after the wait.
    unsafe {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
        };
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, 15_000);
            let _ = CloseHandle(h);
        }
    }
    #[cfg(not(windows))]
    let _ = pid;
}

/// ShellExecute "open": the installer's manifest asks for administrator rights (UAC).
fn shell_open(file: &Path, params: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{HSTRING, PCWSTR};
        let (file, params) = (HSTRING::from(file.as_os_str()), HSTRING::from(params));
        // SAFETY: the strings outlive the call.
        let h = unsafe {
            ShellExecuteW(
                None,
                &HSTRING::from("open"),
                &file,
                &params,
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // Values above 32 mean success; 5 = access denied (UAC refused).
        if h.0 as isize > 32 {
            Ok(())
        } else {
            Err(format!(
                "could not start the installer (code {})",
                h.0 as isize
            ))
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (file, params);
        Err("updates need Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Network: `cargo test -p joystick-gobloon -- --ignored`. This build's version is not older than
    /// the latest release, so nothing is offered; the request, TLS and JSON work.
    #[test]
    #[ignore]
    fn asks_github_for_the_latest_release() {
        assert!(latest().unwrap().is_none());
    }

    /// Network: downloads the 0.4.0 portable zip, checks its digest and unpacks it.
    #[test]
    #[ignore]
    fn downloads_and_unpacks_a_release() {
        let release: GhRelease = agent()
            .get("https://api.github.com/repos/h0useRus/JoystickGobloon/releases/tags/v0.4.0")
            .call()
            .unwrap()
            .body_mut()
            .read_json()
            .unwrap();
        let asset = release
            .assets
            .into_iter()
            .find(|a| a.name.ends_with("-portable.zip"))
            .unwrap();
        assert!(asset.digest.is_some(), "GitHub gives the digest");
        let release = Release {
            version: Version::parse("0.4.0").unwrap(),
            asset,
        };
        let Prepared::Portable(dir) = prepare(&release).unwrap() else {
            panic!("expected the portable folder");
        };
        assert!(dir.join("JoystickGobloon.exe").exists());
        assert!(dir.join("lang").join("en.ftl").exists());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn only_newer_final_releases() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(v("0.4.1") > v("0.4.0"));
        // An alpha of the same version is older than its release.
        assert!(v("0.4.1") > v("0.4.1-alpha.12"));
        assert!(v("0.4.0") < v("0.4.1-alpha.12"));
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
