use super::*;
use crate::{AxisDirection, VirtualButton};

#[test]
fn axis_sequences_as_buttons() {
    let mut p = Profile::default();
    let base = Layer::base();
    let axis = p.bind_layer("{DEV}", "axis", "3", "Default", &base);
    let range = VirtualButton::Axis {
        lower: 0.75,
        upper: 1.0,
        direction: AxisDirection::Below,
    };
    let button = p
        .add_sequence("{DEV}", "axis", "3", "Default", &base, Some(&range))
        .unwrap();
    let b = &p.inputs[0];
    assert_eq!(b.configs.len(), 2);
    assert_eq!(b.configs[0].virtual_button(b.input_type()), None);
    assert_eq!(b.configs[1].virtual_button(b.input_type()), Some(range));

    p.set_virtual_button(
        &axis,
        "axis",
        VirtualButton::default_for(InputType::Axis).as_ref(),
    );
    assert_eq!(p.inputs[0].configs[0].behavior(), InputType::Button);
    p.set_virtual_button(&axis, "axis", None);
    assert_eq!(p.inputs[0].configs[0].behavior(), InputType::Axis);
    assert!(p.inputs[0].configs[0].virtual_button.is_none());

    p.remove_sequence(&button);
    assert_eq!(p.inputs[0].configs.len(), 1);
    assert!(p.action(&button).is_none());
    p.remove_sequence(&axis);
    assert!(p.inputs.is_empty());
}

#[test]
fn a_shared_action_is_counted_and_unlinked() {
    let mut p = Profile::default();
    let a = p.bind("{DEV}", "button", "10", "Default");
    let b = p.bind("{DEV}", "button", "11", "Default");
    let tempo = p
        .add_child(&a, "actions", Action::new(&NewAction::Tempo))
        .unwrap();
    let vjoy = NewAction::MapToVjoy {
        input_type: InputType::Button,
        device: 1,
        input_id: 1,
    };
    let short = p
        .add_child(&tempo, "short-actions", Action::new(&vjoy))
        .unwrap();
    assert!(p.add_ref(&b, "actions", &tempo));
    // A voice command running it is one more place.
    let voice = p.bind(crate::VOICE_GUID, crate::VOICE_INPUT_TYPE, "1", "Default");
    assert!(p.add_ref(&voice, "actions", &tempo));

    let counts = p.use_counts();
    assert_eq!(counts.get(tempo.as_str()), Some(&3));
    assert_eq!(counts.get(short.as_str()), Some(&1));
    assert_eq!(counts.get(a.as_str()), Some(&1));

    let copy = p.unlink_child(&b, "actions", 0).unwrap();
    assert_ne!(copy, tempo);
    assert_eq!(p.use_counts().get(tempo.as_str()), Some(&2));
    assert_eq!(p.use_counts().get(copy.as_str()), Some(&1));
    // The copy has its own outputs: a change there leaves the original alone.
    let copied_short = p.action(&copy).unwrap().child_ids()[0].to_string();
    assert_ne!(copied_short, short);
    p.action_mut(&copied_short)
        .unwrap()
        .set("vjoy-input-id", "int", "5");
    assert_ne!(
        p.action(&short).unwrap().items,
        p.action(&copied_short).unwrap().items
    );
    assert!(p.unlink_child(&b, "actions", 3).is_none());
}

#[test]
fn pasted_named_actions_get_a_number() {
    let mut p = Profile::default();
    let a = p.bind("{DEV}", "button", "10", "Default");
    let b = p.bind("{DEV}", "button", "11", "Default");
    let named = |p: &mut Profile, parent: &str, label: &str| {
        let id = p
            .add_child(parent, "actions", Action::new(&NewAction::Macro))
            .unwrap();
        p.action_mut(&id)
            .unwrap()
            .set("action-label", "string", label);
        id
    };
    let one = named(&mut p, &a, "One");
    let two = named(&mut p, &a, "Two (2)");
    let plain = p
        .add_child(&a, "actions", Action::new(&NewAction::Macro))
        .unwrap();
    let label = |p: &Profile, id: &str| p.action(id).unwrap().label().map(String::from);

    let tops = vec![one.clone(), two.clone(), plain.clone()];
    let clip = p.subtree(&tops);
    let first = p.paste(&clip, &tops);
    let second = p.paste(&clip, &tops);
    for id in first.iter().chain(&second) {
        assert!(p.add_ref(&b, "actions", id));
    }
    assert_eq!(label(&p, &first[0]).as_deref(), Some("One (2)"));
    assert_eq!(label(&p, &second[0]).as_deref(), Some("One (3)"));
    assert_eq!(label(&p, &first[1]).as_deref(), Some("Two (3)"));
    assert_eq!(label(&p, &second[1]).as_deref(), Some("Two (4)"));
    // No name, no number.
    assert_eq!(label(&p, &first[2]), label(&p, &plain));
    // The originals keep theirs.
    assert_eq!(label(&p, &one).as_deref(), Some("One"));

    // Into a profile without the name, the copy keeps it.
    let mut other = Profile::default();
    let pasted = other.paste(&clip, &tops);
    assert_eq!(label(&other, &pasted[0]).as_deref(), Some("One"));
}

#[test]
fn hat_directions_round_trip() {
    let mut p = Profile::default();
    let root = p.bind("{DEV}", "hat", "1", "Default");
    let vb = VirtualButton::Hat {
        directions: vec![Some(0), Some(4500), None],
    };
    p.set_virtual_button(&root, "hat", Some(&vb));
    let b = &p.inputs[0];
    assert_eq!(b.configs[0].virtual_button(b.input_type()), Some(vb));
}

#[test]
fn keys_and_voice_commands_run_their_actions_as_buttons() {
    let mut p = Profile::default();
    let base = Layer::base();
    for (device, input_type) in [
        (crate::KEYBOARD_GUID, "key"),
        (crate::VOICE_GUID, crate::VOICE_INPUT_TYPE),
        (crate::EVENTS_GUID, crate::EVENT_INPUT_TYPE),
        ("{DEV}", "button"),
    ] {
        p.bind_layer(device, input_type, "1", "Default", &base);
    }
    p.bind_layer("{DEV}", "axis", "2", "Default", &base);
    let behaviors: Vec<InputType> = p.inputs.iter().map(|b| b.configs[0].behavior()).collect();
    assert_eq!(
        behaviors,
        [
            InputType::Button,
            InputType::Button,
            InputType::Button,
            InputType::Button,
            InputType::Axis
        ]
    );
}
