use gob_core::{
    ActionKind, InputType, Layer, NewAction, Profile, Section, VOICE_GUID, VOICE_INPUT_TYPE,
    VoiceCommand,
};

const SAMPLE: &str = include_str!("../../../tests/fixtures/sample_r14.xml");
const DEV: &str = "11111111-0000-0000-0000-000000000001";

fn reload(p: &Profile) -> Profile {
    gob_gremlin::from_str(&gob_gremlin::to_string(p)).unwrap()
}

/// A command bound to a vJoy button, and a joystick button that listens on a chosen microphone.
fn voice_profile() -> Profile {
    let mut p = Profile::default();
    let lights = p.add_voice_command();
    p.set_voice_phrases(lights, &["lights on".into(), "  lights   up ".into()]);
    let gear = p.add_voice_command();
    p.set_voice_phrases(gear, &["gear down".into()]);
    p.set_voice_hold(gear, 0.5);
    let root = p.bind(VOICE_GUID, VOICE_INPUT_TYPE, &lights.to_string(), "Default");
    p.add_child(
        &root,
        "actions",
        gob_core::Action::new(&NewAction::MapToVjoy {
            input_type: InputType::Button,
            device: 1,
            input_id: 3,
        }),
    )
    .unwrap();
    let root = p.bind(DEV, "button", "5", "Default");
    let listen = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::VoiceInput),
        )
        .unwrap();
    p.action_mut(&listen)
        .unwrap()
        .set("microphone", "string", "{0.0.1.00000000}.{mic}");
    p
}

