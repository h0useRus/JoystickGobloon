//! The program in the desktop's application menu: started from an AppImage, it writes its own
//! desktop entry and icon to `$XDG_DATA_HOME` (like AppImage integration tools do). The entry's
//! name matches the window's app id, so the dock shows the program's icon instead of a generic
//! one, and the program can be started from the menu. Rewritten when the AppImage moves.

use std::path::{Path, PathBuf};

use crate::APP_ID;

const ICON: &[u8] = include_bytes!("../../../../../assets/icon/JoystickGobloon-256.png");

/// `$XDG_DATA_HOME`, by default `~/.local/share`.
fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
}

fn entry(appimage: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={}\n\
         Comment=Device hiding, virtual joysticks and input mapping\n\
         Exec={}\n\
         Icon={APP_ID}\n\
         StartupWMClass={APP_ID}\n\
         Categories=Game;\n\
         Terminal=false\n",
        crate::APP_NAME,
        super::autostart::exec_arg(&appimage.to_string_lossy()),
    )
}

/// Writes `bytes` to `path` unless it holds them already.
fn write_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|b| b == bytes) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes)
}

/// Adds the program to the menu when it runs from an AppImage; logged on failure.
pub(super) fn integrate() {
    let Some(appimage) = std::env::var_os("APPIMAGE").filter(|a| !a.is_empty()) else {
        return;
    };
    let Some(data) = data_home() else { return };
    let icon = data.join(format!("icons/hicolor/256x256/apps/{APP_ID}.png"));
    let desktop = data.join(format!("applications/{APP_ID}.desktop"));
    let result = write_changed(&icon, ICON)
        .and_then(|()| write_changed(&desktop, entry(Path::new(&appimage)).as_bytes()));
    if let Err(e) = result {
        log::warn!("adding to the application menu: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_starts_the_appimage() {
        let text = entry(Path::new("/home/u/Downloads/Joystick Gobloon.AppImage"));
        assert!(text.contains("Exec=\"/home/u/Downloads/Joystick Gobloon.AppImage\"\n"));
        assert!(text.contains(&format!("StartupWMClass={APP_ID}\n")));
        assert!(text.contains(&format!("Icon={APP_ID}\n")));
    }
}
