//! Start at logon: a desktop entry in `$XDG_CONFIG_HOME/autostart` (the XDG autostart spec;
//! GNOME, KDE, Xfce and the Steam Deck's desktop mode read it).

use std::path::{Path, PathBuf};

const FILE: &str = "joystick-gobloon.desktop";

fn autostart_dir() -> Option<PathBuf> {
    Some(super::config_home()?.join("autostart"))
}

/// The program to start: the AppImage file itself when running from one (the running exe is
/// inside its temporary mount), otherwise the exe.
fn program() -> Result<PathBuf, String> {
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|a| !a.is_empty()) {
        return Ok(PathBuf::from(appimage));
    }
    std::env::current_exe().map_err(|e| e.to_string())
}

/// One argument of an `Exec` line, quoted as the desktop entry spec wants: `"`, `` ` ``, `$`
/// and `\` get a backslash inside the quotes, then backslashes are doubled once more (string
/// escaping comes first) and `%` becomes `%%` (field codes).
fn exec_arg(arg: &str) -> String {
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(c);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

fn entry(program: &Path, arg: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={}\n\
         Exec={} {arg}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        crate::APP_NAME,
        exec_arg(&program.to_string_lossy()),
    )
}

pub(super) fn set(enabled: bool, arg: &str) -> Result<(), String> {
    let dir = autostart_dir().ok_or("no home folder")?;
    let path = dir.join(FILE);
    if !enabled {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(&path, entry(&program()?, arg)).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_line_quoting() {
        assert_eq!(
            exec_arg("/opt/Joystick Gobloon/x"),
            "\"/opt/Joystick Gobloon/x\""
        );
        assert_eq!(exec_arg("a$b"), "\"a\\\\$b\"");
        assert_eq!(exec_arg("a\\b"), "\"a\\\\\\\\b\"");
        assert_eq!(exec_arg("100%"), "\"100%%\"");
        let text = entry(
            Path::new("/home/u/Joystick Gobloon.AppImage"),
            "--autostart",
        );
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(
            text.contains("Exec=\"/home/u/Joystick Gobloon.AppImage\" --autostart\n"),
            "{text}"
        );
    }
}
