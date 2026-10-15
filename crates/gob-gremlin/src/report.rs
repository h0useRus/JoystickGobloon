//! Import summary and consistency checks shown to the user after loading.

use std::collections::{BTreeMap, HashSet};

use gob_core::{Profile, SUPPORTED_KINDS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// An action or input references an action id missing from the library.
    DanglingRef { from: String, to: String },
    /// An input is bound in a mode that is not declared in `<modes>`.
    UnknownMode(String),
    /// Action type is preserved on save but JoyGob cannot execute it yet.
    NotExecutable { kind: String, count: usize },
    /// The profile contains user Python scripts, which JoyGob does not run.
    HasScripts,
}

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub version: String,
    pub inputs: usize,
    pub actions: usize,
    pub devices: usize,
    pub modes: usize,
    pub by_kind: BTreeMap<String, usize>,
    pub warnings: Vec<Warning>,
}

pub fn report(p: &Profile) -> ImportReport {
    let mut r = ImportReport {
        version: p.version.clone(),
        inputs: p.inputs.len(),
        actions: p.library.len(),
        devices: p.devices.len(),
        modes: p.modes.len(),
        ..Default::default()
    };
    for a in &p.library {
        *r.by_kind.entry(a.kind.clone()).or_default() += 1;
    }

    let index = p.action_index();
    let mut dangling = |from: &str, to: &str| {
        if !index.contains_key(to) {
            r.warnings.push(Warning::DanglingRef {
                from: from.into(),
                to: to.into(),
            });
        }
    };
    for b in &p.inputs {
        dangling(
            &format!("{} {} {}", b.device_id, b.input_type, b.input_id),
            &b.root_action,
        );
    }
    for a in &p.library {
        for child in a.child_ids() {
            dangling(&a.id, child);
        }
    }

    let modes: HashSet<&str> = p.modes.iter().map(|m| m.name.as_str()).collect();
    let mut unknown: Vec<&str> = p
        .inputs
        .iter()
        .map(|b| b.mode.as_str())
        .filter(|m| !modes.contains(m))
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    r.warnings
        .extend(unknown.into_iter().map(|m| Warning::UnknownMode(m.into())));

    for (kind, &count) in &r.by_kind {
        if !SUPPORTED_KINDS.contains(&kind.as_str()) {
            r.warnings.push(Warning::NotExecutable {
                kind: kind.clone(),
                count,
            });
        }
    }
    if p.has_scripts() {
        r.warnings.push(Warning::HasScripts);
    }
    r
}
