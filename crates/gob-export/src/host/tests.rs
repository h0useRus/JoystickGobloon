use super::*;
use crate::types::NewFile;
use std::collections::BTreeMap;
use std::time::Duration;

#[test]
fn backup_suffix_is_a_utc_date() {
    let t = UNIX_EPOCH + Duration::from_secs(1_793_806_696); // 2026-11-04 15:38:16 UTC
    assert_eq!(backup_suffix(t), ".gobak-20261104-153816");
    assert_eq!(backup_suffix(UNIX_EPOCH), ".gobak-19700101-000000");
}

#[test]
fn writes_with_backups_and_skips_unchanged() {
    let dir = std::env::temp_dir().join(format!("gob-export-apply-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("maps")).unwrap();
    let binds = dir.join("binds.txt");
    let same = dir.join("same.txt");
    std::fs::write(&binds, "old").unwrap();
    std::fs::write(&same, "same").unwrap();
    let s = |p: &Path| p.to_string_lossy().into_owned();
    let target = Target {
        id: "t".into(),
        name: "t".into(),
        files: BTreeMap::from([("binds".into(), s(&binds)), ("same".into(), s(&same))]),
        dirs: BTreeMap::from([("maps".into(), s(&dir.join("maps")))]),
        globs: BTreeMap::new(),
        glob_text: BTreeMap::new(),
        version_file: None,
        each: None,
    };
    assert!(installed(&target));
    let files = read_files(&target);
    assert_eq!(files["binds"].text.as_deref(), Some("old"));
    let result = WriteResult {
        files: BTreeMap::from([
            ("binds".into(), "new".into()),
            ("same".into(), "same".into()),
        ]),
        create: vec![NewFile {
            dir: "maps".into(),
            name: "layout.xml".into(),
            text: "<x/>".into(),
        }],
        changes: Vec::new(),
    };
    let written = apply(&target, &files, &result).unwrap();
    assert_eq!(written.files.len(), 2);
    assert_eq!(written.backups.len(), 1);
    assert_eq!(std::fs::read_to_string(&binds).unwrap(), "new");
    assert_eq!(std::fs::read_to_string(&written.backups[0]).unwrap(), "old");
    assert_eq!(
        std::fs::read_to_string(dir.join("maps/layout.xml")).unwrap(),
        "<x/>"
    );

    let bad = WriteResult {
        create: vec![NewFile {
            dir: "maps".into(),
            name: "../evil.txt".into(),
            text: String::new(),
        }],
        ..WriteResult::default()
    };
    assert!(apply(&target, &files, &bad).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn wildcards_match_parts_of_names() {
    assert!(wildcard("inputprofile_*", "inputprofile_A1B2"));
    assert!(wildcard("inputprofile_*", "InputProfile_x"));
    assert!(!wildcard("inputprofile_*", "container.3"));
    assert!(wildcard("*", "anything"));
    assert!(wildcard("a*b*c", "a-b-c"));
    assert!(!wildcard("a*b*c", "a-c-b"));
    assert!(wildcard("*.xml", "x.xml"));
    assert!(!wildcard("*ab*b", "ab"));
    assert!(wildcard("plain", "PLAIN"));
    assert!(!wildcard("plain", "plainer"));
}

#[test]
fn globs_add_the_files_they_find() {
    let dir = std::env::temp_dir().join(format!("gob-export-glob-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for user in ["111", "222"] {
        let remote = dir.join("userdata").join(user).join("1250410/remote");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join(format!("inputprofile_{user}")), "x").unwrap();
        std::fs::write(remote.join("UserCfg.opt"), "y").unwrap();
        // Our own backup copies are not the game's files.
        std::fs::write(
            remote.join(format!("inputprofile_{user}.gobak-20261118-101500")),
            "x",
        )
        .unwrap();
    }
    std::fs::create_dir_all(dir.join("userdata/333/other/remote")).unwrap();
    let base = dir.to_string_lossy().replace('\\', "/");
    let found = glob(
        &format!("{base}/userdata/*/1250410/remote/inputprofile_*"),
        None,
    );
    assert_eq!(
        found,
        [
            format!("{base}/userdata/111/1250410/remote/inputprofile_111"),
            format!("{base}/userdata/222/1250410/remote/inputprofile_222"),
        ]
    );
    let target = expand(Target {
        id: "t".into(),
        name: "t".into(),
        files: BTreeMap::new(),
        dirs: BTreeMap::new(),
        globs: BTreeMap::from([(
            "p".into(),
            format!("{base}/userdata/*/1250410/remote/inputprofile_*"),
        )]),
        glob_text: BTreeMap::new(),
        version_file: None,
        each: None,
    });
    assert_eq!(target.files.len(), 2);
    assert_eq!(target.files[&format!("p:{}", found[0])], found[0]);
    assert!(installed(&target));
    assert!(glob(&format!("{base}/nothing/*/x"), None).is_empty());
    // A path without wildcards is a file like any other found.
    let one = format!("{base}/userdata/111/1250410/remote/UserCfg.opt");
    assert_eq!(glob(&one, None), std::slice::from_ref(&one));
    assert!(glob(&format!("{base}/userdata/111/1250410/remote/none"), None).is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn each_folder_makes_a_target() {
    let dir = std::env::temp_dir().join(format!("gob-export-each-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for path in [
        "Game/Input/Plane A",
        "Game.beta/Input/Heli",
        "Other/Input/X",
    ] {
        std::fs::create_dir_all(dir.join(path)).unwrap();
    }
    std::fs::write(dir.join("Game/Input/file"), "not a folder").unwrap();
    let base = dir.to_string_lossy().replace('\\', "/");
    let targets = each(Target {
        id: "{1}/{2}".into(),
        name: "{2}".into(),
        files: BTreeMap::from([("k".into(), format!("{base}/{{1}}/Input/{{2}}/k.txt"))]),
        dirs: BTreeMap::new(),
        globs: BTreeMap::from([("g".into(), format!("{base}/{{1}}/Input/{{2}}/*.lua"))]),
        glob_text: BTreeMap::new(),
        version_file: None,
        each: Some(format!("{base}/Game*/Input/*")),
    });
    let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["Game/Plane A", "Game.beta/Heli"]);
    assert_eq!(targets[0].name, "Plane A");
    assert_eq!(
        targets[1].files["k"],
        format!("{base}/Game.beta/Input/Heli/k.txt")
    );
    assert_eq!(
        targets[1].globs["g"],
        format!("{base}/Game.beta/Input/Heli/*.lua")
    );
    assert!(targets.iter().all(|t| t.each.is_none()));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn globs_keep_files_with_their_text() {
    let dir = std::env::temp_dir().join(format!("gob-export-text-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("saves")).unwrap();
    std::fs::write(dir.join("saves/a"), "<Device Name=\"x\">").unwrap();
    std::fs::write(dir.join("saves/b"), [0u8, 159, 146, 150]).unwrap();
    let mut late = vec![b' '; GLOB_PEEK];
    late.extend_from_slice(b"<Device");
    std::fs::write(dir.join("saves/c"), late).unwrap();
    let big = std::fs::File::create(dir.join("saves/d")).unwrap();
    big.set_len(GLOB_MAX_SIZE + 1).unwrap();
    drop(big);
    let base = dir.to_string_lossy().replace('\\', "/");
    let all = glob(&format!("{base}/saves/*"), None);
    assert_eq!(all.len(), 3, "the large file is left out: {all:?}");
    assert_eq!(
        glob(&format!("{base}/saves/*"), Some("<Device")),
        [format!("{base}/saves/a")]
    );
    assert!(glob(&format!("{base}/saves/d"), None).is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}
