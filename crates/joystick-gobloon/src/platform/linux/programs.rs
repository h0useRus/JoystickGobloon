//! Running programs for switching profiles by program: the window in front and the programs
//! with windows, from the X server's window manager hints (`_NET_ACTIVE_WINDOW`,
//! `_NET_CLIENT_LIST`, `_NET_WM_PID`). On Wayland this sees the X11 windows through XWayland,
//! which is where Wine and Proton games run; a native Wayland window in front reads as none.
//! A Windows program under Wine is named by its .exe, as on Windows.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use crate::platform::Program;

struct X {
    conn: RustConnection,
    root: Window,
    active: Atom,
    clients: Atom,
    pid: Atom,
}

impl X {
    fn connect() -> Option<Self> {
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let atom = |name: &str| -> Option<Atom> {
            Some(
                conn.intern_atom(false, name.as_bytes())
                    .ok()?
                    .reply()
                    .ok()?
                    .atom,
            )
        };
        let (active, clients, pid) = (
            atom("_NET_ACTIVE_WINDOW")?,
            atom("_NET_CLIENT_LIST")?,
            atom("_NET_WM_PID")?,
        );
        Some(Self {
            conn,
            root,
            active,
            clients,
            pid,
        })
    }

    /// 32-bit values of a property; `None` when the connection failed.
    fn values(&self, window: Window, property: Atom, kind: AtomEnum) -> Option<Vec<u32>> {
        let reply = self
            .conn
            .get_property(false, window, property, kind, 0, 4096)
            .ok()?
            .reply()
            .ok()?;
        Some(reply.value32().map(Iterator::collect).unwrap_or_default())
    }
}

/// One connection, kept between calls (asked about once a second); made again after an error.
static CONNECTION: Mutex<Option<X>> = Mutex::new(None);

fn with_x<T>(f: impl Fn(&X) -> Option<T>) -> Option<T> {
    let mut guard = CONNECTION.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = X::connect();
    }
    let result = f(guard.as_ref()?);
    if result.is_none() {
        // The server may have gone (session ended); try a new connection next time.
        *guard = None;
    }
    result
}

fn window_pid(x: &X, window: Window) -> Option<u32> {
    x.values(window, x.pid, AtomEnum::CARDINAL)?
        .first()
        .copied()
        .filter(|&p| p != 0)
}

pub(super) fn foreground_program() -> Option<Program> {
    // An empty answer is fine ("none in front"); only a failed connection is retried.
    let pid = with_x(|x| {
        let window = x.values(x.root, x.active, AtomEnum::WINDOW)?;
        Some(
            window
                .first()
                .copied()
                .filter(|&w| w != 0)
                .and_then(|w| window_pid(x, w)),
        )
    })??;
    process(pid)
}

pub(super) fn running_programs() -> Vec<Program> {
    let pids = with_x(|x| {
        let windows = x.values(x.root, x.clients, AtomEnum::WINDOW)?;
        Some(
            windows
                .into_iter()
                .filter_map(|w| window_pid(x, w))
                .collect::<Vec<_>>(),
        )
    })
    .unwrap_or_default();
    let mut programs: Vec<Program> = Vec::new();
    for pid in pids {
        if !programs.iter().any(|p| p.pid == pid)
            && let Some(program) = process(pid)
        {
            programs.push(program);
        }
    }
    programs
}

/// Process `pid` as a program: a Windows program under Wine by its .exe, else its file.
fn process(pid: u32) -> Option<Program> {
    let dir = Path::new("/proc").join(pid.to_string());
    if let Some(exe) = std::fs::read(dir.join("cmdline"))
        .ok()
        .and_then(|c| wine_exe(&c))
    {
        // The file itself where the Wine prefix can be found, so the Applications page can
        // keep it; else the path as Wine names it.
        let environ = std::fs::read(dir.join("environ")).unwrap_or_default();
        let path = unix_path(&exe, &wine_prefix(&environ)).unwrap_or_else(|| PathBuf::from(&exe));
        let mut program = Program::new(pid, path);
        program.name = windows_file_name(&exe).to_string();
        return Some(program);
    }
    let path = std::fs::read_link(dir.join("exe")).ok()?;
    Some(Program::new(pid, path))
}

/// The Windows program a Wine process runs: its first argument naming an .exe (Wine puts the
/// program's Windows path first, e.g. `C:\Games\Game.exe` or `Z:\home\h0use\Game.exe`).
fn wine_exe(cmdline: &[u8]) -> Option<String> {
    cmdline
        .split(|&b| b == 0)
        .filter(|a| !a.is_empty())
        .map(String::from_utf8_lossy)
        .find(|a| windows_file_name(a).to_lowercase().ends_with(".exe"))
        .map(|a| a.into_owned())
}

