use gob_core::{ActionKind, InputType, NewAction, Profile};

const SAMPLE: &str = include_str!("../../../tests/fixtures/sample_r14.xml");
const DEV: &str = "11111111-0000-0000-0000-000000000001";

fn sample() -> Profile {
    gob_gremlin::from_str(SAMPLE).unwrap()
}

fn reload(p: &Profile) -> Profile {
    gob_gremlin::from_str(&gob_gremlin::to_string(p)).unwrap()
}

#[test]
fn uuids_are_unique_v4() {
    let a = gob_core::new_uuid();
    let b = gob_core::new_uuid();
    assert_ne!(a, b);
    assert_eq!(a.len(), 36);
    assert_eq!(&a[14..15], "4");
}

#[test]
fn bind_and_add_actions_survive_save() {
    let mut p = sample();
    let root = p.bind(DEV, "button", "7", "Default");
    assert_eq!(
        p.bind(DEV, "button", "7", "Default"),
        root,
        "binding twice reuses the root"
    );
    let vjoy = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::MapToVjoy {
                input_type: InputType::Button,
            }),
        )
        .unwrap();
    p.action_mut(&vjoy)
        .unwrap()
        .set("vjoy-input-id", "int", "12");
    let tempo = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::Tempo))
        .unwrap();
    p.add_child(
        &tempo,
        "long-actions",
        gob_core::Action::new(&NewAction::Description),
    )
    .unwrap();

    let p = reload(&p);
    let v = p.action(&vjoy).unwrap();
    assert_eq!(
        v.kind(),
        ActionKind::MapToVjoy {
            device: 1,
            input_type: InputType::Button,
            input_id: 12,
            inverted: false
        }
    );
    let names: Vec<&str> = v
        .items
        .iter()
        .filter_map(|i| match i {
            gob_core::Item::Property(p) => Some(p.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        [
            "vjoy-device-id",
            "vjoy-input-id",
            "vjoy-input-type",
            "button-inverted",
            "action-label",
            "activation-mode"
        ],
        "Gremlin's property order"
    );
    assert!(
        matches!(p.action(&tempo).unwrap().kind(), ActionKind::Tempo { long, .. } if long.len() == 1)
    );
    assert!(gob_gremlin::report(&p)
        .warnings
        .iter()
        .all(|w| !matches!(w, gob_gremlin::Warning::DanglingRef { .. })));
}

#[test]
fn removing_children_collects_garbage() {
    let mut p = sample();
    let before = p.library.len();
    let root = p.bind(DEV, "button", "7", "Default");
    let tempo = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::Tempo))
        .unwrap();
    p.add_child(
        &tempo,
        "short-actions",
        gob_core::Action::new(&NewAction::Description),
    )
    .unwrap();
    assert_eq!(p.library.len(), before + 3);
    p.remove_child(&root, "actions", 0);
    assert_eq!(
        p.library.len(),
        before + 1,
        "tempo and its child are gone, the root stays"
    );
    p.unbind(DEV, "button", "7", "Default");
    assert_eq!(p.library.len(), before);
}

#[test]
fn move_child_reorders() {
    let mut p = sample();
    let root = p.bind(DEV, "button", "7", "Default");
    let a = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::Description),
        )
        .unwrap();
    let b = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::ResponseCurve),
        )
        .unwrap();
    p.move_child(&root, "actions", 1, 0);
    assert_eq!(p.action(&root).unwrap().children("actions"), [b, a]);
}

#[test]
fn rename_mode_updates_every_reference() {
    let mut p = sample();
    assert!(p.rename_mode("Default", "Main"));
    assert!(!p.rename_mode("Combat", "Main"), "names stay unique");
    let p = reload(&p);
    assert_eq!(p.startup_mode().as_deref(), Some("Main"));
    assert_eq!(p.modes[1].parent.as_deref(), Some("Main"));
    assert!(p.inputs.iter().any(|b| b.mode == "Main"));
    assert!(!gob_gremlin::to_string(&p).contains(">Default<"));
}

