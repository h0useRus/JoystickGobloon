//! Device plugins downloaded on demand: the program ships without them; the list of plugins
//! (`plugins/devices/index.gob`) is read from the project's repository the first time a
//! connected device has no plugin, and a plugin fitting it is downloaded on request into the
//! user's plugin folder, every file checked against the list's size and SHA-256. Final releases
//! read the `main` branch, the Alpha update channel `dev`. One state for the whole program: the
//! Input viewer and the Game page share it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use eframe::egui;
use gob_devices::Index;
use gob_devices::index::{Entry, FILE_MAX, INDEX};
use gob_devices::plugin::MANIFEST;
use gob_input::DeviceInfo;

const RAW: &str = "https://raw.githubusercontent.com/h0useRus/JoystickGobloon";
/// How long after a failed read of the list it is tried again.
const RETRY: Duration = Duration::from_secs(10 * 60);

/// What a device without a plugin can get.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// No plugin to download (yet: the list may still be read).
    None,
    /// The plugin `id` named `name` can be downloaded.
    Available {
        id: String,
        name: String,
    },
    Downloading,
}

enum List {
    NotRead,
    Reading,
    Read(Arc<Index>),
    Failed(Instant),
}

struct State {
    list: List,
    /// Plugins being downloaded.
    busy: Vec<String>,
    errors: Vec<String>,
}

static STATE: Mutex<State> = Mutex::new(State {
    list: List::NotRead,
    busy: Vec::new(),
    errors: Vec::new(),
});
/// Counts finished downloads: a plugin store loads its plugins again when it changes.
static DOWNLOADED: AtomicU64 = AtomicU64::new(0);
static ALPHA: AtomicBool = AtomicBool::new(false);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Which branch the list and plugins come from: `dev` for the Alpha update channel.
pub fn set_alpha(alpha: bool) {
    if ALPHA.swap(alpha, Ordering::Relaxed) != alpha {
        state().list = List::NotRead;
    }
}

fn branch() -> &'static str {
    if ALPHA.load(Ordering::Relaxed) {
        "dev"
    } else {
        "main"
    }
}

pub fn downloaded() -> u64 {
    DOWNLOADED.load(Ordering::Relaxed)
}

/// Errors of downloads since the last call (English, for the log and a dialog).
pub fn take_errors() -> Vec<String> {
    std::mem::take(&mut state().errors)
}

/// What a connected device without a local plugin can get; reads the list the first time.
pub fn offer(ctx: &egui::Context, info: &DeviceInfo) -> Offer {
    let mut s = state();
    let index = match &s.list {
        List::Read(index) => index.clone(),
        List::Reading => return Offer::None,
        List::Failed(at) if at.elapsed() < RETRY => return Offer::None,
        List::NotRead | List::Failed(_) => {
            s.list = List::Reading;
            let ctx = ctx.clone();
            let url = format!("{RAW}/{}/plugins/devices/{INDEX}", branch());
            std::thread::spawn(move || {
                let read = fetch(&url, FILE_MAX)
                    .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                    .and_then(|t| Index::parse(&t));
                let mut s = state();
                s.list = match read {
                    Ok(index) => {
                        log::debug!("device plugin list: {} plugins", index.devices.len());
                        List::Read(Arc::new(index))
                    }
                    Err(e) => {
                        log::warn!("device plugin list {url}: {e}");
                        List::Failed(Instant::now())
                    }
                };
                ctx.request_repaint();
            });
            return Offer::None;
        }
    };
    let Some(entry) = index.find(info.vendor_id, info.product_id, &info.name) else {
        return Offer::None;
    };
    if s.busy.contains(&entry.id) {
        return Offer::Downloading;
    }
    Offer::Available {
        id: entry.id.clone(),
        name: entry.name.clone(),
    }
}