/// The Wine prefix of a process from its environment: `WINEPREFIX`, Proton's
/// `STEAM_COMPAT_DATA_PATH`/pfx, else `~/.wine`.
fn wine_prefix(environ: &[u8]) -> Option<PathBuf> {
    let var = |name: &str| {
        environ
            .split(|&b| b == 0)
            .filter_map(|v| std::str::from_utf8(v).ok())
            .find_map(|v| v.strip_prefix(name)?.strip_prefix('=').map(String::from))
            .filter(|v| !v.is_empty())
    };
    var("WINEPREFIX")
        .map(PathBuf::from)
        .or_else(|| var("STEAM_COMPAT_DATA_PATH").map(|d| Path::new(&d).join("pfx")))
        .or_else(|| var("HOME").map(|h| Path::new(&h).join(".wine")))
}

/// A Windows path of a Wine program as a file of this system: drive letters through the
/// prefix's `dosdevices` links (`Z:` is usually `/`). `None` when it cannot be found.
fn unix_path(windows: &str, prefix: &Option<PathBuf>) -> Option<PathBuf> {
    let (drive, rest) = windows.split_once(':')?;
    if drive.len() != 1 || !drive.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut path = prefix
        .as_ref()?
        .join("dosdevices")
        .join(format!("{}:", drive.to_ascii_lowercase()));
    for part in rest.split(['\\', '/']).filter(|p| !p.is_empty()) {
        path.push(part);
    }
    let path = std::fs::canonicalize(path).ok()?;
    path.is_file().then_some(path)
}

/// The last part of a path with `\` or `/` between folders.
fn windows_file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wine_programs_are_named_by_their_exe() {
        let wine = b"C:\\Games\\Flight\\Flight.exe\0-windowed\0";
        assert_eq!(
            wine_exe(wine).as_deref(),
            Some("C:\\Games\\Flight\\Flight.exe")
        );
        // Proton starts some games through a loader first.
        let proton =
            b"/home/h0use/.steam/proton/files/bin/wine64\0Z:\\home\\h0use\\Game\\Game.EXE\0";
        let exe = wine_exe(proton).unwrap();
        assert_eq!(windows_file_name(&exe), "Game.EXE");
        assert_eq!(wine_exe(b"/usr/bin/firefox\0--new-window\0"), None);
        assert_eq!(windows_file_name("/usr/bin/firefox"), "firefox");
    }

    /// By hand in a desktop session: `cargo test -p joystick-gobloon programs -- --ignored
    /// --nocapture` prints the program in front and the programs with windows.
    #[test]
    #[ignore]
    fn prints_the_running_programs() {
        println!("front: {:?}", foreground_program());
        for p in running_programs() {
            println!("{} {} {}", p.pid, p.name, p.path.display());
        }
    }

    #[test]
    fn wine_paths_are_found_in_the_prefix() {
        let root = std::env::temp_dir().join(format!("gobloon-wine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let prefix = root.join("pfx");
        std::fs::create_dir_all(prefix.join("drive_c/Games/Flight")).unwrap();
        std::fs::create_dir_all(prefix.join("dosdevices")).unwrap();
        std::os::unix::fs::symlink("../drive_c", prefix.join("dosdevices/c:")).unwrap();
        let exe = prefix.join("drive_c/Games/Flight/Flight.exe");
        std::fs::write(&exe, "").unwrap();
        let found = unix_path(r"C:\Games\Flight\Flight.exe", &Some(prefix.clone())).unwrap();
        assert_eq!(found, std::fs::canonicalize(&exe).unwrap());
        assert_eq!(
            unix_path(r"C:\Games\Missing.exe", &Some(prefix.clone())),
            None
        );
        assert_eq!(unix_path("Flight.exe", &Some(prefix)), None);
        let env = b"HOME=/home/h0use\0STEAM_COMPAT_DATA_PATH=/s/compatdata/1\0";
        assert_eq!(wine_prefix(env), Some(PathBuf::from("/s/compatdata/1/pfx")));
        let env = b"WINEPREFIX=/w\0STEAM_COMPAT_DATA_PATH=/s\0";
        assert_eq!(wine_prefix(env), Some(PathBuf::from("/w")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn this_process_is_a_program() {
        let me = process(std::process::id()).unwrap();
        assert_eq!(me.path, std::env::current_exe().unwrap());
    }
}
