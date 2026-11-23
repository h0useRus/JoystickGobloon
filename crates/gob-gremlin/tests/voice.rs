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
        [
            (id, "Fire".to_string()),
            (id, "fire".to_string()),
            (other, "stop".to_string())
        ]
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
        phrases: phrases.iter().map(|s| s.to_string()).collect(),
        hold,
        ..VoiceCommand::new(id)
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

const GROUPS: &str = r#"<profile app="Gobloon" version="1">
        <voice-commands>
            <command id="1" hold="0.1"><phrase>lights on</phrase></command>
            <group id="2" hold="0.1"><phrase>radio</phrase><phrase>comms</phrase>
                <group id="3" hold="0.1"><phrase>wing a</phrase>
                    <command id="4" hold="0.2"><phrase>attack</phrase></command>
                    <command id="5" hold="0.1"><phrase>defend</phrase></command>
                </group>
                <command id="6" hold="0.1"><phrase>silence</phrase></command>
            </group>
        </voice-commands>
    </profile>"#;

/// Groups nest commands and groups; they read back as written.
#[test]
fn groups_round_trip() {
    let p = gob_gremlin::from_str(GROUPS).unwrap();
    let tree: Vec<(usize, u32, bool)> = p
        .voice_tree()
        .into_iter()
        .map(|(d, c)| (d, c.id, c.group))
        .collect();
    assert_eq!(
        tree,
        [
            (0, 1, false),
            (0, 2, true),
            (1, 3, true),
            (2, 4, false),
            (2, 5, false),
            (1, 6, false)
        ]
    );
    assert_eq!(p.voice_command(4).unwrap().hold, 0.2);
    let again = gob_gremlin::from_str(&gob_gremlin::to_string(&p)).unwrap();
    assert_eq!(again.voice_commands, p.voice_commands);
}

