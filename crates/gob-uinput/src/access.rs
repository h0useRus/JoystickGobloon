//! Device access for the logged-in user through a udev rule: `/dev/uinput` (virtual joysticks,
//! keyboard and mouse output) and the evdev nodes of game controllers. Most systems give both
//! to root only (or the `input` group). The rule tags them `uaccess`, so systemd-logind gives
//! the user at the seat access; it is installed once with administrator rights (`pkexec`), like
//! vJoy's configuration on Windows.
//!
//! Keyboards are not in that rule: reading every keyboard is what keyboard bindings need, but
//! any program of the user could then read what is typed. Users who want keyboard bindings opt
//! in with a rule of its own ([`KEYBOARD_RULE`], [`install_keyboard`]) or the `input` group.

use std::io;
use std::path::Path;
use std::process::Command;

/// Below 73: `uaccess` is applied by systemd's 73-seat-late.rules.
pub const RULE_PATH: &str = "/etc/udev/rules.d/70-joystick-gobloon.rules";

/// The rule file's text.
pub const RULE: &str = "\
# Joystick Gobloon: the user at the seat may use these devices (installed by Joystick Gobloon).
# Virtual joysticks, keyboard and mouse output.
KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\", OPTIONS+=\"static_node=uinput\"
# Game controllers (also our own virtual ones), for input and hiding.
SUBSYSTEM==\"input\", KERNEL==\"event*\", ENV{ID_INPUT_JOYSTICK}==\"1\", TAG+=\"uaccess\"
";

/// Opt-in rule for keyboard bindings, in a file of its own so it can be removed alone.
pub const KEYBOARD_RULE_PATH: &str = "/etc/udev/rules.d/70-joystick-gobloon-keyboard.rules";

/// The keyboard rule's text.
pub const KEYBOARD_RULE: &str = "\
# Joystick Gobloon: the user at the seat may read keyboards, for keyboard bindings (allowed in
# Joystick Gobloon; any program of the user can then read what is typed).
SUBSYSTEM==\"input\", KERNEL==\"event*\", ENV{ID_INPUT_KEYBOARD}==\"1\", TAG+=\"uaccess\"
";

/// Our own virtual devices start with this name; they are not the user's keyboards.
const OWN_PREFIX: &str = "Joystick Gobloon";

/// Writes the rule (`$1`), then has udev apply it to the devices that exist already.
const INSTALL_SCRIPT: &str = "printf '%s' \"$1\" > \"$2\" \
    && udevadm control --reload-rules \
    && udevadm trigger --action=change --subsystem-match=misc --sysname-match=uinput \
    && udevadm trigger --action=change --subsystem-match=input";

/// Deletes the rule file `$1` and has udev apply the change. Access the user already got stays
/// until the device is connected again (udev does not take `uaccess` back on a change event).
const REMOVE_SCRIPT: &str = "rm -f \"$1\"     && udevadm control --reload-rules     && udevadm trigger --action=change --subsystem-match=input";

/// Whether the user lacks access the rule would give: `/dev/uinput` cannot be opened for
/// writing, or a game controller's node cannot be read. False with the rule installed (a
/// session without seat, e.g. over SSH, does not get `uaccess`; nothing to fix then).
pub fn missing() -> bool {
    if Path::new(RULE_PATH).exists() {
        return false;
    }
    let uinput = crate::available().is_err();
    uinput || controllers_unreadable()
}

/// Some game controller's event node exists but is not readable.
fn controllers_unreadable() -> bool {
    let Ok(dir) = std::fs::read_dir("/sys/class/input") else {
        return false;
    };
    dir.flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("event"))
        })
        .filter(|e| is_joystick(&e.path()))
        .any(|e| {
            let node = Path::new("/dev/input").join(e.file_name());
            std::fs::File::open(node).is_err_and(|e| e.kind() == io::ErrorKind::PermissionDenied)
        })
}

/// udev marked the node as a joystick: `E:ID_INPUT_JOYSTICK=1` in its database entry
/// (`/run/udev/data/c<major>:<minor>`).
fn is_joystick(sys_event: &Path) -> bool {
    udev_flag(sys_event, "E:ID_INPUT_JOYSTICK=1")
}

fn udev_flag(sys_event: &Path, line: &str) -> bool {
    let dev = std::fs::read_to_string(sys_event.join("dev")).unwrap_or_default();
    let entry = std::fs::read_to_string(format!("/run/udev/data/c{}", dev.trim()));
    entry.is_ok_and(|e| e.lines().any(|l| l.trim() == line))
}

