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

/// The files matching `pattern`, sorted: `/` separates folders (`\` too), `*` matches any part
/// of one name. Unreadable folders are skipped.
pub fn glob(pattern: &str) -> Vec<String> {
    let pattern = pattern.replace('\\', "/");
    let (start, rest) = match pattern.find('*') {
        Some(star) => {
            let cut = pattern[..star].rfind('/').map_or(0, |i| i + 1);
            (pattern[..cut].to_string(), pattern[cut..].to_string())
        }
        None => {
            return if Path::new(&pattern).is_file() {
                vec![pattern]
            } else {
                Vec::new()
            };
        }
    };
    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    let mut out = Vec::new();
    walk(&start, &parts, &mut out);
    out.sort();
    out
}

fn walk(dir: &str, parts: &[&str], out: &mut Vec<String>) {
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
            if is_small_file(Path::new(&path)) {
                out.push(path);
            }
        } else {
            walk(&format!("{path}/"), rest, out);
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
            if is_small_file(Path::new(&path)) {
                out.push(path);
            }
        } else if Path::new(&path).is_dir() {
            walk(&format!("{path}/"), rest, out);
        }
    }
}

fn is_small_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= GLOB_MAX_SIZE)
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

/// A target with the files of its globs added to `files`.
pub fn expand(mut target: Target) -> Target {
    for (key, pattern) in &target.globs {
        for path in glob(pattern) {
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
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
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
            version_file: None,
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
        }
        std::fs::create_dir_all(dir.join("userdata/333/other/remote")).unwrap();
        let base = dir.to_string_lossy().replace('\\', "/");
        let found = glob(&format!("{base}/userdata/*/1250410/remote/inputprofile_*"));
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
            version_file: None,
        });
        assert_eq!(target.files.len(), 2);
        assert_eq!(target.files[&format!("p:{}", found[0])], found[0]);
        assert!(installed(&target));
        assert!(glob(&format!("{base}/nothing/*/x")).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