/// A profile without groups is written as before groups existed.
#[test]
fn commands_without_groups_are_written_as_before() {
    let text = r#"<profile app="Gobloon" version="1">
        <voice-commands>
            <command id="1" hold="0.1"><phrase>lights on</phrase></command>
            <command id="2" hold="0.25"><phrase>gear</phrase><phrase>gear down</phrase></command>
        </voice-commands>
    </profile>"#;
    let p = gob_gremlin::from_str(text).unwrap();
    assert!(
        p.voice_commands
            .iter()
            .all(|c| c.parent.is_none() && !c.group)
    );
    let written = gob_gremlin::to_string(&p);
    assert!(!written.contains("<group"), "{written}");
    assert!(
        written.contains(r#"<command id="2" hold="0.25">"#),
        "{written}"
    );
}

/// Every command and group is said with its groups' phrases before its own; an open group
/// listens only for what is in it.
#[test]
fn group_phrases_and_scopes() {
    let p = gob_gremlin::from_str(GROUPS).unwrap();
    let said = |list: Vec<(u32, String)>| -> Vec<(u32, String)> { list };
    let all = said(p.voice_phrases());
    for (id, phrase) in [
        (1, "lights on"),
        (2, "radio"),
        (2, "comms"),
        (3, "radio wing a"),
        (4, "radio wing a attack"),
        (4, "comms wing a attack"),
        (5, "radio wing a defend"),
        (6, "comms silence"),
    ] {
        assert!(all.contains(&(id, phrase.to_string())), "{phrase}: {all:?}");
    }
    assert_eq!(all.len(), 11);
    assert_eq!(
        p.voice_phrases_in(Some(3)),
        [(4, "attack".to_string()), (5, "defend".to_string())]
    );
    let radio = p.voice_phrases_in(Some(2));
    assert!(radio.contains(&(4, "wing a attack".to_string())));
    assert!(!radio.iter().any(|(id, _)| *id == 1 || *id == 2));
    assert_eq!(p.voice_full_phrase(4), "radio wing a attack");
}

#[test]
fn groups_move_copy_and_go() {
    let mut p = gob_gremlin::from_str(GROUPS).unwrap();
    // Not into itself or what is inside it, nor into a command.
    assert!(!p.set_voice_parent(2, Some(3)));
    assert!(!p.set_voice_parent(2, Some(2)));
    assert!(!p.set_voice_parent(1, Some(4)));
    assert!(p.set_voice_parent(1, Some(3)));
    assert_eq!(p.voice_full_phrase(1), "radio wing a lights on");
    assert!(p.set_voice_parent(1, None));
    // A copy of wing a, with its own ids, in radio.
    let copy = p.duplicate_voice_group(3).unwrap();
    assert_eq!(p.voice_command(copy).unwrap().parent, Some(2));
    let inside: Vec<Vec<String>> = p
        .voice_descendants(copy)
        .iter()
        .map(|id| p.voice_command(*id).unwrap().phrases.clone())
        .collect();
    assert_eq!(
        inside,
        [vec!["attack".to_string()], vec!["defend".to_string()]]
    );
    // Removing a group removes what is in it.
    let in_radio = p.voice_descendants(2);
    p.remove_voice_command(2);
    assert!(in_radio.iter().all(|id| p.voice_command(*id).is_none()));
    assert_eq!(p.voice_commands.len(), 1);
}

/// A voice command made from an action card runs that very action, survives a save, and is
/// found again for the card; removing the command leaves the action on its input.
#[test]
fn voice_command_for_an_action() {
    let mut p = voice_profile();
    let action = button(&mut p, "6", 8);
    assert_eq!(p.voice_command_running(&action), None);
    // "gear down" (2) runs nothing yet, so a new phrase gets the next id.
    let next = p.next_voice_command_id();
    assert_eq!(p.voice_command_for_phrase("next target"), Ok(next));
    let id = p
        .add_voice_command_for(&action, "  next   target ", "Default")
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
        p.add_voice_command_for(&action, "Next Target", "Default"),
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
/// through a padded hand-edited binding, on the base layer.
#[test]
fn voice_command_for_an_action_reuses_only_empty_commands() {
    let mut p = voice_profile();
    let action = button(&mut p, "6", 8);
    let commands = p.voice_commands.len();
    assert_eq!(p.voice_command_for_phrase(" Lights  ON"), Err(1));
    assert_eq!(
        p.add_voice_command_for(&action, "lights on", "Default"),
        None
    );
    assert_eq!(p.voice_commands.len(), commands);
    assert_eq!(p.voice_command_running(&action), None);

    // "gear down" (2) has a padded binding of its own and no actions.
    p.bind(VOICE_GUID, VOICE_INPUT_TYPE, "2", "Default");
    let b = p.inputs.iter_mut().find(|b| b.input_id == "2").unwrap();
    (b.input_id, b.input_type) = (" 2 ".into(), " voice ".into());
    let bindings = p.inputs.len();
    assert_eq!(
        p.add_voice_command_for(&action, "gear down", "Default"),
        Some(2)
    );
    assert_eq!(p.inputs.len(), bindings, "the padded binding is reused");
    let b = p.inputs.iter().find(|b| b.input_id.trim() == "2").unwrap();
    assert!(
        b.configs.iter().all(|c| c.layer == Layer::base()),
        "the base layer"
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

/// Selected commands and groups go into a new group made where the first of them was.
#[test]
fn selected_nodes_into_a_new_group() {
    let mut p = gob_gremlin::from_str(GROUPS).unwrap();
    // 4 is inside 3: it moves with it, not on its own.
    let g = p.group_voice_nodes(&[6, 3, 4]).unwrap();
    let tree: Vec<(usize, u32)> = p.voice_tree().into_iter().map(|(d, c)| (d, c.id)).collect();
    assert_eq!(
        tree,
        [(0, 1), (0, 2), (1, g), (2, 3), (3, 4), (3, 5), (2, 6)]
    );
    assert!(p.voice_command(g).unwrap().group);
    assert_eq!(p.group_voice_nodes(&[99]), None);
    // Top-level commands make a top-level group.
    let g2 = p.group_voice_nodes(&[1]).unwrap();
    assert_eq!(p.voice_command(g2).unwrap().parent, None);
    assert_eq!(p.voice_command(1).unwrap().parent, Some(g2));
}

/// Whether command `id` has a binding (its actions).
fn voice_bound(p: &gob_core::Profile, id: u32) -> bool {
    p.inputs
        .iter()
        .any(|b| b.device_id == gob_core::VOICE_GUID && b.input_id == id.to_string())
}

/// Copied commands and groups go to the end of the target group with new ids and no actions;
/// a group may be copied into itself.
#[test]
fn copied_nodes_have_no_actions() {
    let mut p = gob_gremlin::from_str(GROUPS).unwrap();
    p.bind_layer(
        gob_core::VOICE_GUID,
        gob_core::VOICE_INPUT_TYPE,
        "4",
        "Default",
        &gob_core::Layer::base(),
    );
    // 4 is inside 3: copied with it, not twice.
    let made = p.copy_voice_nodes(&[4, 3, 1], Some(3));
    // In tree order: 1 (now 7), then 3 (now 8, with 9 and 10).
    assert_eq!(made, [7, 8]);
    let tree: Vec<(usize, u32)> = p.voice_tree().into_iter().map(|(d, c)| (d, c.id)).collect();
    assert_eq!(
        tree,
        [
            (0, 1),
            (0, 2),
            (1, 3),
            (2, 4),
            (2, 5),
            (2, 7),
            (2, 8),
            (3, 9),
            (3, 10),
            (1, 6)
        ]
    );
    assert_eq!(p.voice_command(9).unwrap().phrases, ["attack"]);
    assert_eq!(p.voice_command(9).unwrap().hold, 0.2);
    assert!(voice_bound(&p, 4));
    assert!(!voice_bound(&p, 9));
    // A command is no group to copy into: the top level then.
    assert_eq!(p.copy_voice_nodes(&[5], Some(1)), [11]);
    assert_eq!(p.voice_command(11).unwrap().parent, None);
}

/// Moved commands and groups keep their ids and actions; one cannot go into itself.
#[test]
fn moved_nodes_keep_their_actions() {
    let mut p = gob_gremlin::from_str(GROUPS).unwrap();
    p.bind_layer(
        gob_core::VOICE_GUID,
        gob_core::VOICE_INPUT_TYPE,
        "4",
        "Default",
        &gob_core::Layer::base(),
    );
    assert_eq!(p.move_voice_nodes(&[4, 1], Some(2)), [1, 4]);
    let tree: Vec<(usize, u32)> = p.voice_tree().into_iter().map(|(d, c)| (d, c.id)).collect();
    assert_eq!(tree, [(0, 2), (1, 3), (2, 5), (1, 6), (1, 1), (1, 4)]);
    assert!(voice_bound(&p, 4));
    // 2 holds 3: it stays; 6 goes.
    assert_eq!(p.move_voice_nodes(&[2], Some(3)), Vec::<u32>::new());
    assert_eq!(p.move_voice_nodes(&[6], None), [6]);
    assert_eq!(p.voice_command(6).unwrap().parent, None);
}

/// Groups in a loop and groups deeper than any limit are written too: nothing is lost on save.
#[test]
fn loops_and_deep_groups_survive_saving() {
    let mut p = gob_gremlin::from_str(GROUPS).unwrap();
    // 2 holds 3; now 2 is in 3 too.
    p.voice_commands
        .iter_mut()
        .find(|c| c.id == 2)
        .unwrap()
        .parent = Some(3);
    let mut parent = None;
    for _ in 0..20 {
        parent = Some(p.add_voice_node(parent, true));
    }
    let leaf = p.add_voice_node(parent, false);
    let again = gob_gremlin::from_str(&gob_gremlin::to_string(&p)).unwrap();
    let mut ids: Vec<u32> = again.voice_commands.iter().map(|c| c.id).collect();
    ids.sort();
    let mut want: Vec<u32> = p.voice_commands.iter().map(|c| c.id).collect();
    want.sort();
    assert_eq!(ids, want);
    assert_eq!(again.voice_groups_of(leaf).len(), 20);
    assert_eq!(again.voice_command(4).unwrap().parent, Some(3));
}
