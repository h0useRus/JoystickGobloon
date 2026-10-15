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
    assert_eq!(p.bind(DEV, "button", "7", "Default"), root, "binding twice reuses the root");
    let vjoy = p.add_child(&root, "actions", gob_core::Action::new(&NewAction::MapToVjoy { input_type: InputType::Button })).unwrap();
    p.action_mut(&vjoy).unwrap().set("vjoy-input-id", "int", "12");
    let tempo = p.add_child(&root, "actions", gob_core::Action::new(&NewAction::Tempo)).unwrap();
    p.add_child(&tempo, "long-actions", gob_core::Action::new(&NewAction::Description)).unwrap();

    let p = reload(&p);
    let v = p.action(&vjoy).unwrap();
    assert_eq!(
        v.kind(),
        ActionKind::MapToVjoy { device: 1, input_type: InputType::Button, input_id: 12, inverted: false }
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
        ["vjoy-device-id", "vjoy-input-id", "vjoy-input-type", "button-inverted", "action-label", "activation-mode"],
        "Gremlin's property order"
    );
    assert!(matches!(p.action(&tempo).unwrap().kind(), ActionKind::Tempo { long, .. } if long.len() == 1));
    assert!(gob_gremlin::report(&p).warnings.iter().all(|w| !matches!(w, gob_gremlin::Warning::DanglingRef { .. })));
}

#[test]
fn removing_children_collects_garbage() {
    let mut p = sample();
    let before = p.library.len();
    let root = p.bind(DEV, "button", "7", "Default");
    let tempo = p.add_child(&root, "actions", gob_core::Action::new(&NewAction::Tempo)).unwrap();
    p.add_child(&tempo, "short-actions", gob_core::Action::new(&NewAction::Description)).unwrap();
    assert_eq!(p.library.len(), before + 3);
    p.remove_child(&root, "actions", 0);
    assert_eq!(p.library.len(), before + 1, "tempo and its child are gone, the root stays");
    p.unbind(DEV, "button", "7", "Default");
    assert_eq!(p.library.len(), before);
}

#[test]
fn move_child_reorders() {
    let mut p = sample();
    let root = p.bind(DEV, "button", "7", "Default");
    let a = p.add_child(&root, "actions", gob_core::Action::new(&NewAction::Description)).unwrap();
    let b = p.add_child(&root, "actions", gob_core::Action::new(&NewAction::ResponseCurve)).unwrap();
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
    assert!(!p.set_mode_parent("Default", Some("Landing")), "would create a cycle");
    assert!(p.remove_mode("Combat"));
    assert_eq!(p.modes.iter().find(|m| m.name == "Landing").unwrap().parent.as_deref(), Some("Default"));
    assert!(p.inputs.iter().all(|b| b.mode != "Combat"));
    assert!(!p.remove_mode("Unknown"));
}
