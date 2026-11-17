//! What the page shows: controllers, the profile's outputs and the game's actions, how they
//! connect, and what is lit now.

use super::state::State;
use super::*;

pub(super) struct InputNode {
    /// (profile device id, input type, input id).
    pub(super) key: (String, String, String),
    pub(super) label: String,
    pub(super) axis: bool,
}

pub(super) struct ActionNode {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) category: String,
    /// Bound to an output of the profile.
    pub(super) bound: bool,
    /// In the catalog of the game version.
    pub(super) known: bool,
}

/// Columns and wires of the schematic.
pub(super) struct Graph {
    pub(super) inputs: Vec<InputNode>,
    pub(super) outputs: Vec<GameOutput>,
    pub(super) actions: Vec<ActionNode>,
    /// (input, output) indices.
    pub(super) input_edges: Vec<(usize, usize)>,
    /// (output, action) indices.
    pub(super) action_edges: Vec<(usize, usize)>,
}

impl Graph {
    /// `all`: every action of the catalog, not only the wired ones and a search's first finds.
    pub(super) fn build(
        ctx: &Context,
        i18n: &I18n,
        catalog: &Catalog,
        search: &str,
        all: bool,
        state: &State,
    ) -> Self {
        let profile = ctx.profile;
        // Outputs: what the profile writes (hats as their directions), the keys and mouse
        // buttons it sends, and what it binds without writing it.
        let mut outputs: Vec<GameOutput> = Vec::new();
        let mut sources: HashMap<GameOutput, Vec<usize>> = HashMap::new();
        for (v, uses) in &profile.output_usage(None) {
            let list: Vec<GameOutput> = match v {
                VirtualOutput::Vjoy {
                    input_type: InputType::Hat,
                    ..
                } => HatDirection::ALL
                    .iter()
                    .map(|d| GameOutput::HatDirection(v.clone(), *d))
                    .collect(),
                VirtualOutput::Gamepad { control, .. } if control == "dpad" => HatDirection::ALL
                    .iter()
                    .map(|d| GameOutput::HatDirection(v.clone(), *d))
                    .collect(),
                _ => vec![GameOutput::Virtual(v.clone())],
            };
            for o in list {
                sources.insert(o.clone(), uses.iter().map(|u| u.binding).collect());
                outputs.push(o);
            }
        }
        for (combo, uses) in profile.key_usage(None) {
            let o = GameOutput::Keys(combo);
            sources.insert(o.clone(), uses.iter().map(|u| u.binding).collect());
            outputs.push(o);
        }
        let node = |id: &str| {
            let a = catalog.action(id);
            ActionNode {
                id: id.to_string(),
                name: a.map_or_else(|| id.to_string(), |a| a.name.clone()),
                category: a.map_or_else(String::new, |a| a.category.clone()),
                bound: state.of(id).iter().any(|b| b.mark.profile()),
                known: a.is_some(),
            }
        };
        // Actions: those the outputs drive, and the catalog's matches of a search.
        let mut actions: Vec<ActionNode> = Vec::new();
        let listed: std::collections::HashSet<&GameOutput> = outputs.iter().collect();
        let mut extra: Vec<GameOutput> = Vec::new();
        for (id, bindings) in state.all() {
            let held = bindings.iter().any(|b| b.mark.profile());
            let wired = bindings
                .iter()
                .any(|b| b.mark.in_effect() && listed.contains(b.output));
            if held || wired {
                actions.push(node(id));
            }
            for b in bindings.iter().filter(|b| b.mark.profile()) {
                if !listed.contains(b.output) && !extra.contains(b.output) {
                    extra.push(b.output.clone());
                }
            }
        }
        outputs.extend(extra);
        outputs.sort_by_key(sort_key);
        // Inputs: the bindings writing those outputs.
        let mut inputs: Vec<InputNode> = Vec::new();
        let mut input_edges = Vec::new();
        for (m, o) in outputs.iter().enumerate() {
            for &b in sources.get(o).into_iter().flatten() {
                let Some(binding) = profile.inputs.get(b) else {
                    continue;
                };
                let key = (
                    binding.device_id.clone(),
                    binding.input_type.clone(),
                    binding.input_id.clone(),
                );
                let l = match inputs.iter().position(|n| n.key == key) {
                    Some(l) => l,
                    None => {
                        inputs.push(InputNode {
                            label: input_label(i18n, profile, &key),
                            axis: binding.input_type() == InputType::Axis,
                            key,
                        });
                        inputs.len() - 1
                    }
                };
                if !input_edges.contains(&(l, m)) {
                    input_edges.push((l, m));
                }
            }
        }
        let search = search.trim().to_lowercase();
        if all || !search.is_empty() {
            actions.retain(|a| matches(a, &search));
            let have: std::collections::HashSet<&str> =
                actions.iter().map(|a| a.id.as_str()).collect();
            let more: Vec<ActionNode> = catalog
                .actions
                .iter()
                .filter(|a| !have.contains(a.id.as_str()))
                .map(|a| node(&a.id))
                .filter(|a| matches(a, &search))
                .take(if all { usize::MAX } else { SEARCH_LIMIT })
                .collect();
            actions.extend(more);
        }
        actions.sort_by(|a, b| (&a.category, &a.name, &a.id).cmp(&(&b.category, &b.name, &b.id)));
        let at: HashMap<&GameOutput, usize> =
            outputs.iter().enumerate().map(|(m, o)| (o, m)).collect();
        let mut action_edges = Vec::new();
        for (r, a) in actions.iter().enumerate() {
            for b in state.of(&a.id).iter().filter(|b| b.mark.in_effect()) {
                if let Some(m) = at.get(b.output) {
                    action_edges.push((*m, r));
                }
            }
        }
        Self {
            inputs,
            outputs,
            actions,
            input_edges,
            action_edges,
        }
    }
}

