//! Turning a profile's actions into the engine's nodes and binding them to inputs.

use super::*;

impl Engine {
    /// Attached device for a profile device GUID.
    pub(super) fn device(&self, guid: &str) -> Arc<str> {
        let guid = guid.trim().to_ascii_lowercase();
        self.aliases.get(&guid).cloned().unwrap_or(guid).into()
    }

    /// Child modes use their parent's binding (the very same one) unless they override it.
    pub(super) fn inherit_modes(&mut self, profile: &Profile) {
        let names: Vec<&str> = profile.modes.iter().map(|m| m.name.as_str()).collect();
        let mut queue: VecDeque<&str> = profile
            .modes
            .iter()
            .filter(|m| m.parent.as_deref().is_none_or(|p| !names.contains(&p)))
            .map(|m| m.name.as_str())
            .collect();
        while let Some(parent) = queue.pop_front() {
            for child in profile
                .modes
                .iter()
                .filter(|m| m.parent.as_deref() == Some(parent))
            {
                for modes in self.lookup.values_mut() {
                    if let Some(&set) = modes.get(parent) {
                        modes.entry(child.name.clone()).or_insert(set);
                    }
                }
                queue.push_back(&child.name);
            }
        }
    }

    pub(super) fn compile(
        &mut self,
        index: &std::collections::HashMap<&str, &Action>,
        id: &str,
        depth: usize,
    ) -> NodeId {
        let pruned = self.prune.as_deref().is_some_and(|target| {
            index
                .get(id)
                .is_some_and(|a| a.id != target && !is_container(a))
        });
        let node = match index.get(id).filter(|_| depth < MAX_DEPTH && !pruned) {
            None => Node::Noop,
            Some(action) => match action.kind() {
                ActionKind::Root { children } => Node::Seq(
                    children
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                ),
                ActionKind::MapToVjoy {
                    device,
                    input_type,
                    input_id,
                    inverted,
                } => {
                    let id = input_id.min(255) as u8;
                    let target = match input_type {
                        InputType::Axis => Some(VjoyTarget::Axis(id)),
                        InputType::Button => Some(VjoyTarget::Button(id)),
                        InputType::Hat => Some(VjoyTarget::Hat(id)),
                        InputType::Other => None,
                    };
                    let relative = (action.str("axis-mode") == Some("relative")).then(|| {
                        action
                            .str("axis-scaling")
                            .and_then(|s| s.trim().parse().ok())
                            .unwrap_or(1.0)
                    });
                    target.map_or(Node::Noop, |target| Node::Vjoy {
                        device,
                        target,
                        inverted,
                        act: action.activation_mode(),
                        relative,
                        send: action.button_send(),
                    })
                }
                ActionKind::MapToGamepad {
                    gamepad,
                    control,
                    inverted,
                } => GamepadControl::parse(control).map_or(Node::Noop, |control| Node::Gamepad {
                    gamepad,
                    control,
                    inverted,
                    act: action.activation_mode(),
                    send: action.button_send(),
                }),
                ActionKind::ChangeMode { change_type, .. } => Node::ChangeMode {
                    kind: match change_type {
                        "Previous" => ChangeKind::Previous,
                        "Unwind" => ChangeKind::Unwind,
                        "Cycle" => ChangeKind::Cycle,
                        "Temporary" => ChangeKind::Temporary,
                        _ => ChangeKind::Switch,
                    },
                    targets: target_modes(action),
                    act: action.activation_mode(),
                },
                ActionKind::ResponseCurve {
                    curve_type,
                    deadzone,
                    points,
                } => Node::Curve {
                    deadzone: deadzone.map(|d| d as f32),
                    curve: gob_core::curve::Curve::new(
                        gob_core::curve::CurveType::parse(curve_type),
                        &points,
                    ),
                },
                ActionKind::Tempo {
                    short,
                    long,
                    threshold,
                    activate_on,
                } => Node::Tempo {
                    short: short
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    long: long
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    threshold: Duration::from_secs_f64(threshold.max(0.0)),
                    on_press: activate_on == "press",
                },
                ActionKind::Chain { chains, timeout } => Node::Chain {
                    chains: chains
                        .iter()
                        .map(|c| {
                            c.iter()
                                .map(|a| self.compile(index, a, depth + 1))
                                .collect()
                        })
                        .collect(),
                    timeout: Duration::from_secs_f64(timeout.max(0.0)),
                },
                ActionKind::DoubleTap {
                    single,
                    double,
                    threshold,
                    activate_on,
                } => Node::DoubleTap {
                    single: single
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    double: double
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    threshold: Duration::from_secs_f64(threshold.max(0.0)),
                    exclusive: activate_on != "combined",
                },
                ActionKind::HatButtons { directions } => Node::HatButtons(
                    directions
                        .into_iter()
                        .map(|(_, d, ids)| {
                            (
                                d,
                                ids.iter()
                                    .map(|c| self.compile(index, c, depth + 1))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
                ActionKind::MapToLogicalDevice {
                    input_type,
                    id,
                    inverted,
                    relative,
                } => {
                    let kind = match input_type {
                        InputType::Axis => Some(InputKind::Axis),
                        InputType::Button => Some(InputKind::Button),
                        InputType::Hat => Some(InputKind::Hat),
                        InputType::Other => None,
                    };
                    kind.map_or(Node::Noop, |kind| Node::Logical {
                        key: InputKey {
                            device: gob_core::LOGICAL_DEVICE_GUID.into(),
                            kind,
                            id,
                        },
                        inverted,
                        relative: relative.map(|r| r as f32),
                        act: action.activation_mode(),
                    })
                }
                ActionKind::LoadProfile { file } => Node::LoadProfile {
                    file: file.to_string(),
                    act: action.activation_mode(),
                },
                ActionKind::SendOsc {
                    host,
                    port,
                    address,
                    args,
                } => Node::SendOsc {
                    host: host.to_string(),
                    port,
                    // OSC addresses start with `/`: one typed without it is sent with it.
                    address: gob_core::osc_address(address).unwrap_or_default(),
                    args: args.iter().map(osc_out).collect(),
                    act: action.activation_mode(),
                },
                ActionKind::PlaySound { file, volume } => Node::PlaySound {
                    file: file.to_string(),
                    volume,
                    act: action.activation_mode(),
                },
                ActionKind::LightLed {
                    device,
                    led,
                    element,
                    color,
                    color2,
                    blend,
                    mode,
                    on_activate,
                } => {
                    let light = Light {
                        device: self.device(device).to_string(),
                        led,
                        element,
                        color,
                        color2,
                        blend,
                        mode,
                    };
                    if light.device.is_empty() {
                        Node::Noop
                    } else if on_activate {
                        self.compiled_lights.push(light);
                        Node::Noop
                    } else {
                        Node::LightLed {
                            light,
                            act: action.activation_mode(),
                        }
                    }
                }
                ActionKind::TextToSpeech {
                    text,
                    voice,
                    queue_mode,
                    rate,
                    volume,
                    pitch,
                } => Node::Speak {
                    request: SpeechRequest {
                        text: text.to_string(),
                        voice: voice.map(String::from),
                        rate: rate as f32,
                        volume: volume as f32,
                        pitch: pitch as f32,
                        queue: SpeechQueue::parse(queue_mode),
                    },
                    act: action.activation_mode(),
                },
                ActionKind::Condition {
                    all,
                    conditions,
                    true_actions,
                    false_actions,
                } => Node::Condition {
                    all,
                    conditions: conditions
                        .into_iter()
                        .map(|c| Cond {
                            source: match c.source {
                                ConditionSource::CurrentInput => CondSource::Current,
                                ConditionSource::Joystick(inputs) => CondSource::Inputs(
                                    inputs
                                        .into_iter()
                                        .filter_map(|(device, ty, id)| {
                                            let kind = match ty {
                                                InputType::Axis => InputKind::Axis,
                                                InputType::Button => InputKind::Button,
                                                InputType::Hat => InputKind::Hat,
                                                InputType::Other => return None,
                                            };
                                            Some(InputKey {
                                                device: self.device(&device),
                                                kind,
                                                id,
                                            })
                                        })
                                        .collect(),
                                ),
                                ConditionSource::Keyboard(keys) => CondSource::Keys(keys),
                                ConditionSource::Vjoy {
                                    device,
                                    input_type,
                                    id,
                                } => match input_type {
                                    InputType::Axis => {
                                        CondSource::Vjoy(device, InputKind::Axis, id)
                                    }
                                    InputType::Hat => CondSource::Vjoy(device, InputKind::Hat, id),
                                    _ => CondSource::Vjoy(device, InputKind::Button, id),
                                },
                                ConditionSource::Logical { input_type, id } => {
                                    let kind = match input_type {
                                        InputType::Axis => InputKind::Axis,
                                        InputType::Hat => InputKind::Hat,
                                        _ => InputKind::Button,
                                    };
                                    CondSource::Inputs(vec![InputKey {
                                        device: gob_core::LOGICAL_DEVICE_GUID.into(),
                                        kind,
                                        id,
                                    }])
                                }
                                ConditionSource::Other(_) => CondSource::Never,
                            },
                            comparator: c.comparator,
                        })
                        .collect(),
                    branches: [true_actions, false_actions].map(|ids| {
                        ids.iter()
                            .map(|c| self.compile(index, c, depth + 1))
                            .collect()
                    }),
                    act: action.activation_mode(),
                },
                ActionKind::DualAxisDeadzone {
                    axis1,
                    axis2,
                    inner,
                    outer,
                    first,
                    second,
                } => Node::DualDeadzone {
                    axes: [axis1, axis2].map(|(device, id)| InputKey {
                        device: self.device(device),
                        kind: InputKind::Axis,
                        id,
                    }),
                    inner: inner as f32,
                    outer: outer as f32,
                    outputs: [first, second].map(|ids| {
                        ids.iter()
                            .map(|c| self.compile(index, c, depth + 1))
                            .collect()
                    }),
                },
                ActionKind::AxisDelta {
                    threshold,
                    positive,
                    negative,
                } => Node::AxisDelta {
                    threshold: threshold.max(0.0001) as f32,
                    positive: positive
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    negative: negative
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                },
                ActionKind::MergeAxis {
                    axis1,
                    axis2,
                    operation,
                    children,
                } => Node::Merge {
                    axes: [axis1, axis2].map(|(device, id)| InputKey {
                        device: self.device(device),
                        kind: InputKind::Axis,
                        id,
                    }),
                    op: operation,
                    children: children
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                },
                ActionKind::MapToKeyboard { keys } => Node::Keys {
                    keys,
                    act: action.activation_mode(),
                    send: action.button_send(),
                },
                ActionKind::MapToMouse(action_kind) => Node::Mouse {
                    action: action_kind,
                    act: action.activation_mode(),
                    send: action.button_send(),
                },
                ActionKind::Delay {
                    children,
                    seconds,
                    while_held,
                } => Node::Delay {
                    children: children
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    delay: Duration::from_secs_f64(seconds.clamp(0.0, 600.0)),
                    while_held,
                },
                ActionKind::SmartToggle { children, delay } => Node::SmartToggle {
                    children: children
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    delay: Duration::from_secs_f64(delay.max(0.0)),
                },
                ActionKind::SplitAxis {
                    split,
                    lower,
                    upper,
                } => Node::Split {
                    split: split as f32,
                    lower: lower
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                    upper: upper
                        .iter()
                        .map(|c| self.compile(index, c, depth + 1))
                        .collect(),
                },
                ActionKind::AxisZones { points, zones } => Node::Zones {
                    zones: zones
                        .iter()
                        .map(|z| {
                            let children = z
                                .children
                                .iter()
                                .map(|c| self.compile(index, c, depth + 1))
                                .collect();
                            (z.output, children)
                        })
                        .collect(),
                    points,
                },
                ActionKind::PauseResume { operation } => Node::Pause(match operation.trim() {
                    "Resume" => PauseOp::Resume,
                    "Toggle" => PauseOp::Toggle,
                    _ => PauseOp::Pause,
                }),
                ActionKind::RunCommand {
                    executable,
                    arguments,
                } => Node::Run {
                    executable: executable.to_string(),
                    arguments: split_command(arguments),
                    act: action.activation_mode(),
                },
                ActionKind::Macro { steps, repeat, .. } => Node::Macro {
                    steps: with_default_delay(&steps, self.macro_delay),
                    repeat,
                    act: action.activation_mode(),
                    targets: self.run_targets.get(id).cloned().unwrap_or_default(),
                },
                ActionKind::VoiceInput { microphone } => Node::Voice {
                    microphone: microphone.to_string(),
                    act: action.activation_mode(),
                },
                ActionKind::ButtonAxis(axis) => self.button_axis_node(axis, &action.id),
                ActionKind::Other | ActionKind::Description { .. } => Node::Noop,
            },
        };
        self.nodes.push(Arc::new(node));
        let node = self.nodes.len() - 1;
        if self.prune.is_none() && index.contains_key(id) && !self.action_nodes.contains_key(id) {
            self.action_nodes.insert(id.to_string(), node);
        }
        node
    }

    /// button-axis: its node, the decrease one while compiling the decrease input's tree;
    /// otherwise the decrease input goes to `compiled_decrease`.
    pub(super) fn button_axis_node(&mut self, axis: ButtonAxis, id: &str) -> Node {
        let target = match &axis.output {
            AxisOutput::Vjoy { device, axis } => AxisOut::Vjoy(*device, (*axis).min(255) as u8),
            AxisOutput::Gamepad { gamepad, control } => match GamepadControl::parse(control) {
                Some(c) if matches!(c.kind(), ControlKind::Stick | ControlKind::Trigger) => {
                    AxisOut::Pad(*gamepad, c)
                }
                _ => return Node::Noop,
            },
            AxisOutput::Logical { id } => AxisOut::Logical(InputKey {
                device: gob_core::LOGICAL_DEVICE_GUID.into(),
                kind: InputKind::Axis,
                id: *id,
            }),
        };
        // Compiling the decrease input's tree: this is the node it reaches.
        if self.prune.as_deref() == Some(id) {
            return Node::ButtonAxis {
                axis,
                target,
                up: false,
            };
        }
        if self.prune.is_none()
            && let Some(input) = axis.decrease.clone()
            && matches!(input.input_type.as_str(), "button" | "key")
        {
            self.compiled_decrease.push((input, id.to_string()));
        }
        Node::ButtonAxis {
            axis,
            target,
            up: true,
        }
    }
}