#[test]
fn voice_commands_survive_save() {
    let p = voice_profile();
    let text = gob_gremlin::to_string(&p);
    assert!(text.contains(r#"app="Gobloon""#), "own format: {text}");
    assert!(
        text.contains(r#"<command id="1" hold="0.1">"#)
            && text.contains("<phrase>lights on</phrase>")
            && text.contains("<phrase>lights up</phrase>")
            && text.contains(r#"<command id="2" hold="0.5">"#),
        "{text}"
    );
    let back = reload(&p);
    assert_eq!(back.voice_commands, p.voice_commands);
    assert_eq!(back.inputs, p.inputs);
    assert_eq!(back.library, p.library);
    assert_eq!(
        gob_gremlin::to_string(&back),
        text,
        "saving again changes nothing"
    );
}

#[test]
fn commands_come_after_the_devices() {
    let mut p = gob_gremlin::from_str(SAMPLE).unwrap();
    p.add_voice_command();
    assert_eq!(p.order.last(), Some(&Section::VoiceCommands));
    p.set_virtual_devices(gob_core::VirtualDevices {
        joysticks: Vec::new(),
        gamepads: vec![1],
    });
    assert!(
        p.order.ends_with(&[
            Section::Devices,
            Section::VoiceCommands,
            Section::VirtualDevices
        ]),
        "{:?}",
        p.order
    );
    p.add_voice_command();
    assert_eq!(
        p.order
            .iter()
            .filter(|s| **s == Section::VoiceCommands)
            .count(),
        1
    );
}

#[test]
fn a_profile_without_voice_stays_readable_by_gremlin() {
    let p = gob_gremlin::from_str(SAMPLE).unwrap();
    let text = gob_gremlin::to_string(&p);
    assert!(!text.contains("voice-commands") && !text.contains(r#"app="Gobloon""#));
    // Commands removed again: no empty section, back to Gremlin's header.
    let mut p = p;
    let id = p.add_voice_command();
    assert!(p.needs_own_format());
    p.remove_voice_command(id);
    let text = gob_gremlin::to_string(&p);
    assert!(
        !text.contains("voice-commands") && !p.needs_own_format(),
        "{text}"
    );
}

#[test]
fn a_voice_input_action_needs_the_own_format() {
    let mut p = gob_gremlin::from_str(SAMPLE).unwrap();
    let root = p.bind(DEV, "button", "9", "Default");
    p.add_child(
        &root,
        "actions",
        gob_core::Action::new(&NewAction::VoiceInput),
    );
    assert!(p.voice_commands.is_empty());
    assert!(p.needs_own_format());
}

#[test]
fn finds_the_inputs_that_listen() {
    let mut p = voice_profile();
    let found = p.voice_inputs();
    assert_eq!(found.len(), 1);
    let b = &p.inputs[found[0].binding];
    assert_eq!((b.device_id.as_str(), b.input_id.as_str()), (DEV, "5"));
    assert_eq!(found[0].microphone, "{0.0.1.00000000}.{mic}");
    let action = p.action(&found[0].action).unwrap();
    assert_eq!(
        action.kind(),
        ActionKind::VoiceInput {
            microphone: "{0.0.1.00000000}.{mic}"
        }
    );
    // Inside another action (a tempo's long press), on the default microphone.
    let root = p.bind(DEV, "button", "6", "Default");
    let tempo = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::Tempo))
        .unwrap();
    p.add_child(
        &tempo,
        "long-actions",
        gob_core::Action::new(&NewAction::VoiceInput),
    )
    .unwrap();
    let found = reload(&p).voice_inputs();
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].microphone, "");
    assert_eq!(p.inputs[found[1].binding].input_id, "6");
}

#[test]
fn removing_a_command_removes_its_bindings() {
    let mut p = voice_profile();
    let actions = p.library.len();
    p.remove_voice_command(1);
    assert_eq!(
        p.voice_commands.iter().map(|c| c.id).collect::<Vec<_>>(),
        [2]
    );
    assert!(
        !p.inputs
            .iter()
            .any(|b| gob_core::is_voice_binding(&b.device_id, &b.input_type)),
        "the command's binding is gone"
    );
    assert_eq!(p.library.len(), actions - 2, "its root and map-to-vjoy too");
    assert_eq!(p.voice_inputs().len(), 1, "other inputs stay");
    assert_eq!(p.add_voice_command(), 1, "the lowest free id is reused");
}

#[test]
fn phrases_are_cleaned_up() {
    let mut p = Profile::default();
    let id = p.add_voice_command();
    assert!(p.set_voice_phrases(
        id,
        &[
            " Fire  ".into(),
            "".into(),
            "fire".into(),
            "Fire".into(),
            "   ".into(),
        ]
    ));
    assert_eq!(p.voice_command(id).unwrap().phrases, ["Fire", "fire"]);
    assert!(!p.set_voice_phrases(99, &[]));
    assert!(p.set_voice_hold(id, 100.0));
    assert_eq!(p.voice_command(id).unwrap().hold, 10.0);
    assert!(!p.set_voice_hold(id, f64::NAN));
    let other = p.add_voice_command();
    p.set_voice_phrases(other, &["stop".into()]);
    assert_eq!(
        p.voice_phrases(),
        [(id, "Fire"), (id, "fire"), (other, "stop")]
    );
}

#[test]
fn reads_hand_edited_commands() {
    let text = r#"<profile app="Gobloon" version="1">
        <voice-commands>
            <command id="2"><phrase>  lights
                on </phrase><phrase/></command>
            <command id="2" hold="1"><phrase>repeated id</phrase></command>
            <command id="0"><phrase>zero</phrase></command>
            <command><phrase>no id</phrase></command>
            <command id="3" hold="-1"/>
            <command id="4" hold="0.25"><phrase>gear</phrase></command>
        </voice-commands>
    </profile>"#;
    let p = gob_gremlin::from_str(text).unwrap();
    let command = |id, phrases: &[&str], hold| VoiceCommand {
        id,
        phrases: phrases.iter().map(|s| s.to_string()).collect(),
        hold,
    };
    assert_eq!(
        p.voice_commands,
        [
            command(2, &["lights on"], 0.1),
            command(3, &[], 0.1),
            command(4, &["gear"], 0.25),
        ]
    );
}

/// A voice command made from an action card runs that very action, survives a save, and is
/// found again for the card; removing the command leaves the action on its input.
#[test]
fn voice_command_for_an_action() {
    let base = Layer::base();
    let mut p = voice_profile();
    let action = button(&mut p, "6", 8);
    assert_eq!(p.voice_command_running(&action), None);
    // "gear down" (2) runs nothing yet, so a new phrase gets the next id.
    let next = p.next_voice_command_id();
    assert_eq!(p.voice_command_for_phrase("next target"), Ok(next));
    let id = p
        .add_voice_command_for(&action, "  next   target ", "Default", &base)
        .unwrap();
    assert_eq!(id, next);
    assert_eq!(p.voice_command(id).unwrap().phrases, ["next target"]);
    let p = reload(&p);
    assert_eq!(p.voice_command_running(&action), Some(id));
    let b = p
        .inputs
        .iter()
        .find(|b| b.device_id == VOICE_GUID && b.input_id == id.to_string())
        .unwrap();
    let voice_root = b.configs[0].root_action.clone();
    assert_eq!(
        p.action(&voice_root).unwrap().child_ids(),
        [action.as_str()],
        "linked, not copied"
    );

    // The link removed on the Voice tab: the card links the same command again.
    let mut p = p;
    p.remove_child(&voice_root, "actions", 0);
    assert_eq!(p.voice_command_running(&action), None);
    let count = (p.voice_commands.len(), p.inputs.len());
    assert_eq!(p.voice_command_for_phrase("Next Target"), Ok(id));
    assert_eq!(
        p.add_voice_command_for(&action, "Next Target", "Default", &base),
        Some(id)
    );
    assert_eq!(
        (p.voice_commands.len(), p.inputs.len()),
        count,
        "no second command"
    );
    assert_eq!(p.voice_command_running(&action), Some(id));

    p.remove_voice_command(id);
    assert!(p.action(&action).is_some(), "still on its input");
    assert_eq!(p.voice_command_running(&action), None);
}

/// A vJoy button action bound to stick button `input`.
fn button(p: &mut Profile, input: &str, vjoy: u32) -> String {
    let root = p.bind(DEV, "button", input, "Default");
    p.add_child(
        &root,
        "actions",
        gob_core::Action::new(&NewAction::MapToVjoy {
            input_type: InputType::Button,
            device: 1,
            input_id: vjoy,
        }),
    )
    .unwrap()
}

/// A phrase a command with other actions says is taken; one that runs nothing is reused, also
/// through a padded hand-edited binding, in the card's layer.
#[test]
fn voice_command_for_an_action_reuses_only_empty_commands() {
    let mut p = voice_profile();
    let action = button(&mut p, "6", 8);
    let commands = p.voice_commands.len();
    assert_eq!(p.voice_command_for_phrase(" Lights  ON"), Err(1));
    assert_eq!(
        p.add_voice_command_for(&action, "lights on", "Default", &Layer::base()),
        None
    );
    assert_eq!(p.voice_commands.len(), commands);
    assert_eq!(p.voice_command_running(&action), None);

    // "gear down" (2) has a padded binding of its own and no actions.
    p.bind(VOICE_GUID, VOICE_INPUT_TYPE, "2", "Default");
    let b = p.inputs.iter_mut().find(|b| b.input_id == "2").unwrap();
    (b.input_id, b.input_type) = (" 2 ".into(), " voice ".into());
    let bindings = p.inputs.len();
    let shift = Layer::new(["Shift"]);
    assert_eq!(
        p.add_voice_command_for(&action, "gear down", "Default", &shift),
        Some(2)
    );
    assert_eq!(p.inputs.len(), bindings, "the padded binding is reused");
    let b = p.inputs.iter().find(|b| b.input_id.trim() == "2").unwrap();
    assert!(
        b.configs.iter().any(|c| c.layer == shift),
        "the card's layer"
    );
    assert_eq!(p.voice_command_running(&action), Some(2));
}

/// An action wrapped in a container on the Voice tab still counts as linked, and a leftover
/// binding of a removed command does not hide the real one.
#[test]
fn voice_links_look_inside_containers() {
    let mut p = voice_profile();
    let action = button(&mut p, "6", 8);
    let stale = p.bind(VOICE_GUID, VOICE_INPUT_TYPE, "7", "Default");
    p.add_ref(&stale, "actions", &action);
    let root = p.bind(VOICE_GUID, VOICE_INPUT_TYPE, "2", "Default");
    let tempo = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::Tempo))
        .unwrap();
    p.add_ref(&tempo, "short-actions", &action);
    assert_eq!(p.voice_command_running(&action), Some(2));
    assert_eq!(p.voice_command_running(&tempo), Some(2));
    assert_eq!(p.voice_command_for_phrase("gear down"), Err(2));
}
