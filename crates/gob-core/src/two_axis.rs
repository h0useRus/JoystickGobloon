//! Actions that read two axes (merge-axis, dual-axis-deadzone). Like Gremlin, one library
//! action is bound to both axes it reads, so a change of either axis runs it. Changing one of
//! its axes moves the binding along, so the action stays bound to just the axes it reads.

use crate::{Item, Layer, Profile};

/// Action kinds that read two axes.
pub const TWO_AXIS_KINDS: [&str; 2] = ["merge-axis", "dual-axis-deadzone"];

/// An axis input: (device GUID, axis id).
pub type AxisInput = (String, u32);

fn same(a: &AxisInput, b: &AxisInput) -> bool {
    a.0.eq_ignore_ascii_case(&b.0) && a.1 == b.1
}

impl Profile {
    /// The two axes a two-axis action reads; `None` for other actions.
    pub fn two_axis_inputs(&self, id: &str) -> Option<[AxisInput; 2]> {
        let a = self.action(id)?;
        if !TWO_AXIS_KINDS.contains(&a.kind.as_str()) {
            return None;
        }
        let slot = |n: u8| -> Option<AxisInput> {
            let device = a.str(&format!("axis{n}-guid"))?.trim().to_string();
            let axis = a.str(&format!("axis{n}-axis"))?.trim().parse().ok()?;
            Some((device, axis))
        };
        Some([slot(1)?, slot(2)?])
    }

    /// Whether two-axis action `id` may be bound to `input` too: it reads that axis already,
    /// or it is new (both slots on one axis) and takes it as its second one. Other actions:
    /// always.
    pub fn two_axis_accepts(&self, id: &str, input: &AxisInput) -> bool {
        self.two_axis_inputs(id)
            .is_none_or(|[a, b]| same(&a, input) || same(&b, input) || same(&a, &b))
    }

    /// Two-axis action `id` is being bound to `input`: a new action (both slots on one axis)
    /// takes it as its second axis. Returns a log line when it did.
    pub fn two_axis_adopt(&mut self, id: &str, input: &AxisInput) -> Option<String> {
        let [a, b] = self.two_axis_inputs(id)?;
        if same(&a, input) || !same(&a, &b) {
            return None;
        }
        self.set_slot(id, 2, input);
        Some(format!(
            "{}: axis 2 set to {} axis {}, the input it was linked to",
            self.describe(id),
            input.0,
            input.1
        ))
    }

    /// Sets axis `slot` (1 or 2) of two-axis action `id` to `input` and moves its binding
    /// along in `mode`: bound to the new axis (on `layer`), unbound from the old one unless
    /// the other slot still reads it. Returns what was done, for the log.
    pub fn set_two_axis_input(
        &mut self,
        id: &str,
        slot: u8,
        input: &AxisInput,
        mode: &str,
        layer: &Layer,
    ) -> Vec<String> {
        let Some(before) = self.two_axis_inputs(id) else {
            return Vec::new();
        };
        let i = usize::from(slot.clamp(1, 2) - 1);
        let (old, other) = (before[i].clone(), before[1 - i].clone());
        self.set_slot(id, slot, input);
        let name = self.describe(id);
        let mut log = Vec::new();
        // Bound to the new axis first, so the action is never left unreferenced.
        if self.link_axis(id, input, mode, layer) {
            log.push(format!(
                "{name}: linked to {} axis {} ({mode})",
                input.0, input.1
            ));
        }
        if !same(&old, input) && !same(&old, &other) && self.unlink_axis(id, &old, mode) {
            log.push(format!(
                "{name}: unlinked from {} axis {} ({mode})",
                old.0, old.1
            ));
        }
        log
    }

    fn set_slot(&mut self, id: &str, slot: u8, input: &AxisInput) {
        if let Some(a) = self.action_mut(id) {
            a.set(&format!("axis{slot}-guid"), "uuid", input.0.clone());
            a.set(&format!("axis{slot}-axis"), "int", input.1.to_string());
        }
    }

    /// The action's kind and label for the log.
    fn describe(&self, id: &str) -> String {
        let Some(a) = self.action(id) else {
            return id.to_string();
        };
        match a.str("label").map(str::trim).filter(|l| !l.is_empty()) {
            Some(label) => format!("{} \"{label}\"", a.kind),
            None => format!("{} {}", a.kind, &a.id[..a.id.len().min(8)]),
        }
    }

    /// Puts action `id` at the top of `input`'s `layer` in `mode` unless one of the input's
    /// layers there has it already; true when it was added.
    pub fn link_axis(&mut self, id: &str, input: &AxisInput, mode: &str, layer: &Layer) -> bool {
        let has = self.axis_roots(input, mode).iter().any(|(root, _)| {
            self.action(root)
                .is_some_and(|r| r.children("actions").iter().any(|c| c == id))
        });
        if has {
            return false;
        }
        let root = self.bind_layer(&input.0, "axis", &input.1.to_string(), mode, layer);
        self.add_ref(&root, "actions", id)
    }

