//! Unsaved profile changes kept on disk, so a crash of the program or the system loses nothing:
//! `<data dir>/autosave/profile_N.xml`, written a moment after the last edit. A new profile
//! lives only there until it is saved; for a profile with a file, `settings.gob` (`autosave`)
//! knows which file the copy belongs to. Saving or discarding the changes removes the copy.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Time after the last edit before the copy is written.
pub const DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// One copy of unsaved changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// The copy, in the autosave folder.
    pub file: PathBuf,
    /// The profile's own file; `None` for a new profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<PathBuf>,
}

/// `<data dir>/autosave`.
pub fn dir() -> Option<PathBuf> {
    crate::settings::data_dir().map(|d| d.join("autosave"))
}

/// `profile_N.xml` with the smallest free N (from 1) in `dir`, leaving out `taken`.
pub fn next_file(dir: &Path, taken: &[Entry]) -> PathBuf {
    (1..)
        .map(|n| dir.join(format!("profile_{n}.xml")))
        .find(|f| !f.exists() && !taken.iter().any(|e| e.file == *f))
        .expect("a free name")
}

/// Writes the copy the same crash-safe way as the settings.
pub fn write(file: &Path, profile: &gob_core::Profile) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::settings::write_whole(file, gob_gremlin::to_string(profile).as_bytes())
}

/// The copy that belongs to `path`: the profile's own file, or a new profile's copy itself;
/// the newest when there are several.
pub fn find<'a>(entries: &'a [Entry], path: &Path) -> Option<&'a Entry> {
    entries
        .iter()
        .rev()
        .find(|e| e.origin.as_deref() == Some(path) || e.file == path)
}

/// Takes out the entry of the copy `file` and, with `origin`, of every copy of that profile
/// (saved or discarded changes make them all old). Returns the copies to remove.
pub fn take(entries: &mut Vec<Entry>, file: Option<&Path>, origin: Option<&Path>) -> Vec<PathBuf> {
    let mut gone: Vec<PathBuf> = file.map(Path::to_path_buf).into_iter().collect();
    entries.retain(|e| {
        let same =
            Some(e.file.as_path()) == file || (origin.is_some() && e.origin.as_deref() == origin);
        if same && !gone.contains(&e.file) {
            gone.push(e.file.clone());
        }
        !same
    });
    gone
}

enum Job {
    Write(PathBuf, Box<gob_core::Profile>),
    Remove(PathBuf),
}

/// Writes and removes copies on its own thread, in order, so the window never waits for the
/// disk.
pub struct Writer(std::sync::mpsc::Sender<Job>);

impl Writer {
    pub fn start() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("autosave".into())
            .spawn(move || {
                for job in rx {
                    match job {
                        Job::Write(file, profile) => {
                            if let Err(e) = write(&file, &profile) {
                                log::error!("autosave {}: {e}", file.display());
                            }
                        }
                        Job::Remove(file) => {
                            let _ = std::fs::remove_file(file);
                        }
                    }
                }
            })
            .expect("autosave thread");
        Self(tx)
    }

    pub fn write(&self, file: PathBuf, profile: &gob_core::Profile) {
        let _ = self.0.send(Job::Write(file, Box::new(profile.clone())));
    }

    pub fn remove(&self, file: PathBuf) {
        let _ = self.0.send(Job::Remove(file));
    }
}

/// Drops the entries whose copy is gone.
pub fn prune(entries: &mut Vec<Entry>) {
    entries.retain(|e| e.file.is_file());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_take_the_first_free_number() {
        let dir = std::env::temp_dir().join(format!("gobloon-autosave-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(next_file(&dir, &[]), dir.join("profile_1.xml"));
        std::fs::write(dir.join("profile_1.xml"), "").unwrap();
        let taken = [Entry {
            file: dir.join("profile_2.xml"),
            origin: None,
        }];
        assert_eq!(next_file(&dir, &taken), dir.join("profile_3.xml"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_is_found_by_its_profile_or_itself_and_kept_while_it_exists() {
        let dir =
            std::env::temp_dir().join(format!("gobloon-autosave-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (copy, new) = (dir.join("profile_1.xml"), dir.join("profile_2.xml"));
        write(&copy, &gob_core::Profile::default()).unwrap();
        let mut entries = vec![
            Entry {
                file: copy.clone(),
                origin: Some(PathBuf::from("flight.xml")),
            },
            Entry {
                file: new.clone(),
                origin: None,
            },
        ];
        assert_eq!(find(&entries, Path::new("flight.xml")).unwrap().file, copy);
        // A newer copy of the same profile wins.
        let newer = Entry {
            file: dir.join("profile_3.xml"),
            origin: Some(PathBuf::from("flight.xml")),
        };
        let mut more = entries.clone();
        more.push(newer.clone());
        assert_eq!(find(&more, Path::new("flight.xml")), Some(&newer));
        let gone = take(&mut more, Some(&newer.file), newer.origin.as_deref());
        assert_eq!(
            gone,
            [newer.file.clone(), copy.clone()],
            "all copies of flight.xml"
        );
        assert_eq!(more.len(), 1);
        assert_eq!(find(&entries, &new).unwrap().origin, None);
        assert!(find(&entries, Path::new("other.xml")).is_none());
        prune(&mut entries);
        assert_eq!(entries.len(), 1, "profile_2 was never written");
        assert!(gob_gremlin::load(&copy).is_ok(), "the copy is a profile");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_keep_the_entries() {
        #[derive(Serialize, Deserialize)]
        struct S {
            autosave: Vec<Entry>,
        }
        let s = S {
            autosave: vec![
                Entry {
                    file: PathBuf::from("a/profile_1.xml"),
                    origin: Some(PathBuf::from("b/flight.xml")),
                },
                Entry {
                    file: PathBuf::from("a/profile_2.xml"),
                    origin: None,
                },
            ],
        };
        let text = toml::to_string(&s).unwrap();
        let back: S = toml::from_str(&text).unwrap();
        assert_eq!(back.autosave, s.autosave);
    }
}
