use super::*;

fn release(tag: &str, prerelease: bool) -> GhRelease {
    GhRelease {
        tag_name: tag.into(),
        prerelease,
        draft: false,
        assets: vec![GhAsset {
            name: format!("JoystickGobloon-{}-setup.exe", tag.trim_start_matches('v')),
            browser_download_url: String::new(),
            digest: None,
        }],
    }
}

#[test]
fn channels_pick_the_newest_release_they_take() {
    let list = || {
        vec![
            release("v1.0.1-alpha.120", true),
            release("v1.0.0", false),
            release("v1.0.0-alpha.112", true),
        ]
    };
    let pick = |current: &str, channel| {
        newest(
            list(),
            &Version::parse(current).unwrap(),
            "-setup.exe",
            channel,
        )
        .unwrap()
        .map(|r| r.version.to_string())
    };
    // Release channel: finals only, also when an alpha is newer.
    assert_eq!(
        pick("0.15.3", UpdateChannel::Release).as_deref(),
        Some("1.0.0")
    );
    assert_eq!(pick("1.0.0", UpdateChannel::Release), None);
    // Alpha channel: whatever is newest.
    assert_eq!(
        pick("1.0.0", UpdateChannel::Alpha).as_deref(),
        Some("1.0.1-alpha.120")
    );
    assert_eq!(
        pick("1.0.0-alpha.112", UpdateChannel::Alpha).as_deref(),
        Some("1.0.1-alpha.120")
    );
    // An alpha on the release channel waits for the next final release.
    assert_eq!(pick("1.0.1-alpha.120", UpdateChannel::Release), None);
    let mut with_final = list();
    with_final.insert(0, release("v1.0.1", false));
    let next = newest(
        with_final,
        &Version::parse("1.0.1-alpha.120").unwrap(),
        "-setup.exe",
        UpdateChannel::Release,
    )
    .unwrap();
    assert_eq!(
        next.map(|r| r.version.to_string()).as_deref(),
        Some("1.0.1")
    );
    // The newest build is still uploading its files: the next newest that has them.
    let mut uploading = list();
    let mut bare = release("v1.0.1-alpha.121", true);
    bare.assets.clear();
    uploading.insert(0, bare);
    let found = newest(
        uploading,
        &Version::parse("1.0.0").unwrap(),
        "-setup.exe",
        UpdateChannel::Alpha,
    )
    .unwrap();
    assert_eq!(
        found.map(|r| r.version.to_string()).as_deref(),
        Some("1.0.1-alpha.120")
    );
}

fn zip_with(dir: &Path, names: &[&str]) -> PathBuf {
    use std::io::Write;
    let path = dir.join("test.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for name in names {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
    }
    zip.finish().unwrap();
    path
}