/// The keyboard rule is installed.
pub fn keyboard_rule_installed() -> bool {
    Path::new(KEYBOARD_RULE_PATH).exists()
}

/// Some keyboard of the user (not one of our virtual devices) can be read.
pub fn keyboards_readable() -> bool {
    let Ok(dir) = std::fs::read_dir("/sys/class/input") else {
        return false;
    };
    dir.flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("event"))
        })
        .filter(|e| udev_flag(&e.path(), "E:ID_INPUT_KEYBOARD=1"))
        .filter(|e| {
            let name = std::fs::read_to_string(e.path().join("device/name")).unwrap_or_default();
            !name.starts_with(OWN_PREFIX)
        })
        .any(|e| std::fs::File::open(Path::new("/dev/input").join(e.file_name())).is_ok())
}

/// Installs the rule with administrator rights: the system asks for the password (polkit).
/// Blocking until the user answered: call it off the GUI thread.
pub fn install() -> Result<(), String> {
    pkexec(&[INSTALL_SCRIPT, "sh", RULE, RULE_PATH])
}

/// Installs the keyboard rule like [`install`].
pub fn install_keyboard() -> Result<(), String> {
    pkexec(&[INSTALL_SCRIPT, "sh", KEYBOARD_RULE, KEYBOARD_RULE_PATH])
}

/// Removes the keyboard rule like [`install`]; access already given ends when the keyboard is
/// connected again or the computer restarts.
pub fn remove_keyboard() -> Result<(), String> {
    pkexec(&[REMOVE_SCRIPT, "sh", KEYBOARD_RULE_PATH])
}

/// Runs `/bin/sh -c <args>` as root through pkexec.
fn pkexec(args: &[&str]) -> Result<(), String> {
    let output = Command::new("pkexec")
        .args(["/bin/sh", "-c"])
        .args(args)
        .output()
        .map_err(|e| format!("pkexec: {e}"))?;
    match output.status.code() {
        Some(0) => Ok(()),
        // pkexec: the dialog was dismissed (126) or the password was not accepted (127).
        Some(126 | 127) => Err("not authorized".into()),
        _ => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_tags_uinput_and_controllers() {
        assert!(RULE_PATH.ends_with(".rules"));
        let rules: Vec<&str> = RULE.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().all(|r| r.contains("TAG+=\"uaccess\"")));
        assert!(
            !RULE.contains("ID_INPUT_KEYBOARD"),
            "keyboards stay private"
        );
    }

    #[test]
    fn keyboard_rule_is_separate() {
        assert_ne!(KEYBOARD_RULE_PATH, RULE_PATH);
        let rules: Vec<&str> = KEYBOARD_RULE
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].contains("ENV{ID_INPUT_KEYBOARD}==\"1\""));
        assert!(rules[0].contains("TAG+=\"uaccess\""));
    }

    /// The remove script deletes the file and calls udevadm; stub udevadm as below.
    #[test]
    fn remove_script_deletes_the_rule() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("gob-uinput-remove-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("udevadm");
        std::fs::write(
            &stub,
            "#!/bin/sh
",
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let rule = dir.join("70-joystick-gobloon-keyboard.rules");
        std::fs::write(&rule, KEYBOARD_RULE).unwrap();
        let path = format!(
            "{}:{}",
            dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let status = Command::new("/bin/sh")
            .args(["-c", REMOVE_SCRIPT, "sh"])
            .arg(&rule)
            .env("PATH", path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(!rule.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The script pkexec runs writes the rule unchanged (quotes and all) and calls udevadm;
    /// here as the user, into a temporary file, with a stub udevadm.
    #[test]
    fn install_script_writes_the_rule() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("gob-uinput-access-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("udevadm.log");
        let stub = dir.join("udevadm");
        let script = format!(
            "#!/bin/sh
echo \"$@\" >> '{}'
",
            log.display()
        );
        std::fs::write(&stub, script).unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let rule = dir.join("70-joystick-gobloon.rules");
        let path = format!(
            "{}:{}",
            dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let status = Command::new("/bin/sh")
            .args(["-c", INSTALL_SCRIPT, "sh", RULE])
            .arg(&rule)
            .env("PATH", path)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&rule).unwrap(), RULE);
        let calls = std::fs::read_to_string(&log).unwrap();
        assert_eq!(calls.lines().count(), 3, "{calls}");
        assert!(calls.starts_with("control --reload-rules"), "{calls}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
