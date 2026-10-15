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