/// Downloads plugin `id` into `into` (the user's plugin folder) on a thread.
pub fn start(ctx: &egui::Context, id: &str, into: PathBuf) {
    let mut s = state();
    let List::Read(index) = &s.list else {
        return;
    };
    let Some(entry) = index.devices.iter().find(|e| e.id == id).cloned() else {
        return;
    };
    if s.busy.contains(&entry.id) {
        return;
    }
    s.busy.push(entry.id.clone());
    let ctx = ctx.clone();
    let base = format!("{RAW}/{}/plugins/devices", branch());
    std::thread::spawn(move || {
        let result = install(&base, &entry, &into);
        let mut s = state();
        s.busy.retain(|b| *b != entry.id);
        match result {
            Ok(dir) => {
                log::info!(
                    "device plugin {} downloaded into {}",
                    entry.id,
                    dir.display()
                );
                DOWNLOADED.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                log::error!("device plugin {}: {e}", entry.id);
                s.errors.push(e);
            }
        }
        ctx.request_repaint();
    });
}

fn fetch(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    crate::update::agent()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

/// Every file into a temporary folder, checked, then the folder into place. A folder there
/// already is replaced only when it holds the same plugin (an older download); else the plugin
/// goes into a free folder beside it.
fn install(base: &str, entry: &Entry, into: &Path) -> Result<PathBuf, String> {
    let temp = into.join(format!(".download-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let result = (|| {
        for file in &entry.files {
            if file.size > FILE_MAX {
                return Err(format!("{}: too large", file.path));
            }
            let bytes = fetch(&format!("{base}/{}/{}", entry.path, file.path), FILE_MAX)?;
            if !file.matches(&bytes) {
                return Err(format!("{}: not the file the list names", file.path));
            }
            let path = temp.join(&file.path);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        gob_devices::DevicePlugin::load(&temp)?;
        let mut target = into.join(&entry.path);
        let mut n = 1;
        while target.exists() {
            if holds(&target, &entry.id) {
                std::fs::remove_dir_all(&target)
                    .map_err(|e| format!("{}: {e}", target.display()))?;
                break;
            }
            n += 1;
            target = into.join(format!("{}-{n}", entry.path));
        }
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::rename(&temp, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        Ok(target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&temp);
    }
    result
}

/// Whether `dir` holds the plugin `id`.
fn holds(dir: &Path, id: &str) -> bool {
    std::fs::read_to_string(dir.join(MANIFEST))
        .ok()
        .and_then(|t| toml::from_str::<gob_devices::Manifest>(&t).ok())
        .is_some_and(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// Serves `root`'s files over HTTP on a local port; `tamper` changes every byte served.
    fn serve(root: PathBuf, tamper: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                        break;
                    }
                }
                let path = line
                    .split(' ')
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/');
                let mut stream = reader.into_inner();
                match std::fs::read(root.join(path)) {
                    Ok(mut body) => {
                        if tamper {
                            body.iter_mut().for_each(|b| *b ^= 1);
                        }
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(&body);
                    }
                    Err(_) => {
                        let _ = stream.write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                }
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "JoystickGobloon-download-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(id: &str) -> Entry {
        let text = std::fs::read_to_string(repo().join(INDEX)).unwrap();
        let index = Index::parse(&text).unwrap();
        index.devices.into_iter().find(|e| e.id == id).unwrap()
    }

    #[test]
    fn downloads_a_plugin_and_keeps_another_in_its_folder() {
        let base = serve(repo(), false);
        let into = scratch("ok");
        let e = entry("vkb-t-rudder-mk5");
        // A different plugin of the user's in the same folder stays.
        let theirs = into.join(&e.path);
        std::fs::create_dir_all(&theirs).unwrap();
        std::fs::write(
            theirs.join(MANIFEST),
            "id = \"mine\"\nname = \"Mine\"\nbrand = \"B\"\n",
        )
        .unwrap();
        let dir = install(&base, &e, &into).unwrap();
        assert_eq!(dir, into.join(format!("{}-2", e.path)));
        assert!(holds(&theirs, "mine"));
        assert_eq!(gob_devices::DevicePlugin::load(&dir).unwrap().id(), e.id);
        // Downloaded again: the same plugin's folder is replaced.
        assert_eq!(install(&base, &e, &into).unwrap(), dir);
        let _ = std::fs::remove_dir_all(&into);
    }

    #[test]
    fn a_changed_file_is_not_installed() {
        let base = serve(repo(), true);
        let into = scratch("tampered");
        let e = entry("vkb-t-rudder-mk5");
        assert!(install(&base, &e, &into).is_err());
        let left: Vec<_> = std::fs::read_dir(&into).unwrap().collect();
        assert!(left.is_empty(), "{left:?}");
        let _ = std::fs::remove_dir_all(&into);
    }
}
