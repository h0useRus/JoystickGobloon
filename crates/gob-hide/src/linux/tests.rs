use super::*;

fn info(guid: &str, name: &str) -> DeviceInfo {
    DeviceInfo {
        guid: guid.into(),
        name: name.into(),
        vendor_id: 0x231d,
        product_id: 0x0200,
        ..DeviceInfo::default()
    }
}

#[test]
fn state_file_and_device_list() {
    assert_eq!(parse("").unwrap(), State::default(), "on, nothing hidden");
    let attached = [info("aaaa", "EVO L"), info("bbbb", "EVO R")];
    let devices = list(&attached, &State::default());
    assert_eq!(devices.len(), 2);
    assert!(devices.iter().all(|d| d.present && !d.hidden));

    let state = changed(State::default(), &devices[1], true).unwrap();
    let text = toml::to_string_pretty(&state).unwrap();
    assert_eq!(parse(&text).unwrap(), state);
    let devices = list(&attached, &state);
    assert!(!devices[0].hidden && devices[1].hidden);
    assert_eq!(devices[1].parts[0].vid_pid, Some((0x231d, 0x0200)));

    // Unplugged: still listed, as hidden and not present.
    let devices = list(&attached[..1], &state);
    assert_eq!(devices.len(), 2);
    assert!(!devices[1].present && devices[1].hidden);
    assert_eq!(devices[1].name, "EVO R");

    let state = changed(state, &devices[1], false).unwrap();
    assert!(state.devices.is_empty());
}

#[test]
fn hidden_device_with_a_readable_raw_node_is_exposed() {
    // Any file the user can read stands in for an open hidraw node.
    let open = std::env::current_exe().unwrap().display().to_string();
    let attached = [
        DeviceInfo {
            hid_path: open.clone(),
            ..info("aaaa", "EVO L")
        },
        DeviceInfo {
            hid_path: "/nonexistent/hidraw0".into(),
            ..info("bbbb", "EVO R")
        },
        DeviceInfo {
            hid_path: open,
            ..info("cccc", "Pad")
        },
    ];
    let mut state = State::default();
    for d in &list(&attached, &state)[..2] {
        state = changed(state, d, true).unwrap();
    }
    let devices = list(&attached, &state);
    assert!(devices[0].exposed, "hidden, node open to the user");
    assert!(!devices[1].exposed, "hidden, no node");
    assert!(!devices[2].exposed, "not hidden");
    assert!(!readable(""));
}

#[test]
fn raw_nodes_closed_for_open_or_closed_hidden_sticks() {
    let open = std::env::current_exe().unwrap().display().to_string();
    let stick = |guid: &str, product, hid_path: &str| DeviceInfo {
        hid_path: hid_path.into(),
        product_id: product,
        ..info(guid, "Stick")
    };
    let attached = [
        stick("aaaa", 1, &open),
        stick("bbbb", 2, "/nonexistent/hidraw0"),
        stick("cccc", 3, &open),
    ];
    let mut state = State::default();
    for d in &list(&attached, &state)[..2] {
        state = changed(state, d, true).unwrap();
    }
    assert_eq!(
        closed_sticks(&state, &attached, &[]),
        [(0x231d, 1)],
        "hidden and open"
    );
    assert_eq!(
        closed_sticks(&state, &attached, &[(0x231d, 2), (0x231d, 3)]),
        [(0x231d, 1), (0x231d, 2)],
        "closed stays closed while hidden; shown opens again"
    );
    assert_eq!(
        closed_sticks(&state, &[], &[(0x231d, 1)]),
        [(0x231d, 1)],
        "unplugged"
    );
    state.enabled = false;
    assert!(
        closed_sticks(&state, &attached, &[(0x231d, 1)]).is_empty(),
        "hiding off"
    );
}