#[test]
fn mode_parents_and_removal() {
    let mut p = sample();
    assert!(p.add_mode("Landing", Some("Combat")));
    assert!(
        !p.set_mode_parent("Default", Some("Landing")),
        "would create a cycle"
    );
    assert!(p.remove_mode("Combat"));
    assert_eq!(
        p.modes
            .iter()
            .find(|m| m.name == "Landing")
            .unwrap()
            .parent
            .as_deref(),
        Some("Default")
    );
    assert!(p.inputs.iter().all(|b| b.mode != "Combat"));
    assert!(!p.remove_mode("Unknown"));
}

#[test]
fn hat_button_count_keeps_shared_directions() {
    let mut p = sample();
    let root = p.bind(DEV, "hat", "1", "Default");
    let hb = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::HatButtons),
        )
        .unwrap();
    let north = p
        .add_child(&hb, "North", gob_core::Action::new(&NewAction::Description))
        .unwrap();
    p.set_hat_button_count(&hb, 8);
    let ne = p
        .add_child(
            &hb,
            "North-East",
            gob_core::Action::new(&NewAction::Description),
        )
        .unwrap();
    let p8 = reload(&p);
    let ActionKind::HatButtons { directions } = p8.action(&hb).unwrap().kind() else {
        panic!("not hat-buttons");
    };
    assert_eq!(directions.len(), 9);
    assert_eq!(directions[0].2, [north.as_str()]);
    assert_eq!(directions[1], ("North-East", Some(4500), vec![ne.as_str()]));

    p.set_hat_button_count(&hb, 4);
    let p4 = reload(&p);
    let ActionKind::HatButtons { directions } = p4.action(&hb).unwrap().kind() else {
        panic!("not hat-buttons");
    };
    let tags: Vec<&str> = directions.iter().map(|d| d.0).collect();
    assert_eq!(tags, ["North", "East", "South", "West", "Center"]);
    assert_eq!(directions[0].2, [north.as_str()]);
    assert!(
        p4.action(&ne).is_none(),
        "dropped diagonal actions are collected"
    );
}

#[test]
fn merge_axis_is_shared_by_both_axes() {
    let mut p = sample();
    let r1 = p.bind(DEV, "axis", "6", "Default");
    let r2 = p.bind(DEV, "axis", "7", "Default");
    let merge = p
        .add_child(
            &r1,
            "actions",
            gob_core::Action::new(&NewAction::MergeAxis {
                device: DEV.into(),
                axis: 6,
            }),
        )
        .unwrap();
    assert!(p.add_ref(&r2, "actions", &merge));
    p.action_mut(&merge).unwrap().set("axis2-axis", "int", "7");
    p.remove_child(&r1, "actions", 0);

    let p = reload(&p);
    assert_eq!(
        p.action(&merge).unwrap().kind(),
        ActionKind::MergeAxis {
            axis1: (DEV, 6),
            axis2: (DEV, 7),
            operation: gob_core::MergeOp::Average,
            children: Vec::new(),
        },
        "still reachable through the second axis"
    );
}

#[test]
fn link_refuses_loops_and_duplicate_copies_the_subtree() {
    let mut p = sample();
    let root = p.bind(DEV, "button", "7", "Default");
    let tempo = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::Tempo))
        .unwrap();
    let desc = p
        .add_child(
            &tempo,
            "short-actions",
            gob_core::Action::new(&NewAction::Description),
        )
        .unwrap();
    assert!(
        !p.add_ref(&desc, "actions", &tempo),
        "tempo contains desc: loop"
    );
    assert!(!p.add_ref(&tempo, "long-actions", &tempo), "self");
    assert!(
        p.add_ref(&tempo, "long-actions", &desc),
        "shared inside is fine"
    );

    let before = p.library.len();
    let copy = p.duplicate(&tempo).unwrap();
    assert_ne!(copy, tempo);
    assert_eq!(
        p.library.len(),
        before + 2,
        "tempo and its one (shared) child"
    );
    let other = p.bind(DEV, "button", "8", "Default");
    assert!(p.add_ref(&other, "actions", &copy));
    let p = reload(&p);
    let copied = p.action(&copy).unwrap();
    let child = copied.child_ids()[0].to_string();
    assert_ne!(child, desc, "children are copies too");
    assert_eq!(
        copied.child_ids(),
        [child.as_str(), child.as_str()],
        "sharing kept"
    );
}

