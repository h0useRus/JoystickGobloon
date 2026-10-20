//! Exclusive reading: while a listed device is attached, the input service reads it alone and
//! other programs (games) get nothing from it. This is how device hiding works where the
//! platform has no hiding driver (Linux: `EVIOCGRAB`, see gob-hide). Where hiding works
//! otherwise (Windows: HidHide) it is not [`supported`] and the list is ignored.

use std::collections::HashSet;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::DeviceInfo;
use crate::platform::{Backend, Device, Platform};

static WANTED: RwLock<Option<HashSet<String>>> = RwLock::new(None);
/// Bumped on every [`set`], so the input thread looks at the list only then.
static VERSION: AtomicU64 = AtomicU64::new(0);

/// Whether this platform reads devices exclusively.
pub fn supported() -> bool {
    Platform::EXCLUSIVE
}

/// The attached controllers that can be read exclusively (not our own virtual devices); empty
/// where it is not [`supported`]. Opens each device briefly: call it off the GUI thread or
/// rarely.
pub fn devices() -> Vec<DeviceInfo> {
    if !supported() {
        return Vec::new();
    }
    let Some(platform) = Platform::new() else {
        return Vec::new();
    };
    platform
        .attached()
        .iter()
        .filter_map(|(_, found)| platform.open(found, false))
        .map(|d| d.state().info.clone())
        .filter(|info| !info.is_vjoy())
        .collect()
}

/// Replaces the list of devices (ids as in [`DeviceInfo::guid`]) to read exclusively; the
/// input service takes and lets go of them on its next round.
pub fn set(ids: impl IntoIterator<Item = String>) {
    let ids = ids.into_iter().map(|id| id.to_ascii_lowercase()).collect();
    *WANTED.write().unwrap_or_else(|e| e.into_inner()) = Some(ids);
    VERSION.fetch_add(1, Ordering::Release);
}

/// The list's version, to notice changes.
pub(crate) fn version() -> u64 {
    VERSION.load(Ordering::Acquire)
}

pub(crate) fn wanted(id: &str) -> bool {
    WANTED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|ids| ids.contains(&id.to_ascii_lowercase()))
}
