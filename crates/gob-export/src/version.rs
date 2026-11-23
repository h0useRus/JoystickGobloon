//! Game versions: dotted numbers ("4.10.197.2878"), compared segment by segment.

use std::cmp::Ordering;

/// Compares two versions segment by segment: numbers as numbers, anything else as text; a
/// missing segment counts as 0, so "4.10" equals "4.10.0".
pub fn compare(a: &str, b: &str) -> Ordering {
    let mut a = a.trim().split('.');
    let mut b = b.trim().split('.');
    loop {
        let (x, y) = match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (x, y) => (x.unwrap_or("0"), y.unwrap_or("0")),
        };
        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
}

/// Whether `version` is at most `max`. A `max` ending in ".*" covers every version starting with
/// it: "4.10.*" takes "4.10.197.2878" but not "4.11".
pub fn at_most(version: &str, max: &str) -> bool {
    match max.trim().strip_suffix(".*") {
        Some(prefix) => {
            let len = prefix.split('.').count();
            let head: Vec<&str> = version.trim().split('.').take(len).collect();
            compare(&head.join("."), prefix) != Ordering::Greater
        }
        None => compare(version, max) != Ordering::Greater,
    }
}

/// The game versions a plugin supports; both ends optional and inclusive.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Range {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
}

impl Range {
    pub fn contains(&self, version: &str) -> bool {
        self.min
            .as_deref()
            .is_none_or(|min| compare(version, min) != Ordering::Less)
            && self.max.as_deref().is_none_or(|max| at_most(version, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_compare_as_numbers() {
        assert_eq!(compare("4.10.197", "4.9.500"), Ordering::Greater);
        assert_eq!(compare("4.10", "4.10.0.0"), Ordering::Equal);
        assert_eq!(compare("4.10.197.2878", "4.10.200.251"), Ordering::Less);
        assert_eq!(compare("1.2b", "1.2a"), Ordering::Greater);
    }

    #[test]
    fn wildcard_max_covers_its_prefix() {
        assert!(at_most("4.10.197.2878", "4.10.*"));
        assert!(at_most("4.9", "4.10.*"));
        assert!(!at_most("4.11.0", "4.10.*"));
        assert!(at_most("4.10", "4.10"));
        assert!(!at_most("4.10.1", "4.10"));
    }

    #[test]
    fn range_ends_are_inclusive() {
        let r = Range {
            min: Some("4.9".into()),
            max: Some("4.10.*".into()),
        };
        assert!(r.contains("4.9"));
        assert!(r.contains("4.10.200.251"));
        assert!(!r.contains("4.8.9"));
        assert!(!r.contains("5.0"));
        assert!(Range::default().contains("1"));
    }
}
