//! Device access for the logged-in user through a udev rule: `/dev/uinput` (virtual joysticks,
//! keyboard and mouse output) and the evdev nodes of game controllers. Most systems give both
//! to root only (or the `input` group). The rule tags them `uaccess`, so systemd-logind gives
//! the user at the seat access; it is installed once with administrator rights (`pkexec`), like
//! vJoy's configuration on Windows.
//!
//! Keyboards are not in that rule: reading every keyboard is what keyboard bindings need, but
//! any program of the user could then read what is typed. Users who want keyboard bindings opt
//! in with a rule of its own ([`KEYBOARD_RULE`], [`install_keyboard`]) or the `input` group.
//! Sticks with more than 80 buttons get read access to their raw HID node one by one
//! ([`install_raw`]). Hidden sticks get their raw HID node closed to the user again
//! ([`set_hidden_sticks`]).

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

/// Rules giving the user at the seat read access to the raw HID node (hidraw) of single sticks,
/// one line per stick: the kernel passes at most 80 buttons on to evdev, the rest are read
/// from there. Per stick, never every hidraw node: keyboards have one too.
pub const RAW_RULE_PATH: &str = "/etc/udev/rules.d/70-joystick-gobloon-hidraw.rules";

const RAW_RULE_HEADER: &str = "\
# Joystick Gobloon: the user at the seat may read the raw HID device of these sticks, for their
# buttons past 80 (allowed per stick in Joystick Gobloon).
";

/// Rules closing the raw HID node (hidraw) of hidden sticks to the user, one line per stick:
/// hiding through evdev does not keep programs that read hidraw (Steam, SDL's HIDAPI, Wine) away.
/// After the rules giving access (`70-`, also [`RAW_RULE_PATH`]: hiding wins), before systemd
/// applies `uaccess` (73-seat-late.rules).
pub const HIDDEN_RULE_PATH: &str = "/etc/udev/rules.d/72-joystick-gobloon-hidden.rules";

const HIDDEN_RULE_HEADER: &str = "\
# Joystick Gobloon: only root may open the raw HID device of these sticks while they are hidden
# (hidden in Joystick Gobloon).
";

/// Writes the rule (`$1`), then has udev apply it to the devices that exist already.
const INSTALL_SCRIPT: &str = "printf '%s' \"$1\" > \"$2\" \
    && udevadm control --reload-rules \
    && udevadm trigger --action=change --subsystem-match=misc --sysname-match=uinput \
    && udevadm trigger --action=change --subsystem-match=input \
    && udevadm trigger --action=change --subsystem-match=hidraw";

/// Deletes the rule file `$1` and has udev apply the change. Access the user already got stays
/// until the device is connected again (udev does not take `uaccess` back on a change event).
const REMOVE_SCRIPT: &str = "rm -f \"$1\" \
    && udevadm control --reload-rules \
    && udevadm trigger --action=change --subsystem-match=input \
    && udevadm trigger --action=change --subsystem-match=hidraw";

/// The hidraw rule line of the USB stick with these ids.
pub fn raw_rule(vendor: u16, product: u16) -> String {
    format!(
        "SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{vendor:04x}\", \
         ATTRS{{idProduct}}==\"{product:04x}\", TAG+=\"uaccess\"\n"
    )
}

/// The hidden-stick rule line of the USB stick with these ids: no `uaccess`, root only (the mode
/// also masks access lists given before).
pub fn hidden_rule(vendor: u16, product: u16) -> String {
    format!(
        "SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{vendor:04x}\", \
         ATTRS{{idProduct}}==\"{product:04x}\", TAG-=\"uaccess\", OWNER=\"root\", \
         GROUP=\"root\", MODE=\"0600\"\n"
    )
}

/// The hidden-stick rule file for these sticks; empty for none.
fn hidden_rules(sticks: &[(u16, u16)]) -> String {
    if sticks.is_empty() {
        return String::new();
    }
    let mut text = HIDDEN_RULE_HEADER.to_string();
    for &(vendor, product) in sticks {
        text.push_str(&hidden_rule(vendor, product));
    }
    text
}

/// The (vendor, product) ids of the sticks in a rule file's lines.
fn rule_sticks(text: &str) -> Vec<(u16, u16)> {
    let id = |line: &str, attr: &str| {
        let key = format!("ATTRS{{{attr}}}==\"");
        let start = line.find(&key)? + key.len();
        u16::from_str_radix(line.get(start..start + 4)?, 16).ok()
    };
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| Some((id(l, "idVendor")?, id(l, "idProduct")?)))
        .collect()
}

/// The sticks whose raw HID node the hidden-stick rule closes now.
pub fn hidden_sticks() -> Vec<(u16, u16)> {
    rule_sticks(&std::fs::read_to_string(HIDDEN_RULE_PATH).unwrap_or_default())
}

