//! The host's side of an export: reading a target's files for the plugin and writing what the
//! plugin returns, each replaced file copied aside first.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{Files, GameFile, Target, WriteResult};

/// Reads every file of a target; missing or unreadable files have no text.
pub fn read_files(target: &Target) -> Files {
    target
        .files
        .iter()
        .map(|(key, path)| {
            let text = std::fs::read(path)
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned());
            (
                key.clone(),
                GameFile {
                    path: path.clone(),
                    text,
                },
            )
        })
        .collect()
}

/// Most files one glob finds: a game's folder of saves is not searched without end.
pub const GLOB_LIMIT: usize = 2000;
/// Larger files are not binding files and are left out of glob results.
pub const GLOB_MAX_SIZE: u64 = 8 << 20;
/// How much of a found file is read to look for its `Target::glob_text`.
pub const GLOB_PEEK: usize = 4096;

/// The files matching `pattern`, in folder order (names sorted in each folder): `/` separates
/// folders (`\` too), `*` matches any part of one name. Unreadable folders, our own backup
/// copies and files without `text` (when given) in their first [`GLOB_PEEK`] bytes are left
/// out.
pub fn glob(pattern: &str, text: Option<&str>) -> Vec<String> {
    let pattern = pattern.replace('\\', "/");
    let cut = match pattern.find('*') {
        Some(star) => pattern[..star].rfind('/').map_or(0, |i| i + 1),
        None => pattern.rfind('/').map_or(0, |i| i + 1),
    };
    let parts: Vec<&str> = pattern[cut..]
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
    let mut out = Vec::new();
    walk(&pattern[..cut], &parts, text, &mut out);
    out
}

fn walk(dir: &str, parts: &[&str], text: Option<&str>, out: &mut Vec<String>) {
    let Some((first, rest)) = parts.split_first() else {
        return;
    };
    if out.len() >= GLOB_LIMIT {
        return;
    }
    let base = if dir.is_empty() { "." } else { dir };
    if !first.contains('*') {
        let path = format!("{dir}{first}");
        if rest.is_empty() {
            if wanted(&path, text) {
                out.push(path);
            }
        } else {
            walk(&format!("{path}/"), rest, text, out);
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| wildcard(first, n))
        .collect();
    names.sort();
    for name in names {
        if out.len() >= GLOB_LIMIT {
            return;
        }
        let path = format!("{dir}{name}");
        if rest.is_empty() {
            if wanted(&path, text) {
                out.push(path);
            }
        } else if Path::new(&path).is_dir() {
            walk(&format!("{path}/"), rest, text, out);
        }
    }
}

/// A small file that is not a backup copy [`apply`] made (nor a `.new` file a write cut short
/// left), with `text` near its start.
fn wanted(path: &str, text: Option<&str>) -> bool {
    use std::io::Read;
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    if name.contains(".gobak-")
        || name.ends_with(".new")
        || !std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= GLOB_MAX_SIZE)
    {
        return false;
    }
    let Some(text) = text else {
        return true;
    };
    let mut head = Vec::with_capacity(GLOB_PEEK);
    std::fs::File::open(path)
        .and_then(|f| f.take(GLOB_PEEK as u64).read_to_end(&mut head))
        .is_ok_and(|_| String::from_utf8_lossy(&head).contains(text))
}

/// Whether `name` matches `pattern`, where `*` is any run of characters; letters compare
/// without case, as Windows names do.
fn wildcard(pattern: &str, name: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let name = name.to_lowercase();
    let mut pieces = pattern.split('*');
    let first = pieces.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let pieces: Vec<&str> = pieces.collect();
    for (i, piece) in pieces.iter().enumerate() {
        if i + 1 == pieces.len() {
            return rest.len() >= piece.len() && rest.ends_with(piece);
        }
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    rest.is_empty()
}

/// The folders matching `pattern` (like [`glob`], for folders), each with the names the `*`
/// parts of the pattern matched, in order.
pub fn glob_dirs(pattern: &str) -> Vec<(String, Vec<String>)> {
    let pattern = pattern.replace('\\', "/");
    let pattern = pattern.trim_end_matches('/');
    let cut = match pattern.find('*') {
        Some(star) => pattern[..star].rfind('/').map_or(0, |i| i + 1),
        None => pattern.rfind('/').map_or(0, |i| i + 1),
    };
    let parts: Vec<&str> = pattern[cut..]
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
    let mut out = Vec::new();
    walk_dirs(&pattern[..cut], &parts, &mut Vec::new(), &mut out);
    out
}

fn walk_dirs(
    dir: &str,
    parts: &[&str],
    names: &mut Vec<String>,
    out: &mut Vec<(String, Vec<String>)>,
) {
    let Some((first, rest)) = parts.split_first() else {
        let path = dir.trim_end_matches('/');
        if Path::new(path).is_dir() {
            out.push((path.to_string(), names.clone()));
        }
        return;
    };
    if out.len() >= GLOB_LIMIT {
        return;
    }
    if !first.contains('*') {
        walk_dirs(&format!("{dir}{first}/"), rest, names, out);
        return;
    }
    let base = if dir.is_empty() { "." } else { dir };
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| wildcard(first, n))
        .collect();
    found.sort();
    for name in found {
        names.push(name.clone());
        walk_dirs(&format!("{dir}{name}/"), rest, names, out);
        names.pop();
    }
}

