//! Who may use the service: root and the user active on a seat (logind writes
//! `ACTIVE_UID=<uid>` into `/run/systemd/seats/<seat>`). Games do not speak the protocol; the goal
//! is that they do not see hidden devices, not keeping out a determined local user.

use std::path::Path;

const SEATS: &str = "/run/systemd/seats";

/// `ACTIVE_UID` of one seat file.
fn active_uid(text: &str) -> Option<u32> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("ACTIVE_UID="))
        .and_then(|v| v.trim().parse().ok())
}

/// The users active on a seat now.
pub fn active_uids() -> Vec<u32> {
    active_uids_in(Path::new(SEATS))
}

fn active_uids_in(dir: &Path) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|text| active_uid(&text))
        .collect()
}

/// Whether `uid` may use the service now.
pub fn allowed(uid: u32) -> bool {
    uid == 0 || active_uids().contains(&uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_uid_from_seat_file() {
        let seat = "# This is private data. Do not parse.\nIS_SEAT0=1\nCAN_MULTI_SESSION=1\n\
                    ACTIVE=c2\nACTIVE_UID=1000\nSESSIONS=c2 3\nUIDS=1000 1001\n";
        assert_eq!(active_uid(seat), Some(1000));
        assert_eq!(active_uid("IS_SEAT0=1\n"), None, "nobody active");
        assert_eq!(active_uid("ACTIVE_UID=x\n"), None);
    }

    #[test]
    fn seats_folder() {
        let dir = std::env::temp_dir().join(format!("system-gobloon-seats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("seat0"), "ACTIVE_UID=1000\n").unwrap();
        std::fs::write(dir.join("seat1"), "IS_SEAT0=0\n").unwrap();
        assert_eq!(active_uids_in(&dir), [1000]);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(active_uids_in(&dir).is_empty(), "no logind");
    }
}
