use super::*;

const STICK: &str = "11111111-0000-0000-0000-000000000001";
const LINKED: &str = "11111111-0000-0000-0000-0000000000aa";
const OTHER: &str = "22222222-0000-0000-0000-000000000002";

fn prop(name: &str, value: &str) -> String {
    format!(r#"<property type="string"><name>{name}</name><value>{value}</value></property>"#)
}

fn binding(device: &str, id: u32, mode: &str, root: &str, layer: &str) -> String {
    format!(
        r#"<input><device-id>{device}</device-id><input-type>button</input-type><mode>{mode}</mode><input-id>{id}</input-id><action-configuration layer="{layer}"><root-action>{root}</root-action><behavior>button</behavior></action-configuration></input>"#
    )
}

/// An Axis from buttons moved down by `device` button `id`, under a root labeled `label`.
fn axis_down(root: &str, label: &str, device: &str, id: u32) -> String {
    let down = prop("decrease-device-id", device)
        + &prop("decrease-input-type", "button")
        + &prop("decrease-input-id", &id.to_string());
    format!(
        r#"<action id="{root}" type="root">{}<actions><action-id>{root}-in</action-id></actions></action>
            <action id="{root}-in" type="root"><actions><action-id>{root}-ax</action-id></actions></action>
            <action id="{root}-ax" type="button-axis">{down}</action>"#,
        prop("action-label", label)
    )
}

fn profile() -> Profile {
    let inputs = [
        // Default: stick button 3 (through a nested root), moved down by stick button 4.
        binding(STICK, 3, "Default", "a", ""),
        // Child: stick button 3 does something else, so its axis is not there.
        binding(STICK, 3, "Child", "plain", ""),
        // Another stick's button 1 in layer M1, moved down by stick button 5.
        binding(OTHER, 1, "Default", "b", "M1"),
        // A labeled logical input, moved down by stick button 6.
        binding(gob_core::LOGICAL_DEVICE_GUID, 2, "Default", "c", ""),
    ]
    .concat();
    let library = [
        axis_down("a", "Throttle", STICK, 4),
        axis_down("b", "", STICK, 5),
        axis_down("c", "", STICK, 6),
        r#"<action id="plain" type="root"><actions/></action>"#.into(),
    ]
    .concat();
    let xml = format!(
        r#"<profile app="Gobloon" version="1"><devices><device><device-id>{STICK}</device-id><device-name>Stick</device-name></device><device><device-id>{OTHER}</device-id><device-name>Other</device-name></device></devices>
            <inputs>{inputs}</inputs><settings/><library>{library}</library>
            <logical-device><input><input-type>button</input-type><input-id>2</input-id><label>Flaps up</label></input></logical-device>
            <modes><mode>Default</mode><mode parent="Default">Child</mode></modes></profile>"#
    );
    gob_gremlin::from_str(&xml).expect("profile")
}

fn tab(guid: &str) -> Tab<'_> {
    Tab {
        guid,
        name: "Stick",
        live: None,
        alias: None,
        logical: false,
    }
}

fn owners(p: &Profile, tab: &Tab, mode: &str) -> Vec<(u32, String, String)> {
    let i18n = I18n::new(crate::LANGUAGES, None);
    let index = p.action_index();
    let mut found: Vec<(u32, String, String)> =
        down_owners(&i18n, p, &mode_chain(p, mode), tab, &index)
            .into_iter()
            .flat_map(|((_, id), v)| v.into_iter().map(move |o| (id, o.device, o.name)))
            .collect();
    found.sort();
    found
}

#[test]
fn a_mode_chain_runs_from_the_mode_to_its_parents() {
    let p = profile();
    assert_eq!(mode_chain(&p, "Child"), ["Child", "Default"]);
    assert_eq!(mode_chain(&p, "Default"), ["Default"]);
}

#[test]
fn down_inputs_name_the_input_their_action_belongs_to() {
    let p = profile();
    let found = owners(&p, &tab(STICK), "Default");
    assert_eq!(
        found,
        [
            (4, STICK.to_string(), "Button 3 (Throttle)".to_string()),
            (5, OTHER.to_string(), "Other: Button 1, M1".to_string()),
            (
                6,
                gob_core::LOGICAL_DEVICE_GUID.to_string(),
                "Logical device: Flaps up".to_string()
            ),
        ]
    );
}

#[test]
fn a_nearer_mode_hides_the_parents_owner() {
    let p = profile();
    let found = owners(&p, &tab(STICK), "Child");
    assert!(found.iter().all(|(id, ..)| *id != 4), "{found:?}");
    assert_eq!(found.len(), 2, "the other owners are inherited: {found:?}");
}

#[test]
fn a_linked_tab_counts_its_profile_device_as_its_own() {
    let p = profile();
    let mut linked = tab(LINKED);
    linked.alias = Some(STICK);
    // The binding is under the profile device id: the same device as the tab.
    let found = owners(&p, &linked, "Default");
    assert_eq!(found[0].2, "Button 3 (Throttle)");
    // Down inputs of another device are not on this tab.
    assert!(owners(&p, &tab(OTHER), "Default").is_empty());
}