#[test]
fn conditions_are_edited_in_place() {
    use gob_core::{Comparator, Condition, ConditionSource};
    let mut p = sample();
    let root = p.bind(DEV, "hat", "1", "Default");
    let cond = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::Condition),
        )
        .unwrap();
    let hat = Condition {
        source: ConditionSource::CurrentInput,
        comparator: Comparator::Direction(vec![Some(0), None]),
    };
    let vjoy = Condition {
        source: ConditionSource::Vjoy {
            device: 2,
            input_type: InputType::Button,
            id: 7,
        },
        comparator: Comparator::Pressed(false),
    };
    p.set_condition(&cond, 0, &hat);
    p.set_condition(&cond, 1, &vjoy);
    let p2 = reload(&p);
    let ActionKind::Condition {
        all, conditions, ..
    } = p2.action(&cond).unwrap().kind()
    else {
        panic!("not a condition");
    };
    assert!(all);
    assert_eq!(conditions, [hat.clone(), vjoy.clone()]);

    p.remove_condition(&cond, 0);
    let ActionKind::Condition { conditions, .. } = reload(&p).action(&cond).unwrap().kind() else {
        panic!("not a condition");
    };
    assert_eq!(conditions, [vjoy]);
}

#[test]
fn map_to_keyboard_keys_are_replaced() {
    use gob_core::Key;
    let mut p = sample();
    let root = p.bind(DEV, "button", "9", "Default");
    let keys = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::MapToKeyboard),
        )
        .unwrap();
    let ctrl_r = Key {
        scan_code: 0x1D,
        extended: true,
    };
    let a = Key {
        scan_code: 0x1E,
        extended: false,
    };
    p.set_keys(&keys, &[ctrl_r, a]);
    p.set_keys(&keys, &[a, ctrl_r]);
    let p = reload(&p);
    assert_eq!(
        p.action(&keys).unwrap().kind(),
        ActionKind::MapToKeyboard {
            keys: vec![a, ctrl_r]
        }
    );
}

#[test]
fn chain_steps_are_added_and_removed() {
    let mut p = sample();
    let root = p.bind(DEV, "button", "10", "Default");
    let chain = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::Chain))
        .unwrap();
    p.add_chain_step(&chain);
    p.add_chain_step(&chain);
    let first = p
        .add_child(
            &chain,
            "chain-0",
            gob_core::Action::new(&NewAction::Description),
        )
        .unwrap();
    let third = p
        .add_child(
            &chain,
            "chain-2",
            gob_core::Action::new(&NewAction::Description),
        )
        .unwrap();
    p.remove_chain_step(&chain, 0);
    assert!(
        p.action(&first).is_none(),
        "the removed step's actions are collected"
    );
    let p = reload(&p);
    let ActionKind::Chain { chains, .. } = p.action(&chain).unwrap().kind() else {
        panic!("not a chain");
    };
    assert_eq!(chains, [vec![], vec![third.as_str()]], "renumbered from 0");
}