/// One copy of a target per folder its `each` pattern finds (see `Target::each`); a target
/// without `each` as it is.
pub fn each(target: Target) -> Vec<Target> {
    let Some(pattern) = target.each.clone() else {
        return vec![target];
    };
    glob_dirs(&pattern)
        .into_iter()
        .map(|(_, names)| {
            let fill = |s: &str| {
                let mut s = s.to_string();
                for (i, name) in names.iter().enumerate() {
                    s = s.replace(&format!("{{{}}}", i + 1), name);
                }
                s
            };
            let map = |m: &std::collections::BTreeMap<String, String>| {
                m.iter().map(|(k, v)| (k.clone(), fill(v))).collect()
            };
            Target {
                id: fill(&target.id),
                name: fill(&target.name),
                files: map(&target.files),
                dirs: map(&target.dirs),
                globs: map(&target.globs),
                glob_text: map(&target.glob_text),
                version_file: target.version_file.clone(),
                each: None,
            }
        })
        .collect()
}

/// A target with the files of its globs added to `files`.
pub fn expand(mut target: Target) -> Target {
    for (key, pattern) in &target.globs {
        let text = target.glob_text.get(key).map(String::as_str);
        for path in glob(pattern, text) {
            target.files.insert(format!("{key}:{path}"), path);
        }
    }
    target
}

/// Whether a target is installed: its version file exists, or (without one) any of its files
/// or folders.
pub fn installed(target: &Target) -> bool {
    match target
        .version_file
        .as_ref()
        .and_then(|k| target.files.get(k))
    {
        Some(path) => Path::new(path).is_file(),
        None => {
            target.files.values().any(|p| Path::new(p).is_file())
                || target.dirs.values().any(|p| Path::new(p).is_dir())
        }
    }
}

/// What a write did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Written {
    /// Files written.
    pub files: Vec<PathBuf>,
    /// Copies of the files they replaced.
    pub backups: Vec<PathBuf>,
}

/// The suffix of backup copies: `.gobak-20261109-153816` (UTC).
pub fn backup_suffix(now: SystemTime) -> String {
    let secs = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        ".gobak-{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Writes a plugin's result: changed target files and new files in the target's folders. Files
/// whose text did not change are left alone; replaced files are copied aside first.
pub fn apply(target: &Target, files: &Files, result: &WriteResult) -> Result<Written, String> {
    let suffix = backup_suffix(SystemTime::now());
    let mut out = Written::default();
    let mut writes: Vec<(PathBuf, &str)> = Vec::new();
    for (key, text) in &result.files {
        let path = target
            .files
            .get(key)
            .ok_or_else(|| format!("the plugin wrote a file it does not have: {key}"))?;
        if files.get(key).and_then(|f| f.text.as_deref()) == Some(text.as_str()) {
            continue;
        }
        writes.push((PathBuf::from(path), text));
    }
    for new in &result.create {
        let dir = target.dirs.get(&new.dir).ok_or_else(|| {
            format!(
                "the plugin wrote into a folder it does not have: {}",
                new.dir
            )
        })?;
        let name = new.name.trim();
        if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
            return Err(format!("the plugin gave a bad file name: {name}"));
        }
        let path = Path::new(dir).join(name);
        if std::fs::read_to_string(&path).is_ok_and(|old| old == new.text) {
            continue;
        }
        writes.push((path, &new.text));
    }
    for (path, text) in writes {
        if path.is_file() {
            let mut backup = path.clone().into_os_string();
            backup.push(&suffix);
            let backup = PathBuf::from(backup);
            std::fs::copy(&path, &backup).map_err(|e| format!("{}: {e}", backup.display()))?;
            out.backups.push(backup);
        } else if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        // A running game may keep its file open so it cannot be replaced: then it is written
        // in place (the backup above keeps the old text).
        gob_core::file::write_whole(&path, text.as_bytes())
            .or_else(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied if path.is_file() => {
                    std::fs::write(&path, text)
                }
                _ => Err(e),
            })
            .map_err(|e| format!("{}: {e}", path.display()))?;
        out.files.push(path);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
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
}