    /// Takes action `id` off the top of every layer of `input` in `mode`; a layer left
    /// empty is unbound. True when it was there.
    pub fn unlink_axis(&mut self, id: &str, input: &AxisInput, mode: &str) -> bool {
        let mut removed = false;
        for (root, layer) in self.axis_roots(input, mode) {
            let Some(r) = self.action_mut(&root) else {
                continue;
            };
            let refs = r.group_mut("actions");
            let before = refs.len();
            refs.retain(|item| !matches!(item, Item::ActionRef(x) if x == id));
            if refs.len() == before {
                continue;
            }
            removed = true;
            if r.children("actions").is_empty() {
                self.unbind_layer(&input.0, "axis", &input.1.to_string(), mode, &layer);
            }
        }
        if removed {
            self.collect_garbage();
        }
        removed
    }

    /// Root actions of `input`'s layers in `mode`.
    fn axis_roots(&self, input: &AxisInput, mode: &str) -> Vec<(String, Layer)> {
        let axis = input.1.to_string();
        self.inputs
            .iter()
            .filter(|b| {
                b.device_id.eq_ignore_ascii_case(&input.0)
                    && b.input_type == "axis"
                    && b.input_id == axis
                    && b.mode == mode
            })
            .flat_map(|b| &b.configs)
            .map(|c| (c.root_action.clone(), c.layer.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, NewAction};

    const A: &str = "{DEV-A}";
    const B: &str = "{DEV-B}";

    /// A new merge-axis on axis 1 of device A, as the add menu makes it.
    fn merge_on_a1() -> (Profile, String) {
        let mut p = Profile::default();
        let root = p.bind(A, "axis", "1", "Default");
        let action = Action::new(&NewAction::MergeAxis {
            device: A.into(),
            axis: 1,
        });
        let id = p.add_child(&root, "actions", action).unwrap();
        (p, id)
    }

    fn linked(p: &Profile, id: &str) -> Vec<AxisInput> {
        let mut v: Vec<AxisInput> = p
            .inputs
            .iter()
            .filter(|b| {
                b.configs.iter().any(|c| {
                    p.action(&c.root_action)
                        .is_some_and(|r| r.children("actions").contains(&id.to_string()))
                })
            })
            .map(|b| (b.device_id.clone(), b.input_id.parse().unwrap()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn picking_an_axis_moves_the_binding() {
        let (mut p, id) = merge_on_a1();
        let base = Layer::base();
        // Second axis picked: bound there too.
        let log = p.set_two_axis_input(&id, 2, &(B.into(), 3), "Default", &base);
        assert_eq!(log.len(), 1, "{log:?}");
        assert_eq!(linked(&p, &id), [(A.into(), 1), (B.into(), 3)]);
        // Second axis changed again: moved from B 3 to B 4, B 3 left unbound.
        let log = p.set_two_axis_input(&id, 2, &(B.into(), 4), "Default", &base);
        assert_eq!(log.len(), 2, "{log:?}");
        assert_eq!(linked(&p, &id), [(A.into(), 1), (B.into(), 4)]);
        assert!(!p.inputs.iter().any(|b| b.input_id == "3"));
        // First axis moved onto the second one: bound once, A 1 left.
        p.set_two_axis_input(&id, 1, &(B.into(), 4), "Default", &base);
        assert_eq!(linked(&p, &id), [(B.into(), 4)]);
        assert!(p.action(&id).is_some(), "still in the library");
        assert_eq!(
            p.two_axis_inputs(&id).unwrap(),
            [(B.into(), 4), (B.into(), 4)]
        );
    }

    #[test]
    fn linking_adopts_a_second_axis_and_refuses_a_third() {
        let (mut p, id) = merge_on_a1();
        let b2: AxisInput = (B.into(), 2);
        assert!(p.two_axis_accepts(&id, &b2), "new: takes it");
        assert!(p.two_axis_adopt(&id, &b2).is_some());
        assert_eq!(p.two_axis_inputs(&id).unwrap()[1], b2);
        assert!(p.two_axis_adopt(&id, &b2).is_none(), "already its axis");
        assert!(p.two_axis_accepts(&id, &b2));
        assert!(p.two_axis_accepts(&id, &(A.to_lowercase(), 1)));
        assert!(!p.two_axis_accepts(&id, &(B.into(), 5)), "a third axis");
        assert!(p.two_axis_adopt(&id, &(B.into(), 5)).is_none());
    }

    #[test]
    fn other_layers_and_actions_stay() {
        let (mut p, id) = merge_on_a1();
        // A 1 has another action next to the merge: its binding stays when the merge leaves.
        let root = p.bind(A, "axis", "1", "Default");
        p.add_child(&root, "actions", Action::new(&NewAction::Description));
        p.set_two_axis_input(&id, 2, &(B.into(), 2), "Default", &Layer::base());
        p.set_two_axis_input(&id, 1, &(B.into(), 1), "Default", &Layer::base());
        assert_eq!(linked(&p, &id), [(B.into(), 1), (B.into(), 2)]);
        assert_eq!(p.action(&root).unwrap().children("actions").len(), 1);
    }
}
