use super::*;
use gob_input::DeviceInfo;

fn live(guid: &str, name: &str) -> DeviceState {
    DeviceState::new(DeviceInfo {
        guid: guid.into(),
        name: name.into(),
        vendor_id: 0x231d,
        product_id: 0x0200,
        axes: vec![1],
        buttons: 1,
        hats: 0,
        ..Default::default()
    })
}

#[test]
fn similar_names() {
    let nxt_l = "VKBsim Gladiator NXT EVO OT L";
    assert!(name_score(nxt_l, "VKBsim Gladiator EVO OT L").is_some());
    assert_eq!(
        name_score(nxt_l, "VKBsim Gladiator EVO R"),
        None,
        "other side"
    );
    assert_eq!(name_score("Thrustmaster T.16000M", "Saitek X52"), None);
    assert!(
        name_score(nxt_l, "VKBsim Gladiator EVO OT L") > name_score(nxt_l, "VKBsim Gladiator L")
    );
}

#[test]
fn auto_map_skips_ambiguous_and_mapped_rows() {
    let profile = Profile {
        devices: vec![
            gob_core::Device {
                id: "p1".into(),
                name: "VKBsim Gladiator NXT L".into(),
            },
            gob_core::Device {
                id: "p2".into(),
                name: "Generic Throttle".into(),
            },
            gob_core::Device {
                id: "p3".into(),
                name: "MFG Crosswind".into(),
            },
        ],
        ..Default::default()
    };
    let devices = [
        live("d1", "VKBsim Gladiator EVO L"),
        live("d2", "Generic Throttle A"),
        live("d3", "Generic Throttle B"),
    ];
    let mut dialog = SubstituteDialog::new(&profile, &devices);
    assert!(dialog.rows.iter().all(|r| r.target.is_none()));
    assert_eq!(dialog.auto_map(), 1);
    assert_eq!(dialog.rows[0].target.as_deref(), Some("d1"));
    assert_eq!(dialog.rows[1].target, None, "two equally similar throttles");
    assert_eq!(dialog.rows[2].target, None, "no similar device");
}

#[test]
fn maps_onto_devices_the_profile_already_knows() {
    // Gremlin on this PC already added the attached sticks; old entries remain.
    let dev = |id: &str, name: &str| gob_core::Device {
        id: id.into(),
        name: name.into(),
    };
    let profile = Profile {
        devices: vec![
            dev("old-l", " VKBsim Gladiator EVO OT L  "),
            dev("old-r", " VKBsim Gladiator EVO  R  "),
            dev("new-l", " VKBsim Gladiator EVO OT L  "),
            dev("new-r", " VKBsim Gladiator EVO R  "),
        ],
        ..Default::default()
    };
    let devices = [
        live("new-l", "VKBsim Gladiator EVO OT L"),
        live("new-r", "VKBsim Gladiator EVO R"),
    ];
    let mut dialog = SubstituteDialog::new(&profile, &devices);
    for r in &mut dialog.rows {
        r.target = None;
    }
    assert_eq!(dialog.auto_map(), 2);
    assert_eq!(dialog.rows[0].target.as_deref(), Some("new-l"));
    assert_eq!(dialog.rows[1].target.as_deref(), Some("new-r"));
}

#[test]
fn aliases_profile_devices_by_name() {
    let profile = Profile {
        devices: vec![
            gob_core::Device {
                id: "7D12D5C0-43EA-11F0-800A-444553540000".into(),
                name: " VKBsim Gladiator EVO OT L  ".into(),
            },
            gob_core::Device {
                id: "ec8bbeb0-4009-11f0-8002-444553540000".into(),
                name: " VKBsim Gladiator EVO  R  ".into(),
            },
        ],
        ..Default::default()
    };
    let devices = [
        live(
            "a2196530-a616-11f0-8012-444553540000",
            "VKBsim Gladiator EVO OT L",
        ),
        live(
            "a21805a0-a616-11f0-8006-444553540000",
            "VKBsim Gladiator EVO R",
        ),
    ];
    let aliases = auto_aliases(&profile, &devices);
    assert_eq!(aliases.len(), 2);
    assert_eq!(
        aliases["7d12d5c0-43ea-11f0-800a-444553540000"],
        "a2196530-a616-11f0-8012-444553540000"
    );
    assert_eq!(
        aliases["ec8bbeb0-4009-11f0-8002-444553540000"],
        "a21805a0-a616-11f0-8006-444553540000"
    );
}