/// Closes the raw HID node of exactly these sticks (the hidden-stick rule; none: removed) with
/// administrator rights like [`install`]; nothing to do when the rule says so already. Returns
/// whether it changed. Sticks shown again get their access back from the other rules at once.
/// Blocking until the user answered: call it off the GUI thread.
pub fn set_hidden_sticks(sticks: &[(u16, u16)]) -> Result<bool, String> {
    let mut sticks = sticks.to_vec();
    sticks.sort_unstable();
    sticks.dedup();
    let mut now = hidden_sticks();
    now.sort_unstable();
    if now == sticks {
        return Ok(false);
    }
    if sticks.is_empty() {
        pkexec(&[REMOVE_SCRIPT, "sh", HIDDEN_RULE_PATH])?;
    } else {
        pkexec(&[
            INSTALL_SCRIPT,
            "sh",
            &hidden_rules(&sticks),
            HIDDEN_RULE_PATH,
        ])?;
    }
    Ok(true)
}

/// The hidraw rule file `current` with the stick's line added (the header first when new).
fn with_raw_rule(current: &str, vendor: u16, product: u16) -> String {
    let line = raw_rule(vendor, product);
    let mut text = if current.trim().is_empty() {
        RAW_RULE_HEADER.to_string()
    } else {
        current.to_string()
    };
    if !text.lines().any(|l| format!("{l}\n") == line) {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&line);
    }
    text
}

/// Whether the user lacks access the rule would give: `/dev/uinput` cannot be opened for
/// writing, or a game controller's node cannot be read. False with the rule installed (a
/// session without seat, e.g. over SSH, does not get `uaccess`; nothing to fix then).
pub fn missing() -> bool {
    if Path::new(RULE_PATH).exists() {
        return false;
    }
    let uinput = crate::available().is_err();
    uinput || controllers_missing()
}

/// Some game controller's event node exists but cannot be read.
pub fn controllers_missing() -> bool {
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
            // Our own virtual keyboard is not one of the user's.
            let phys = std::fs::read_to_string(e.path().join("device/phys")).unwrap_or_default();
            !phys.starts_with(crate::OWN_PHYS)
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

/// Lets the user read the raw HID node of the USB stick with these ids, like [`install`]: adds
/// its line to the hidraw rule file (others stay).
pub fn install_raw(vendor: u16, product: u16) -> Result<(), String> {
    let current = std::fs::read_to_string(RAW_RULE_PATH).unwrap_or_default();
    let text = with_raw_rule(&current, vendor, product);
    pkexec(&[INSTALL_SCRIPT, "sh", &text, RAW_RULE_PATH])
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
    fn raw_rules_are_per_stick_and_added_once() {
        let line = raw_rule(0x231d, 0x0200);
        assert_eq!(
            line,
            "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"231d\", ATTRS{idProduct}==\"0200\", \
             TAG+=\"uaccess\"\n"
        );
        let one = with_raw_rule("", 0x231d, 0x0200);
        assert!(one.starts_with('#'));
        assert!(one.ends_with(&line));
        assert_eq!(with_raw_rule(&one, 0x231d, 0x0200), one, "already there");
        let two = with_raw_rule(&one, 0x231d, 0x0201);
        assert_eq!(two.lines().filter(|l| !l.starts_with('#')).count(), 2);
        assert_ne!(RAW_RULE_PATH, RULE_PATH);
    }

    #[test]
    fn hidden_rules_close_hidraw_per_stick() {
        assert_eq!(hidden_rules(&[]), "");
        let text = hidden_rules(&[(0x044f, 0xb687), (0x231d, 0x0200)]);
        assert!(text.starts_with('#'));
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.starts_with("SUBSYSTEM==\"hidraw\"")
            && l.contains("TAG-=\"uaccess\"")
            && l.contains("MODE=\"0600\"")));
        assert_eq!(rule_sticks(&text), [(0x044f, 0xb687), (0x231d, 0x0200)]);
        assert_eq!(rule_sticks(&raw_rule(0x231d, 0x0201)), [(0x231d, 0x0201)]);
        assert!(rule_sticks("# ATTRS{idVendor}==\"1234\"").is_empty());
        // After every rule that gives access, before systemd applies it.
        let name = |p: &str| p.rsplit('/').next().unwrap().to_string();
        assert!(name(HIDDEN_RULE_PATH) > name(RAW_RULE_PATH));
        assert!(name(HIDDEN_RULE_PATH) > name(RULE_PATH));
        assert!(name(HIDDEN_RULE_PATH).as_str() < "73-seat-late.rules");
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
        assert_eq!(calls.lines().count(), 4, "{calls}");
        assert!(calls.starts_with("control --reload-rules"), "{calls}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
