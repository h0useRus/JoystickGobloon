//! Checks a plugin for mistakes an author makes: unknown views or parts, hotspots outside the
//! picture or on top of each other, inputs used twice, controls without inputs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::input::Input;
use crate::inputs::Entry;
use crate::plugin::{DevicePlugin, PARTS};
use crate::svg::view_box;

/// Hotspots closer than this (in SVG units) are hard to tell apart.
const MIN_GAP: f32 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Works, but looks unfinished (e.g. a control nobody taught yet).
    Warning,
    /// Wrong: the plugin does not show or bind as written.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub level: Level,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.level {
            Level::Warning => "warning",
            Level::Error => "error",
        };
        write!(f, "{level}: {}", self.message)
    }
}

pub fn check(plugin: &DevicePlugin) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut error = |message: String| {
        issues.push(Issue {
            level: Level::Error,
            message,
        })
    };
    let manifest = &plugin.manifest;
    if manifest.matches.is_empty() {
        error("no [[match]]: the plugin never fits a device".into());
    }
    for m in &manifest.matches {
        if m.pid.is_none() && m.name.is_none() {
            error(format!("match vid {:04X} needs a pid or a name", m.vid));
        }
    }
    let mut boxes = BTreeMap::new();
    for view in &manifest.views {
        match plugin.svgs.get(&view.id).and_then(|svg| view_box(svg)) {
            Some(b) => {
                boxes.insert(view.id.as_str(), b);
            }
            None => error(format!("view `{}`: {} has no viewBox", view.id, view.svg)),
        }
    }
    let mut ids = BTreeSet::new();
    let mut used: BTreeMap<Input, String> = BTreeMap::new();
    let mut warnings = Vec::new();
    for control in &manifest.controls {
        let id = &control.id;
        if !ids.insert(id.as_str()) {
            error(format!("control `{id}` appears twice"));
        }
        let parts = control.parts();
        for part in &parts {
            if !PARTS.contains(part) {
                error(format!("control `{id}`: unknown part `{part}`"));
            }
        }
        match boxes.get(control.view.as_str()) {
            None if !manifest.views.iter().any(|v| v.id == control.view) => {
                error(format!("control `{id}`: no view `{}`", control.view))
            }
            None => {}
            Some([x, y, w, h]) => {
                let [ax, ay] = control.at;
                if ax < *x || ay < *y || ax > x + w || ay > y + h {
                    error(format!(
                        "control `{id}`: hotspot {ax},{ay} is outside its picture"
                    ));
                }
            }
        }
        for part in &parts {
            let name = if part.is_empty() {
                id.clone()
            } else {
                format!("{id}.{part}")
            };
            match plugin.input_of(id, part) {
                None => warnings.push(format!("{name} sends no input yet (teach it)")),
                Some(input) => {
                    let axis = matches!(input, Input::Axis(_));
                    if axis != control.kind.is_axis_part(part) {
                        let wanted = if axis { "a button or hat" } else { "an axis" };
                        error(format!("{name} is {input}, it needs {wanted}"));
                    }
                    if let Some(other) = used.insert(input, name.clone()) {
                        error(format!("{input} is used by both {other} and {name}"));
                    }
                }
            }
        }
    }
    for (control, entry) in &plugin.inputs.0 {
        let Some(c) = plugin.control(control) else {
            error(format!("inputs: no control `{control}`"));
            continue;
        };
        if let Entry::Parts(parts) = entry {
            let known = c.parts();
            for part in parts.keys() {
                if !known.contains(&part.as_str()) {
                    error(format!("inputs: control `{control}` has no part `{part}`"));
                }
            }
        }
    }
    let controls = &manifest.controls;
    for (i, a) in controls.iter().enumerate() {
        for b in &controls[i + 1..] {
            let gap = (a.at[0] - b.at[0]).hypot(a.at[1] - b.at[1]);
            if a.view == b.view && gap < MIN_GAP {
                warnings.push(format!("hotspots of `{}` and `{}` overlap", a.id, b.id));
            }
        }
    }
    issues.extend(warnings.into_iter().map(|message| Issue {
        level: Level::Warning,
        message,
    }));
    issues
}