pub(super) fn matches(a: &ActionNode, search: &str) -> bool {
    a.name.to_lowercase().contains(search)
        || a.id.to_lowercase().contains(search)
        || a.category.to_lowercase().contains(search)
}

pub(super) fn sort_key(o: &GameOutput) -> (u8, u32, u8, u32, u8, String) {
    let t = |t: &InputType| match t {
        InputType::Axis => 0,
        InputType::Button => 1,
        _ => 2,
    };
    match o {
        GameOutput::Virtual(VirtualOutput::Vjoy {
            device,
            input_type,
            id,
        }) => (0, *device, t(input_type), *id, 0, String::new()),
        GameOutput::HatDirection(VirtualOutput::Vjoy { device, id, .. }, d) => {
            (0, *device, 2, *id, *d as u8, String::new())
        }
        GameOutput::Keys(c) => (3, 0, 0, 0, 0, c.to_string()),
        other => (2, 0, 0, 0, 0, other.to_string()),
    }
}

pub(super) fn input_label(
    i18n: &I18n,
    profile: &Profile,
    (device, kind, id): &(String, String, String),
) -> String {
    let name = crate::devices_view::device_name(i18n, profile, device);
    format!(
        "{name} · {}",
        crate::output_marks::input_name(i18n, kind, id)
    )
}

/// How lit each node is now, 0 to 1.
pub(super) struct Live {
    pub(super) inputs: Vec<f32>,
    pub(super) outputs: Vec<f32>,
    pub(super) actions: Vec<f32>,
    pub(super) any: bool,
}