#[test]
fn unpacks_a_portable_zip_and_refuses_paths_outside_the_folder() {
    let dir = std::env::temp_dir().join(format!("gobloon-unzip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("out")).unwrap();
    let good = zip_with(&dir, &["JoystickGobloon/lang/de.ftl"]);
    unpack_zip(&good, &dir.join("out")).unwrap();
    assert!(dir.join("out/JoystickGobloon/lang/de.ftl").is_file());
    let bad = zip_with(&dir, &["../evil.txt"]);
    assert!(unpack_zip(&bad, &dir.join("out")).is_err());
    assert!(!dir.join("evil.txt").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_portable_update_puts_every_file_back() {
    let root = std::env::temp_dir().join(format!("gobloon-rollback-{}", std::process::id()));
    let (new, old) = (root.join("new"), root.join("old"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(new.join("lang")).unwrap();
    std::fs::create_dir_all(old.join("lang")).unwrap();
    for name in ["sherpa-onnx-c-api.dll", "LICENSE", "NEW.txt", "lang/de.ftl"] {
        std::fs::write(new.join(name), "new").unwrap();
    }
    for name in ["sherpa-onnx-c-api.dll", "LICENSE", "lang/de.ftl"] {
        std::fs::write(old.join(name), "old").unwrap();
    }
    // The language files copy after the others; one fails, as when it cannot be written.
    let failing = |from: &Path, to: &Path| {
        if from.ends_with("de.ftl") {
            Err(std::io::Error::other("locked"))
        } else {
            std::fs::copy(from, to).map(|_| ())
        }
    };
    assert!(copy_portable_with(&new, &old, failing).is_err());
    let mut left: Vec<String> = ["", "lang"]
        .iter()
        .flat_map(|d| std::fs::read_dir(old.join(d)).unwrap().flatten())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort_unstable();
    assert_eq!(
        left,
        ["LICENSE", "de.ftl", "sherpa-onnx-c-api.dll"],
        "nothing new, nothing aside"
    );
    for name in ["sherpa-onnx-c-api.dll", "LICENSE", "lang/de.ftl"] {
        assert_eq!(
            std::fs::read_to_string(old.join(name)).unwrap(),
            "old",
            "{name}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_portable_update_copies_every_file() {
    let root = std::env::temp_dir().join(format!("gobloon-portable-{}", std::process::id()));
    let (new, old) = (root.join("new"), root.join("old"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(new.join("lang")).unwrap();
    std::fs::create_dir_all(&old).unwrap();
    let exe = crate::platform::exe_name();
    for (name, text) in [
        (exe, "new exe"),
        ("sherpa-onnx-c-api.dll", "speech"),
        ("LICENSE", "license"),
        ("lang/de.ftl", "de"),
    ] {
        std::fs::write(new.join(name), text).unwrap();
    }
    std::fs::write(old.join(exe), "old exe").unwrap();
    copy_portable(&new, &old).unwrap();
    for (name, text) in [
        (exe, "new exe"),
        ("sherpa-onnx-c-api.dll", "speech"),
        ("LICENSE", "license"),
        ("lang/de.ftl", "de"),
    ] {
        assert_eq!(
            std::fs::read_to_string(old.join(name)).unwrap(),
            text,
            "{name}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_portable_update_follows_the_manifest() {
    let root = std::env::temp_dir().join(format!("gobloon-manifest-{}", std::process::id()));
    let (new, old) = (root.join("new"), root.join("old"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(new.join("lang")).unwrap();
    std::fs::create_dir_all(old.join("lang")).unwrap();
    let exe = crate::platform::exe_name();
    std::fs::write(
        new.join(MANIFEST),
        format!(
            "files = [\"{exe}\", \"sherpa-onnx-c-api.dll\", \"lang/de.ftl\", \"voice/model.txt\"]
"
        ),
    )
    .unwrap();
    std::fs::create_dir_all(new.join("voice")).unwrap();
    for name in [
        exe,
        "sherpa-onnx-c-api.dll",
        "lang/de.ftl",
        "voice/model.txt",
        "stray.txt",
    ] {
        std::fs::write(new.join(name), "new").unwrap();
    }
    std::fs::write(
        old.join(MANIFEST),
        format!(
            "files = [\"{exe}\", \"libold.dll\", \"lang/de.ftl\"]
"
        ),
    )
    .unwrap();
    for name in [exe, "libold.dll", "lang/de.ftl", "lang/xx.ftl"] {
        std::fs::write(old.join(name), "old").unwrap();
    }
    copy_portable(&new, &old).unwrap();
    for name in [
        exe,
        "sherpa-onnx-c-api.dll",
        "lang/de.ftl",
        "voice/model.txt",
    ] {
        assert_eq!(
            std::fs::read_to_string(old.join(name)).unwrap(),
            "new",
            "{name}"
        );
    }
    assert!(old.join(MANIFEST).exists(), "the new manifest comes along");
    assert!(!old.join("stray.txt").exists(), "only listed files");
    assert!(
        !old.join("libold.dll").exists(),
        "no longer listed: removed"
    );
    assert!(
        old.join("libold.dll.old").exists(),
        "set aside until the next start"
    );
    assert_eq!(
        std::fs::read_to_string(old.join("lang/xx.ftl")).unwrap(),
        "old",
        "the user's own language stays"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_manifest_never_leaves_the_folder() {
    let dir = std::env::temp_dir().join(format!("gobloon-bad-manifest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for bad in [
        "../evil.dll",
        "C:/Windows/evil.dll",
        "/etc/evil",
        "lang\\..\\..\\evil.dll",
        "a/./b",
        "JoystickGobloon.exe.old",
        "",
    ] {
        std::fs::write(
            dir.join(MANIFEST),
            format!(
                "files = [{bad:?}]
"
            ),
        )
        .unwrap();
        assert!(manifest(&dir).is_err(), "{bad}");
    }
    std::fs::write(
        dir.join(MANIFEST),
        "files = [\"lang/en.ftl\"]
",
    )
    .unwrap();
    assert_eq!(
        manifest(&dir).unwrap().unwrap(),
        [PathBuf::from("lang/en.ftl"), PathBuf::from(MANIFEST)]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn temp(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("gobloon-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("new")).unwrap();
    std::fs::create_dir_all(root.join("old")).unwrap();
    root
}

#[test]
fn a_name_differing_only_in_case_is_not_removed() {
    let root = temp("case");
    let (new, old) = (root.join("new"), root.join("old"));
    std::fs::create_dir_all(new.join("lang")).unwrap();
    std::fs::create_dir_all(old.join("lang")).unwrap();
    std::fs::write(new.join(MANIFEST), "files = [\"lang/de.ftl\"]\n").unwrap();
    std::fs::write(new.join("lang/de.ftl"), "new").unwrap();
    std::fs::write(old.join(MANIFEST), "files = [\"lang/DE.ftl\"]\n").unwrap();
    std::fs::write(old.join("lang/DE.ftl"), "old").unwrap();
    copy_portable(&new, &old).unwrap();
    // Not set aside as gone (on Windows both names are the same file).
    assert_eq!(
        std::fs::read_to_string(old.join("lang/de.ftl")).unwrap(),
        "new"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cleanup_removes_what_was_set_aside_and_the_emptied_folders() {
    let root = temp("cleanup");
    let (new, old) = (root.join("new"), root.join("old"));
    std::fs::write(new.join(MANIFEST), "files = [\"a.dll\"]\n").unwrap();
    std::fs::write(new.join("a.dll"), "new").unwrap();
    std::fs::create_dir_all(old.join("voice/deep")).unwrap();
    std::fs::write(
        old.join(MANIFEST),
        "files = [\"a.dll\", \"voice/deep/x.dll\"]\n",
    )
    .unwrap();
    std::fs::write(old.join("a.dll"), "old").unwrap();
    std::fs::write(old.join("voice/deep/x.dll"), "old").unwrap();
    copy_portable(&new, &old).unwrap();
    assert!(old.join("voice/deep/x.dll.old").exists());
    cleanup_in(&old);
    assert!(!old.join("voice").exists(), "the emptied folders go");
    assert!(!old.join("a.dll.old").exists());
    assert!(!old.join(ASIDE).exists());
    assert_eq!(std::fs::read_to_string(old.join("a.dll")).unwrap(), "new");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failed_update_removes_the_folders_it_made() {
    let root = temp("folders");
    let (new, old) = (root.join("new"), root.join("old"));
    std::fs::create_dir_all(new.join("voice/deep")).unwrap();
    std::fs::write(
        new.join(MANIFEST),
        "files = [\"voice/deep/model.txt\", \"b.dll\"]\n",
    )
    .unwrap();
    std::fs::write(new.join("voice/deep/model.txt"), "new").unwrap();
    std::fs::write(new.join("b.dll"), "new").unwrap();
    let failing = |from: &Path, to: &Path| {
        if from.ends_with("b.dll") {
            Err(std::io::Error::other("locked"))
        } else {
            std::fs::copy(from, to).map(|_| ())
        }
    };
    assert!(copy_portable_with(&new, &old, failing).is_err());
    assert_eq!(std::fs::read_dir(&old).unwrap().count(), 0, "nothing left");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_unreadable_new_folder_is_an_error() {
    let root = temp("unreadable");
    assert!(copy_portable(&root.join("missing"), &root.join("old")).is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_portable_version_must_list_its_exe() {
    let root = temp("exe");
    let new = root.join("new");
    let exe = crate::platform::exe_name();
    std::fs::write(new.join(exe), "new").unwrap();
    std::fs::write(new.join("a.dll"), "new").unwrap();
    std::fs::write(new.join(MANIFEST), "files = [\"a.dll\"]\n").unwrap();
    assert!(check_portable(&new).is_err());
    std::fs::write(
        new.join(MANIFEST),
        format!("files = [\"a.dll\", \"{exe}\"]\n"),
    )
    .unwrap();
    check_portable(&new).unwrap();
    std::fs::write(
        new.join(MANIFEST),
        format!("files = [\"b.dll\", \"{exe}\"]\n"),
    )
    .unwrap();
    assert!(check_portable(&new).is_err(), "a listed file is missing");
    let _ = std::fs::remove_dir_all(&root);
}

/// Network: the alpha channel's list has alpha builds, and an old version is offered one.
#[test]
#[ignore]
fn the_alpha_channel_reads_the_recent_releases() {
    let releases: Vec<GhRelease> = agent()
        .get(RECENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap();
    assert!(releases.iter().any(|r| r.prerelease), "alpha builds listed");
    let old = Version::parse("0.15.3").unwrap();
    let found = newest(releases, &old, "-setup.exe", UpdateChannel::Alpha).unwrap();
    assert!(found.is_some_and(|r| r.version > old));
}

/// Network: `cargo test -p joystick-gobloon -- --ignored`. This build's version is not older than
/// the latest release, so nothing is offered; the request, TLS and JSON work.
#[test]
#[ignore]
fn asks_github_for_the_latest_release() {
    assert!(latest(UpdateChannel::Release).unwrap().is_none());
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
    assert!(dir.join(crate::platform::exe_name()).exists());
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