#[test]
fn macro_steps_round_trip_and_keep_unknown_ones() {
    use gob_core::{MacroStep, MouseButton};
    let mut p = sample();
    let root = p.bind(DEV, "button", "11", "Default");
    let m = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::Macro))
        .unwrap();
    let steps = [
        MacroStep::Key {
            scan_code: 0x1D,
            extended: true,
            pressed: true,
        },
        MacroStep::Pause(0.25),
        MacroStep::MouseButton {
            button: MouseButton::WheelUp,
            pressed: true,
        },
        MacroStep::MouseMotion { dx: -5, dy: 7 },
        MacroStep::VjoyButton {
            device: 2,
            id: 9,
            pressed: false,
        },
        MacroStep::VjoyAxis {
            device: 1,
            id: 3,
            value: -0.5,
            relative: true,
        },
        MacroStep::VjoyHat {
            device: 1,
            id: 1,
            direction: Some(13500),
        },
    ];
    for (i, s) in steps.iter().enumerate() {
        p.set_macro_step(&m, i, s);
    }
    // A step Joystick Gobloon cannot represent, as Gremlin would write it.
    p.action_mut(&m).unwrap().items.push(gob_core::Item::Group {
        tag: "macro-action".into(),
        attrs: vec![("type".into(), "joystick".into())],
        items: Vec::new(),
    });
    p.swap_macro_steps(&m, 0);
    p.remove_macro_step(&m, 2);
    let p = reload(&p);
    let ActionKind::Macro { steps: got, .. } = p.action(&m).unwrap().kind() else {
        panic!("not a macro");
    };
    let mut want = steps.to_vec();
    want.swap(0, 1);
    want.remove(2);
    want.push(MacroStep::Other);
    assert_eq!(got, want);
}

#[test]
fn logical_device_inputs_round_trip() {
    use gob_core::{LogicalInput, LOGICAL_DEVICE_GUID};
    let mut p = sample();
    assert_eq!(p.add_logical_input(InputType::Axis), 1);
    assert_eq!(p.add_logical_input(InputType::Button), 1);
    assert_eq!(p.add_logical_input(InputType::Button), 2);
    assert!(p.rename_logical_input(InputType::Button, 2, "Fire"));
    assert!(
        !p.rename_logical_input(InputType::Axis, 1, "Fire"),
        "labels are unique"
    );
    let root = p.bind(LOGICAL_DEVICE_GUID, "button", "1", "Default");
    p.add_child(
        &root,
        "actions",
        gob_core::Action::new(&NewAction::Description),
    );
    p.remove_logical_input(InputType::Button, 1);
    let p = reload(&p);
    assert_eq!(
        p.logical_inputs(),
        [
            LogicalInput {
                input_type: InputType::Axis,
                id: 1,
                label: "Axis 1".into()
            },
            LogicalInput {
                input_type: InputType::Button,
                id: 2,
                label: "Fire".into()
            },
        ]
    );
    assert!(
        !p.inputs.iter().any(|b| b.device_id == LOGICAL_DEVICE_GUID),
        "bindings of a removed input go too"
    );
}

#[test]
fn light_led_round_trips() {
    let mut p = sample();
    let root = p.bind(DEV, "button", "3", "Default");
    let led = p
        .add_child(
            &root,
            "actions",
            gob_core::Action::new(&NewAction::LightLed {
                device: DEV.into(),
                led: 10,
            }),
        )
        .unwrap();
    let a = p.action_mut(&led).unwrap();
    a.set("color", "string", "#FF8000");
    a.set("led-mode", "string", "slow-blink");
    a.set("on-activate", "bool", "True");

    let xml = gob_gremlin::to_string(&p);
    assert!(xml.contains(r#"type="light-led""#));
    let p = reload(&p);
    let a = p.action(&led).unwrap();
    assert_eq!(
        a.kind(),
        ActionKind::LightLed {
            device: DEV,
            led: 10,
            color: [0xFF, 0x80, 0x00],
            mode: gob_core::LedMode::SlowBlink,
            on_activate: true,
        }
    );
    assert_eq!(a.activation_mode(), gob_core::ActivationMode::Press);
    assert_eq!(gob_gremlin::to_string(&p), xml, "saved again unchanged");
    assert!(gob_gremlin::report(&p)
        .warnings
        .iter()
        .all(|w| !w.to_string().contains("light-led")));
}

#[test]
fn led_colors_and_modes_parse() {
    assert_eq!(gob_core::parse_color("#00ff7F"), Some([0, 0xFF, 0x7F]));
    assert_eq!(gob_core::parse_color("12"), None);
    assert_eq!(gob_core::format_color([0, 0xFF, 0x7F]), "#00FF7F");
    for m in gob_core::LedMode::ALL {
        assert_eq!(gob_core::LedMode::parse(m.name()), Some(m));
    }
}