impl Live {
    pub(super) fn read(ctx: &Context, graph: &Graph) -> Self {
        let aliases = crate::substitute_view::auto_aliases(ctx.profile, ctx.devices);
        let state = |device: &str| {
            let id = aliases.get(device).map_or(device, String::as_str);
            ctx.devices
                .iter()
                .find(|d| d.info.guid.eq_ignore_ascii_case(id))
        };
        // Per input: lit amount, and the hat angle for hat inputs.
        let mut hats: Vec<Option<u16>> = Vec::with_capacity(graph.inputs.len());
        let inputs: Vec<f32> = graph
            .inputs
            .iter()
            .map(|n| {
                let (device, kind, id) = &n.key;
                let index = id.trim().parse::<usize>().unwrap_or(0).wrapping_sub(1);
                let s = state(device);
                let mut hat = None;
                let v = match kind.as_str() {
                    "button" => s
                        .and_then(|s| s.buttons.get(index).copied())
                        .map_or(0.0, |b| if b { 1.0 } else { 0.0 }),
                    // Lit past a dead zone, so a resting axis a little off center stays dark.
                    "axis" => s
                        .and_then(|s| s.axes.get(index).copied())
                        .map_or(0.0, |v| ((v.abs() - 0.15) / 0.5).clamp(0.0, 1.0)),
                    "hat" => {
                        hat = s.and_then(|s| s.hats.get(index).copied()).flatten();
                        if hat.is_some() { 1.0 } else { 0.0 }
                    }
                    _ => 0.0,
                };
                hats.push(hat);
                v
            })
            .collect();
        let mut outputs = vec![0.0f32; graph.outputs.len()];
        // A running profile says what it really sends: modes, conditions and macros count.
        // Outputs it cannot tell (the logical device) and a stopped profile go by the inputs.
        let mut told = vec![false; graph.outputs.len()];
        if let Some(sent) = &ctx.sent {
            let now = std::time::Instant::now();
            sent.read(|map| {
                for (m, o) in graph.outputs.iter().enumerate() {
                    if let Some(lit) = sent_lit(map, o, now) {
                        told[m] = true;
                        outputs[m] = lit;
                    }
                }
            });
        }
        for (l, m) in &graph.input_edges {
            if told[*m] {
                continue;
            }
            let mut v = inputs[*l];
            if let GameOutput::HatDirection(_, d) = &graph.outputs[*m] {
                v = match hats[*l] {
                    Some(angle) if hat_has(angle, *d) => 1.0,
                    _ => 0.0,
                };
            }
            outputs[*m] = outputs[*m].max(v);
        }
        let mut actions = vec![0.0f32; graph.actions.len()];
        for (m, r) in &graph.action_edges {
            actions[*r] = actions[*r].max(outputs[*m]);
        }
        let any = inputs.iter().chain(&outputs).any(|v| *v > 0.0);
        Self {
            inputs,
            outputs,
            actions,
            any,
        }
    }
}

/// How lit `o` is in what the running profile sent (`map`, the [`gob_engine::OutputTap`]);
/// `None` for outputs the tap does not carry. A hat direction is lit while the hat points
/// there; keys and mouse buttons pressed together are lit while all of them are.
pub(super) fn sent_lit(
    map: &HashMap<gob_engine::Sent, gob_engine::Tapped>,
    o: &GameOutput,
    now: std::time::Instant,
) -> Option<f32> {
    use gob_engine::{InputKind, Sent};
    let of = |v: &VirtualOutput| match v {
        VirtualOutput::Vjoy {
            device,
            input_type,
            id,
        } => {
            let kind = match input_type {
                InputType::Axis => InputKind::Axis,
                InputType::Button => InputKind::Button,
                InputType::Hat => InputKind::Hat,
                _ => return None,
            };
            Some(Sent::Joystick(*device, kind, *id))
        }
        VirtualOutput::Gamepad { gamepad, control } => gob_engine::GamepadControl::ALL
            .into_iter()
            .find(|c| c.name() == control)
            .map(|c| Sent::Gamepad(*gamepad, c)),
        VirtualOutput::Logical { .. } => None,
    };
    let lit = |key: Sent| map.get(&key).map_or(0.0, |t| t.lit(now));
    match o {
        GameOutput::Virtual(v) => of(v).map(lit),
        GameOutput::HatDirection(v, d) => of(v).map(|key| match map.get(&key).map(|t| t.value) {
            Some(gob_engine::Value::Hat(Some(angle))) if hat_has(angle, *d) => 1.0,
            _ => 0.0,
        }),
        GameOutput::Keys(c) => Some(
            c.parts()
                .iter()
                .map(|p| match p {
                    gob_core::ComboPart::Key(k) => lit(Sent::Key(k.scan_code, k.extended)),
                    gob_core::ComboPart::Mouse(b) => lit(Sent::Mouse(*b)),
                })
                .fold(1.0, f32::min),
        ),
    }
}

/// Whether a hat at `angle` (hundredths of a degree, 0 = up, clockwise) points `d`, diagonals
/// counting for both directions.
pub(super) fn hat_has(angle: u16, d: HatDirection) -> bool {
    let a = f32::from(angle) / 100.0;
    let center = match d {
        HatDirection::Up => 0.0,
        HatDirection::Right => 90.0,
        HatDirection::Down => 180.0,
        HatDirection::Left => 270.0,
    };
    let diff = ((a - center + 540.0) % 360.0 - 180.0).abs();
    diff < 67.5
}
