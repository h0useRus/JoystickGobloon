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
