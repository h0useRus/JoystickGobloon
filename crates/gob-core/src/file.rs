//! Files written whole or not at all, for everything Joystick Gobloon saves (profiles, settings,
//! state files, game bindings).

use std::io::Write;
use std::path::Path;

/// Writes `path` so that a crash or power loss leaves either the old file or the new one, never
/// a cut one: the bytes go to a file next to it (`<name>.new`), reach the disk, then replace it.
pub fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    let new = path.with_file_name(name);
    let written = std::fs::File::create(&new).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    written
        .and_then(|()| std::fs::rename(&new, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&new);
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_leaves_nothing_next_to_it() {
        let dir = std::env::temp_dir().join(format!("gob-core-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.gob");
        write_whole(&path, b"one").unwrap();
        write_whole(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(names.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_folder_fails_without_leftovers() {
        let path = std::env::temp_dir()
            .join("gob-core-no-such-folder")
            .join("a.gob");
        assert!(write_whole(&path, b"x").is_err());
        assert!(!path.with_file_name("a.gob.new").exists());
    }
}
