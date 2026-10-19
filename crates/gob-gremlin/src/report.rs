//! Import summary and consistency checks run after loading.

use std::collections::{BTreeMap, HashSet};

use gob_core::{Profile, SUPPORTED_KINDS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// An action or input references an action id missing from the library.
    DanglingRef { from: String, to: String },
    /// An input is bound in a mode that is not declared in `<modes>`.
    UnknownMode(String),
    /// Action type is preserved on save but Joystick Gobloon cannot execute it yet.
    NotExecutable { kind: String, count: usize },
    /// The profile contains user Python scripts, which Joystick Gobloon does not run.
    HasScripts,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DanglingRef { from, to } => {
                write!(f, "missing action {to} (referenced from {from})")
            }
            Self::UnknownMode(m) => write!(f, "inputs use the undeclared mode \"{m}\""),
            Self::NotExecutable { kind, count } => {
                write!(f, "{count} \"{kind}\" action(s) are kept but not executed")
            }
            Self::HasScripts => write!(f, "Python plugins are kept but not run"),
        }
    }
}

impl std::fmt::Display for ImportReport {
    /// One-line summary, e.g. for a log file.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "format {}: {} inputs, {} actions, {} devices, {} modes, {} warnings",
            self.version,
            self.inputs,
            self.actions,
            self.devices,
            self.modes,
            self.warnings.len()
        )
    }
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
        for cfg in &b.configs {
            dangling(
                &format!("{} {} {}", b.device_id, b.input_type, b.input_id),
                &cfg.root_action,
            );
        }
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
