//! How a press of an input reaches one of its actions: the mode, the modifier layer and the
//! branches of container actions on the way (short or long press, single or double tap, ...).
//! Lets views tell apart several outputs of one input.

use std::collections::{HashMap, HashSet};

use crate::action::Action;
use crate::modifier::Layer;
use crate::profile::Profile;

/// The way from an input to one action under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub mode: String,
    pub layer: Layer,
    /// Per container on the way: its kind and the child group holding the next action, e.g.
    /// `("tempo", "long-actions")`. Containers without groups are left out.
    pub branches: Vec<(String, String)>,
}

impl Profile {
    /// The route from binding `binding` to action `target`: the first configuration whose tree
    /// holds it, the first way found in that tree. `None` when no configuration reaches it.
    pub fn route(&self, binding: usize, target: &str) -> Option<Route> {
        self.route_in(&self.action_index(), binding, target)
    }

    /// [`Profile::route`] with the [`Profile::action_index`] built already (for callers asking
    /// about many routes at once).
    pub fn route_in(
        &self,
        index: &HashMap<&str, &Action>,
        binding: usize,
        target: &str,
    ) -> Option<Route> {
        let b = self.inputs.get(binding)?;
        b.configs.iter().find_map(|c| {
            let mut branches = Vec::new();
            let mut seen = HashSet::new();
            walk(index, &c.root_action, target, &mut seen, &mut branches).then(|| Route {
                mode: b.mode.clone(),
                layer: c.layer.clone(),
                branches,
            })
        })
    }
}

fn walk(
    index: &HashMap<&str, &Action>,
    id: &str,
    target: &str,
    seen: &mut HashSet<String>,
    branches: &mut Vec<(String, String)>,
) -> bool {
    if id == target {
        return true;
    }
    if !seen.insert(id.to_string()) {
        return false;
    }
    let Some(action) = index.get(id) else {
        return false;
    };
    for (group, child) in action.children_by_group() {
        let step = !group.is_empty() && action.kind != "root";
        if step {
            branches.push((action.kind.clone(), group.to_string()));
        }
        if walk(index, child, target, seen, branches) {
            return true;
        }
        if step {
            branches.pop();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Item;
    use crate::profile::{ActionConfig, InputBinding};

    fn action(id: &str, kind: &str, groups: &[(&str, &[&str])]) -> Action {
        Action {
            id: id.into(),
            kind: kind.into(),
            items: groups
                .iter()
                .map(|(tag, ids)| Item::Group {
                    tag: (*tag).into(),
                    attrs: Vec::new(),
                    items: ids.iter().map(|i| Item::ActionRef((*i).into())).collect(),
                })
                .collect(),
        }
    }

    fn profile() -> Profile {
        Profile {
            library: vec![
                action("root", "root", &[("actions", &["tempo"])]),
                action(
                    "tempo",
                    "tempo",
                    &[("short-actions", &["a"]), ("long-actions", &["dt"])],
                ),
                action(
                    "dt",
                    "double-tap",
                    &[("single-actions", &["b"]), ("double-actions", &["c"])],
                ),
                action("a", "map-to-vjoy", &[]),
                action("b", "map-to-vjoy", &[]),
                action("c", "map-to-vjoy", &[]),
            ],
            inputs: vec![InputBinding {
                device_id: "dev".into(),
                input_type: "button".into(),
                mode: "Default".into(),
                input_id: "1".into(),
                configs: vec![ActionConfig {
                    root_action: "root".into(),
                    behavior: "button".into(),
                    virtual_button: None,
                    layer: Layer::parse("M1"),
                    additive: false,
                    keep_value: false,
                    send_state: false,
                    extra: Vec::new(),
                }],
                extra: Vec::new(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn routes_through_branches() {
        let p = profile();
        let r = p.route(0, "a").unwrap();
        assert_eq!(r.mode, "Default");
        assert_eq!(r.layer, Layer::parse("M1"));
        assert_eq!(r.branches, vec![("tempo".into(), "short-actions".into())]);
        let r = p.route(0, "c").unwrap();
        assert_eq!(
            r.branches,
            vec![
                ("tempo".into(), "long-actions".into()),
                ("double-tap".into(), "double-actions".into())
            ]
        );
        assert!(p.route(0, "nope").is_none());
        assert!(p.route(1, "a").is_none());
    }
}
