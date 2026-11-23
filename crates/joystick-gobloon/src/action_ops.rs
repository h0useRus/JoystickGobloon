//! Profile edits the action editor asks for (`Op`) and how they change the profile (`apply`):
//! no UI here, so the window applies them after drawing.

use crate::action_editor::{AddKind, py_float};

use gob_core::{
    Action, InputType, Item, Layer, NewAction, Profile, Property, VirtualButton, VirtualOutput,
};

/// Adds a bound device to the profile's `<devices>` (not the logical device or the keyboard).
fn list_device(p: &mut Profile, device: &str, name: String) {
    if !gob_core::is_builtin_device(device)
        && !p.devices.iter().any(|d| d.id.eq_ignore_ascii_case(device))
    {
        p.devices.push(gob_core::Device {
            id: device.to_string(),
            name,
        });
    }
}

fn add_to(p: &mut Profile, parent: &str, group: &str, kind: &AddKind) {
    match kind {
        // Id 0: a new logical input of that type.
        AddKind::New(NewAction::MapToLogicalDevice { input_type, id: 0 }) => {
            let id = p.add_logical_input(*input_type);
            let kind = NewAction::MapToLogicalDevice {
                input_type: *input_type,
                id,
            };
            p.add_child(parent, group, Action::new(&kind));
        }
        AddKind::New(kind) => {
            p.add_child(parent, group, Action::new(kind));
        }
        AddKind::Reuse(id) => {
            p.add_ref(parent, group, id);
        }
        AddKind::Duplicate(id) => {
            if let Some(copy) = p.duplicate(id) {
                p.add_ref(parent, group, &copy);
                p.separate_outputs(&copy);
            }
        }
        AddKind::Paste(clip) => {
            for copy in p.paste(&clip.actions, &clip.tops) {
                p.add_ref(parent, group, &copy);
                p.separate_outputs(&copy);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Op {
    /// Creates the binding (empty root), optionally with a first action.
    Bind {
        device: String,
        name: String,
        input_type: String,
        input_id: String,
        mode: String,
        /// Layer the actions go on (the base without modifiers).
        layer: Layer,
        add: Option<AddKind>,
    },
    /// Removes one layer of an input's binding (the binding when it was the last one).
    Unbind {
        device: String,
        input_type: String,
        input_id: String,
        mode: String,
        layer: Layer,
    },
    SetLayerAdditive {
        device: String,
        input_type: String,
        input_id: String,
        mode: String,
        layer: Layer,
        additive: bool,
    },
    SetLayerSendState {
        device: String,
        input_type: String,
        input_id: String,
        mode: String,
        layer: Layer,
        send: bool,
    },
    SetLayerKeepValue {
        device: String,
        input_type: String,
        input_id: String,
        mode: String,
        layer: Layer,
        keep: bool,
    },
    /// Makes the action sequence `root` of an axis or hat run as a button (`Some`) or as the
    /// input itself (`None`).
    SetVirtualButton {
        root: String,
        input_type: String,
        button: Option<VirtualButton>,
    },
    /// Adds an action sequence running as `button` to an input's layer, binding it if needed.
    AddButtonSequence {
        device: String,
        name: String,
        input_type: String,
        input_id: String,
        mode: String,
        layer: Layer,
        button: VirtualButton,
    },
    /// Removes one action sequence (the binding's layer with its last one).
    RemoveSequence(String),
    Add {
        parent: String,
        group: String,
        action: AddKind,
    },
    /// Sets axis `slot` of a two-axis action (merge-axis, dual-axis-deadzone) and moves its
    /// binding along: bound to the new axis, unbound from the old one (logged).
    TwoAxisInput {
        action: String,
        slot: u8,
        device: String,
        /// Device name for the profile's device list.
        name: String,
        axis: u32,
        mode: String,
        layer: Layer,
    },
    /// A two-axis action is being linked to `device`'s `axis`: a new one takes it as its
    /// second axis (logged).
    TwoAxisAdopt {
        action: String,
        device: String,
        axis: u32,
    },
    Remove {
        parent: String,
        group: String,
        index: usize,
    },
    /// Gives a shared child its own copy here (see [`gob_core::Profile::unlink_child`]).
    Unlink {
        parent: String,
        group: String,
        index: usize,
    },
    Move {
        parent: String,
        group: String,
        from: usize,
        to: usize,
    },
    Unset {
        action: String,
        name: String,
    },
    Set {
        action: String,
        name: String,
        ty: String,
        value: String,
    },
    /// Property inside a group, e.g. a deadzone limit.
    SetInGroup {
        action: String,
        group: String,
        name: String,
        ty: String,
        value: String,
    },
    SetPoints {
        action: String,
        points: Vec<(f32, f32)>,
    },
    SetTargets {
        action: String,
        targets: Vec<String>,
    },
    /// Replaces (or appends, at the end index) a condition of a condition action.
    SetCondition {
        action: String,
        index: usize,
        condition: gob_core::Condition,
    },
    RemoveCondition {
        action: String,
        index: usize,
    },
    /// Replaces (or appends, at the end index) a macro step.
    SetMacroStep {
        action: String,
        index: usize,
        step: gob_core::MacroStep,
    },
    RemoveMacroStep {
        action: String,
        index: usize,
    },
    /// Swaps step `index` with the next one.
    SwapMacroSteps {
        action: String,
        index: usize,
    },
    AddLogicalInput(InputType),
    RenameLogicalInput {
        input_type: InputType,
        id: u32,
        label: String,
    },
    RemoveLogicalInput {
        input_type: InputType,
        id: u32,
    },
    /// A voice command or group (`group`) into group `parent`.
    AddVoiceNode {
        parent: Option<u32>,
        group: bool,
    },
    /// Moves a voice command or group into another group (`None` = the top level).
    SetVoiceParent {
        id: u32,
        parent: Option<u32>,
    },
    DuplicateVoiceGroup(u32),
    /// Puts voice commands and groups into a new group made where the first of them is.
    GroupVoiceNodes(Vec<u32>),
    /// Copies voice commands and groups (without actions) into group `parent` (`None` = the top
    /// level).
    CopyVoiceNodes {
        ids: Vec<u32>,
        parent: Option<u32>,
    },
    /// Moves voice commands and groups, with their actions, into group `parent`.
    MoveVoiceNodes {
        ids: Vec<u32>,
        parent: Option<u32>,
    },
    /// Adds a combination of these keys (or finds it) and binds it in `mode`.
    AddCombo {
        keys: Vec<gob_core::Key>,
        mode: String,
    },
    SetComboKeys {
        id: u32,
        keys: Vec<gob_core::Key>,
    },
    RemoveCombo(u32),
    /// Adds an OSC input for `address` (or finds the one that has it).
    AddOscInput {
        address: String,
        kind: gob_core::OscKind,
    },
    SetOscAddress {
        id: u32,
        address: String,
    },
    SetOscKind {
        id: u32,
        kind: gob_core::OscKind,
    },
    SetOscRange {
        id: u32,
        low: f64,
        high: f64,
    },
    RemoveOscInput(u32),
    /// Replaces the arguments of a send-osc action.
    SetOscArgs {
        action: String,
        args: Vec<gob_core::OscArg>,
    },
    /// Makes a voice command saying `phrase` run `action` itself in `mode` and `layer` (the
    /// card's); `id` is the command it goes to, which the page then opens.
    VoiceFromAction {
        action: String,
        phrase: String,
        mode: String,
        id: u32,
    },
    /// Not an edit: the page opens voice command `id`.
    GoToVoice(u32),
    RemoveVoiceCommand(u32),
    SetVoicePhrases {
        id: u32,
        phrases: Vec<String>,
    },
    /// Seconds a recognized command holds its button.
    SetVoiceHold {
        id: u32,
        hold: f64,
    },
    /// A curve saved as a preset (replacing one of the same name), or a preset deleted; kept
    /// in the settings by the app.
    SaveCurvePreset(crate::settings::SavedCurve),
    DeleteCurvePreset(String),
    AddChainStep(String),
    RemoveChainStep {
        action: String,
        index: usize,
    },
    /// Axis zones: a point added at the top, one removed, one moved, a zone's output.
    AddZonePoint(String),
    RemoveZonePoint {
        action: String,
        index: usize,
    },
    SetZonePoint {
        action: String,
        index: usize,
        value: f64,
    },
    SetZoneOutput {
        action: String,
        zone: usize,
        output: gob_core::ZoneOutput,
    },
    /// Keys of a map-to-keyboard action.
    SetKeys {
        action: String,
        keys: Vec<gob_core::Key>,
    },
    /// Preview in the editor; not an edit.
    PreviewSound {
        file: String,
        volume: u8,
    },
    /// Preview in the editor; not an edit.
    PreviewSpeech(gob_engine::SpeechRequest),
    /// Lights a LED now ("light it now" in the light-led editor); not an edit. `device`
    /// is the profile's device GUID.
    PreviewLight {
        device: String,
        led: crate::leds::LedValue,
    },
    /// button-axis: the input that moves the axis down, `None` to remove it.
    SetDecrease {
        action: String,
        /// (device GUID, device name for the profile's device list, input type, id).
        input: Option<(String, String, String, u32)>,
    },
    /// button-axis: write a logical axis (the first one, a new one when there is none).
    ButtonAxisLogical(String),
    /// hat-buttons: 4 or 8 directions.
    HatButtonCount {
        action: String,
        count: u8,
    },
    AddScript {
        name: String,
        source: gob_core::ScriptSource,
    },
    RemoveScript(String),
    MoveScript {
        id: String,
        up: bool,
    },
    SetScriptName {
        id: String,
        name: String,
    },
    SetScriptSource {
        id: String,
        source: gob_core::ScriptSource,
    },
    /// `None` clears the value.
    SetScriptVariable {
        id: String,
        name: String,
        value: Option<gob_core::ScriptValue>,
    },
    /// Game export: `output` drives the game action `action` of `game`.
    GameBind {
        game: String,
        action: String,
        output: gob_core::GameOutput,
    },
    /// Game export: unlinks `action` from `output` (`None`: from every output).
    GameUnbind {
        game: String,
        action: String,
        output: Option<gob_core::GameOutput>,
    },
    /// Game export: the keyboard and mouse bindings of `action`; `Some` makes them the
    /// profile's (an empty list: none at all), `None` leaves them to the game.
    GameKeys {
        game: String,
        action: String,
        keys: Option<Vec<gob_core::Combo>>,
    },
    /// Game export: a Map to vJoy from a physical input onto `output`, in `mode`.
    GameMap {
        device: String,
        device_name: String,
        input_type: InputType,
        input_id: u32,
        mode: String,
        output: VirtualOutput,
    },
    /// Game export: how the game numbers the virtual devices.
    GameDevices {
        game: String,
        devices: Vec<gob_core::GameDevice>,
    },
    /// Game export: the target and game version last exported to.
    GameExported {
        game: String,
        target: String,
        version: Option<String>,
    },
}

pub fn apply(p: &mut Profile, op: Op) {
    match op {
        Op::Bind {
            device,
            name,
            input_type,
            input_id,
            mode,
            layer,
            add,
        } => {
            list_device(p, &device, name);
            let root = p.bind_layer(&device, &input_type, &input_id, &mode, &layer);
            if let Some(kind) = add {
                add_to(p, &root, "actions", &kind);
            }
        }
        Op::Unbind {
            device,
            input_type,
            input_id,
            mode,
            layer,
        } => p.unbind_layer(&device, &input_type, &input_id, &mode, &layer),
        Op::SetLayerAdditive {
            device,
            input_type,
            input_id,
            mode,
            layer,
            additive,
        } => p.set_layer_additive(&device, &input_type, &input_id, &mode, &layer, additive),
        Op::SetLayerSendState {
            device,
            input_type,
            input_id,
            mode,
            layer,
            send,
        } => p.set_layer_send_state(&device, &input_type, &input_id, &mode, &layer, send),
        Op::SetLayerKeepValue {
            device,
            input_type,
            input_id,
            mode,
            layer,
            keep,
        } => p.set_layer_keep_value(&device, &input_type, &input_id, &mode, &layer, keep),
        Op::SetVirtualButton {
            root,
            input_type,
            button,
        } => p.set_virtual_button(&root, &input_type, button.as_ref()),
        Op::AddButtonSequence {
            device,
            name,
            input_type,
            input_id,
            mode,
            layer,
            button,
        } => {
            list_device(p, &device, name);
            let added = p.add_sequence(
                &device,
                &input_type,
                &input_id,
                &mode,
                &layer,
                Some(&button),
            );
            if added.is_none() {
                let root = p.bind_layer(&device, &input_type, &input_id, &mode, &layer);
                p.set_virtual_button(&root, &input_type, Some(&button));
            }
        }
        Op::RemoveSequence(root) => p.remove_sequence(&root),
        Op::Add {
            parent,
            group,
            action,
        } => add_to(p, &parent, &group, &action),
        Op::TwoAxisInput {
            action,
            slot,
            device,
            name,
            axis,
            mode,
            layer,
        } => {
            list_device(p, &device, name);
            for line in p.set_two_axis_input(&action, slot, &(device, axis), &mode, &layer) {
                log::debug!("{line}");
            }
        }
        Op::TwoAxisAdopt {
            action,
            device,
            axis,
        } => {
            if let Some(line) = p.two_axis_adopt(&action, &(device, axis)) {
                log::debug!("{line}");
            }
        }
        Op::Remove {
            parent,
            group,
            index,
        } => p.remove_child(&parent, &group, index),
        Op::Unlink {
            parent,
            group,
            index,
        } => {
            p.unlink_child(&parent, &group, index);
        }
        Op::Move {
            parent,
            group,
            from,
            to,
        } => p.move_child(&parent, &group, from, to),
        Op::Unset { action, name } => {
            if let Some(a) = p.action_mut(&action) {
                a.unset(&name);
            }
        }
        Op::Set {
            action,
            name,
            ty,
            value,
        } => {
            if let Some(a) = p.action_mut(&action) {
                a.set(&name, &ty, value);
            }
        }
        Op::HatButtonCount { action, count } => p.set_hat_button_count(&action, count),
        Op::AddScript { name, source } => {
            p.add_script(&name, source);
        }
        Op::RemoveScript(id) => p.remove_script(&id),
        Op::MoveScript { id, up } => p.move_script(&id, up),
        Op::SetScriptName { id, name } => p.set_script_name(&id, &name),
        Op::SetScriptSource { id, source } => p.set_script_source(&id, &source),
        Op::SetScriptVariable { id, name, value } => {
            p.set_script_variable(&id, &name, value.as_ref())
        }
        Op::SetDecrease { action, input } => {
            if let Some((device, name, ..)) = &input {
                list_device(p, device, name.clone());
            }
            if let Some(a) = p.action_mut(&action) {
                match input {
                    Some((device, _, input_type, id)) => {
                        a.set("decrease-device-id", "uuid", device);
                        a.set("decrease-input-type", "string", input_type);
                        a.set("decrease-input-id", "int", id.to_string());
                    }
                    None => {
                        for name in [
                            "decrease-device-id",
                            "decrease-input-type",
                            "decrease-input-id",
                        ] {
                            a.unset(name);
                        }
                    }
                }
            }
        }
        Op::ButtonAxisLogical(action) => {
            let first = p
                .logical_inputs()
                .into_iter()
                .find(|i| i.input_type == InputType::Axis)
                .map(|i| i.id);
            let id = first.unwrap_or_else(|| p.add_logical_input(InputType::Axis));
            if let Some(a) = p.action_mut(&action) {
                a.set("output-type", "string", "logical");
                a.set("logical-id", "int", id.to_string());
            }
        }
        Op::SetKeys { action, keys } => p.set_keys(&action, &keys),
        Op::AddChainStep(action) => p.add_chain_step(&action),
        Op::AddZonePoint(action) => p.add_zone_point(&action),
        Op::RemoveZonePoint { action, index } => p.remove_zone_point(&action, index),
        Op::SetZonePoint {
            action,
            index,
            value,
        } => p.set_zone_point(&action, index, value),
        Op::SetZoneOutput {
            action,
            zone,
            output,
        } => p.set_zone_output(&action, zone, output),
        Op::AddLogicalInput(input_type) => {
            p.add_logical_input(input_type);
        }
        Op::RenameLogicalInput {
            input_type,
            id,
            label,
        } => {
            p.rename_logical_input(input_type, id, &label);
        }
        Op::RemoveLogicalInput { input_type, id } => p.remove_logical_input(input_type, id),
        Op::AddVoiceNode { parent, group } => {
            p.add_voice_node(parent, group);
        }
        Op::SetVoiceParent { id, parent } => {
            p.set_voice_parent(id, parent);
        }
        Op::DuplicateVoiceGroup(id) => {
            p.duplicate_voice_group(id);
        }
        Op::GroupVoiceNodes(ids) => {
            p.group_voice_nodes(&ids);
        }
        Op::CopyVoiceNodes { ids, parent } => {
            p.copy_voice_nodes(&ids, parent);
        }
        Op::MoveVoiceNodes { ids, parent } => {
            p.move_voice_nodes(&ids, parent);
        }
        Op::AddCombo { keys, mode } => {
            if let Some(id) = p.add_key_combo(&keys) {
                p.bind_layer(
                    gob_core::KEYBOARD_GUID,
                    gob_core::COMBO_INPUT_TYPE,
                    &id.to_string(),
                    &mode,
                    &Layer::base(),
                );
            }
        }
        Op::SetComboKeys { id, keys } => {
            p.set_combo_keys(id, &keys);
        }
        Op::RemoveCombo(id) => p.remove_key_combo(id),
        Op::AddOscInput { address, kind } => {
            p.add_osc_input(&address, kind);
        }
        Op::SetOscAddress { id, address } => {
            p.set_osc_address(id, &address);
        }
        Op::SetOscKind { id, kind } => {
            p.set_osc_kind(id, kind);
        }
        Op::SetOscRange { id, low, high } => {
            p.set_osc_range(id, low, high);
        }
        Op::RemoveOscInput(id) => p.remove_osc_input(id),
        Op::SetOscArgs { action, args } => p.set_osc_args(&action, &args),
        Op::VoiceFromAction {
            action,
            phrase,
            mode,
            ..
        } => {
            p.add_voice_command_for(&action, &phrase, &mode);
        }
        Op::RemoveVoiceCommand(id) => p.remove_voice_command(id),
        Op::SetVoicePhrases { id, phrases } => {
            p.set_voice_phrases(id, &phrases);
        }
        Op::SetVoiceHold { id, hold } => {
            p.set_voice_hold(id, hold);
        }
        Op::SetMacroStep {
            action,
            index,
            step,
        } => p.set_macro_step(&action, index, &step),
        Op::RemoveMacroStep { action, index } => p.remove_macro_step(&action, index),
        Op::SwapMacroSteps { action, index } => p.swap_macro_steps(&action, index),
        Op::RemoveChainStep { action, index } => p.remove_chain_step(&action, index),
        // Handled by the app (previews) or the page (going to a command) before edits are applied.
        Op::PreviewSound { .. }
        | Op::PreviewSpeech(_)
        | Op::PreviewLight { .. }
        | Op::SaveCurvePreset(_)
        | Op::DeleteCurvePreset(_)
        | Op::GoToVoice(_) => {}
        Op::GameBind {
            game,
            action,
            output,
        } => {
            p.bind_game_action(&game, &action, output);
        }
        Op::GameUnbind {
            game,
            action,
            output,
        } => p.unbind_game_action(&game, &action, output.as_ref()),
        Op::GameKeys { game, action, keys } => p.set_game_keys(&game, &action, keys),
        Op::GameMap {
            device,
            device_name,
            input_type,
            input_id,
            mode,
            output,
        } => {
            if let VirtualOutput::Vjoy {
                device: vjoy,
                input_type: output_type,
                id,
            } = output
            {
                if !p.devices.iter().any(|d| d.id.eq_ignore_ascii_case(&device)) {
                    p.devices.push(gob_core::Device {
                        id: device.clone(),
                        name: device_name,
                    });
                }
                let kind = match input_type {
                    InputType::Axis => "axis",
                    InputType::Hat => "hat",
                    _ => "button",
                };
                let root = p.bind(&device, kind, &input_id.to_string(), &mode);
                let action = gob_core::Action::new(&NewAction::MapToVjoy {
                    input_type: output_type,
                    device: vjoy,
                    input_id: id,
                });
                p.add_child(&root, "actions", action);
            }
        }
        Op::GameDevices { game, devices } => p.set_game_devices(&game, devices),
        Op::GameExported {
            game,
            target,
            version,
        } => {
            let g = p.game_bindings_mut(&game);
            g.target = target;
            g.version = version;
        }
        Op::SetCondition {
            action,
            index,
            condition,
        } => p.set_condition(&action, index, &condition),
        Op::RemoveCondition { action, index } => p.remove_condition(&action, index),
        Op::SetInGroup {
            action,
            group,
            name,
            ty,
            value,
        } => {
            if let Some(a) = p.action_mut(&action) {
                let items = a.group_mut(&group);
                match items.iter_mut().find_map(|i| match i {
                    Item::Property(p) if p.name == name => Some(p),
                    _ => None,
                }) {
                    Some(prop) => prop.value = value,
                    None => items.push(Item::Property(Property { ty, name, value })),
                }
            }
        }
        Op::SetPoints { action, points } => {
            if let Some(a) = p.action_mut(&action) {
                *a.group_mut("control-points") = points
                    .iter()
                    .map(|&(x, y)| {
                        Item::Property(Property {
                            ty: "point2d".into(),
                            name: "point".into(),
                            value: format!("{},{}", py_float(x), py_float(y)),
                        })
                    })
                    .collect();
            }
        }
        Op::SetTargets { action, targets } => {
            if let Some(a) = p.action_mut(&action) {
                a.items
                    .retain(|i| !matches!(i, Item::Group { tag, .. } if tag == "target-mode"));
                let at = a
                    .items
                    .iter()
                    .position(|i| matches!(i, Item::Property(p) if p.name == "change-type"))
                    .map_or(0, |i| i + 1);
                for (n, t) in targets.into_iter().enumerate() {
                    let item = Item::Group {
                        tag: "target-mode".into(),
                        attrs: Vec::new(),
                        items: vec![Item::Property(Property {
                            ty: "string".into(),
                            name: "name".into(),
                            value: t,
                        })],
                    };
                    a.items.insert(at + n, item);
                }
            }
        }
    }
}
