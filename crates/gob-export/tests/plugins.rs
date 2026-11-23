//! The sample plugin in tests/plugins: discovery, catalogs, renames and every hook.

use std::path::{Path, PathBuf};

use gob_export::*;

fn plugins_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/plugins")
}

fn sample() -> Plugin {
    Plugin::load(&plugins_dir().join("sample")).expect("sample plugin loads")
}

fn files(binds: Option<&str>, version: Option<&str>) -> Files {
    let file = |path: &str, text: Option<&str>| GameFile {
        path: path.into(),
        text: text.map(str::to_string),
    };
    [
        ("binds".to_string(), file("C:/Sample/binds.txt", binds)),
        (
            "version".to_string(),
            file("C:/Sample/version.txt", version),
        ),
    ]
    .into()
}

#[test]
fn loads_manifest_catalogs_and_renames() {
    let p = sample();
    assert_eq!(p.id(), "sample");
    assert_eq!(p.manifest.name, "Sample game");
    assert!(p.manifest.game.contains("1.1.5"));
    assert!(!p.manifest.game.contains("2.0"));
    assert_eq!(p.catalogs.versions().collect::<Vec<_>>(), ["1.0", "1.1"]);
    assert_eq!(p.renames.resolve("roll", Some("1.1.5")), "roll_axis");
    for hook in [Hook::Targets, Hook::Version, Hook::Read, Hook::Write] {
        assert!(p.has(hook), "{hook:?}");
    }
}

#[test]
fn finds_targets_and_their_version() {
    let p = sample();
    let env = Env {
        platform: "windows".into(),
        roots: vec!["C:".into(), "D:".into()],
        documents: String::new(),
        local_app_data: String::new(),
    };
    let targets = p.targets(&env).unwrap();
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].files["binds"], "C:/Sample/binds.txt");
    assert_eq!(targets[0].version_file.as_deref(), Some("version"));
    let version = p
        .version(&targets[0], &files(None, Some("1.1.7\r\n")))
        .unwrap();
    assert_eq!(version.as_deref(), Some("1.1.7"));
    assert_eq!(p.version(&targets[0], &files(None, None)).unwrap(), None);
    let (catalog, fit) = p.catalogs.pick(version.as_deref()).unwrap();
    assert_eq!((catalog.version.as_str(), fit), ("1.1", Fit::GameNewer));
}

#[test]
fn write_then_read_gives_the_same_binds() {
    let p = sample();
    let target = p
        .targets(&Env {
            platform: "windows".into(),
            roots: vec!["C:".into()],
            documents: String::new(),
            local_app_data: String::new(),
        })
        .unwrap()
        .remove(0);
    let catalog = p.catalogs.pick(Some("1.1")).unwrap().0.clone();
    let old = "# comment kept\ngear = key:0x22\nlights = vjoy:1:button:9\n";
    let binds = vec![
        Bind {
            action: "gear".into(),
            output: "vjoy:1:button:5".into(),
            invert: false,
            default: false,
        },
        Bind {
            action: "roll_axis".into(),
            output: "vjoy:1:axis:x".into(),
            invert: false,
            default: false,
        },
    ];
    let ctx = WriteContext {
        target: &target,
        files: &files(Some(old), None),
        catalog: &catalog,
        ours: &[],
        devices: &[],
        binds: &binds,
        cleared: &[],
        profile: "Test",
        options: &Default::default(),
    };
    let written = p.write(&ctx).unwrap();
    assert_eq!(
        written.files["binds"],
        "# comment kept\ngear = vjoy:1:button:5\nroll_axis = vjoy:1:axis:x\n"
    );
    let kinds: Vec<_> = written
        .changes
        .iter()
        .map(|c| (c.kind, c.action.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (ChangeKind::Changed, "gear"),
            (ChangeKind::Removed, "lights"),
            (ChangeKind::Added, "roll_axis")
        ]
    );

    let read = p
        .read(&ReadContext {
            target: &target,
            files: &files(Some(&written.files["binds"]), None),
            catalog: &catalog,
            ours: &[],
            devices: &[],
        })
        .unwrap();
    assert_eq!(read.binds, binds);

    // Writing the same binds again changes nothing.
    let again_files = files(Some(&written.files["binds"]), None);
    let again = p
        .write(&WriteContext {
            files: &again_files,
            ..ctx
        })
        .unwrap();
    assert_eq!(again.files["binds"], written.files["binds"]);
    assert!(again.changes.is_empty());
}

#[test]
fn discovery_skips_a_second_plugin_with_the_same_id() {
    let tmp = std::env::temp_dir().join(format!("gob-export-discover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    for name in ["a", "b"] {
        copy_dir(&plugins_dir().join("sample"), &tmp.join(name));
    }
    std::fs::create_dir_all(tmp.join("not-a-plugin")).unwrap();
    let found = discover(&[
        (tmp.clone(), Origin::Bundled),
        (tmp.join("missing"), Origin::User),
    ]);
    std::fs::remove_dir_all(&tmp).unwrap();
    assert_eq!(found.len(), 2);
    assert!(found[0].plugin.is_ok());
    let e = found[1].plugin.as_ref().unwrap_err();
    assert!(e.message.contains("same id"), "{e}");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            copy_dir(&path, &to.join(entry.file_name()));
        } else {
            std::fs::copy(&path, to.join(entry.file_name())).unwrap();
        }
    }
}
