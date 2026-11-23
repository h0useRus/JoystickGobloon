//! The X4: Foundations plugin (plugins/export/x4-foundations) on an inputmap.xml written for these
//! tests in the game's format (tests/data/x4-foundations): slot 1 is vJoy 1 (pinned in config.xml by
//! its instance GUID), slot 2 another stick; Shift+A and a latched Shift+X, a half axis and a hat
//! diagonal.

use std::collections::BTreeMap;
use std::path::Path;

use gob_export::*;

const MAP: &str = include_str!("data/x4-foundations/inputmap.xml");
const CONFIG: &str = include_str!("data/x4-foundations/config.xml");
const BASE: &str = "C:/Users/h0use/Documents/Egosoft/X4";
const ACCOUNT: &str = "C:/Users/h0use/Documents/Egosoft/X4/12345678";

fn plugin() -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export/x4-foundations");
    Plugin::load(&dir).expect("the plugin loads")
}

fn target(p: &Plugin) -> Target {
    p.targets(&Env {
        platform: "windows".into(),
        roots: vec!["C:/".into()],
        documents: "C:/Users/h0use/Documents/".into(),
        local_app_data: String::new(),
    })
    .unwrap()
    .into_iter()
    .find(|t| t.id == BASE)
    .expect("the X4 folder")
}

fn map_key() -> String {
    format!("maps:{ACCOUNT}/inputmap.xml")
}

/// The files as the host gives them: one Steam account's folder.
fn files(t: &Target, map: &str, config: Option<&str>) -> Files {
    let mut files = Files::new();
    for (key, path, text) in [
        ("map", t.files["map"].clone(), None),
        ("config", t.files["config"].clone(), None),
        (&map_key()[..], format!("{ACCOUNT}/inputmap.xml"), Some(map)),
    ] {
        files.insert(
            key.to_string(),
            GameFile {
                path,
                text: text.map(str::to_string),
            },
        );
    }
    if let Some(config) = config {
        files.insert(
            format!("configs:{ACCOUNT}/config.xml"),
            GameFile {
                path: format!("{ACCOUNT}/config.xml"),
                text: Some(config.to_string()),
            },
        );
    }
    files
}

/// vJoy 1 is the device config.xml pins to slot 1; vJoy 2 is not pinned.
fn ours() -> Vec<OurDevice> {
    vec![
        OurDevice {
            output: "vjoy:1".into(),
            name: "vJoy Device".into(),
            guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
            instance: "{42CEC570-0A3F-11F0-800B-444553540000}".into(),
        },
        OurDevice {
            output: "vjoy:2".into(),
            name: "vJoy Device".into(),
            guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
            instance: "{99999999-0A3F-11F0-800B-444553540000}".into(),
        },
    ]
}

fn bind(action: &str, output: &str) -> Bind {
    Bind {
        action: action.into(),
        output: output.into(),
        invert: false,
        default: false,
    }
}

fn read(p: &Plugin, t: &Target, map: &str) -> ReadResult {
    p.read(&ReadContext {
        target: t,
        files: &files(t, map, Some(CONFIG)),
        catalog: p.catalogs.pick(None).unwrap().0,
        ours: &ours(),
        devices: &[],
    })
    .unwrap()
}

fn write(
    p: &Plugin,
    t: &Target,
    config: Option<&str>,
    devices: &[DeviceLink],
    binds: &[Bind],
    cleared: &[String],
) -> Result<WriteResult, PluginError> {
    p.write(&WriteContext {
        target: t,
        files: &files(t, MAP, config),
        catalog: p.catalogs.pick(None).unwrap().0,
        ours: &ours(),
        devices,
        binds,
        cleared,
        profile: "p",
        options: &BTreeMap::new(),
    })
}

fn pairs(binds: &[Bind]) -> Vec<(&str, &str)> {
    let mut list: Vec<(&str, &str)> = binds
        .iter()
        .map(|b| (b.action.as_str(), b.output.as_str()))
        .collect();
    list.sort();
    list
}

#[test]
fn finds_the_account_folders() {
    let p = plugin();
    let t = target(&p);
    assert_eq!(t.files["map"], format!("{BASE}/inputmap.xml"));
    assert_eq!(t.globs["maps"], format!("{BASE}/*/inputmap.xml"));
    assert_eq!(t.globs["configs"], format!("{BASE}/*/config.xml"));
    let linux = p
        .targets(&Env {
            platform: "linux".into(),
            roots: vec!["/home/h0use".into()],
            documents: String::new(),
            local_app_data: String::new(),
        })
        .unwrap();
    assert!(
        linux
            .iter()
            .any(|t| t.id == "/home/h0use/.config/EgoSoft/X4")
    );
}

#[test]
fn the_catalog_lists_ranges_as_axes() {
    let p = plugin();
    let (catalog, _) = p.catalogs.pick(None).unwrap();
    let kind = |id: &str| catalog.actions.iter().find(|a| a.id == id).map(|a| a.kind);
    assert_eq!(kind("INPUT_RANGE_THROTTLE"), Some(ActionKind::Axis));
    assert_eq!(kind("INPUT_STATE_BOOST"), Some(ActionKind::Button));
    assert!(!catalog.actions.iter().any(|a| a.id.contains("DEBUG")));
}

#[test]
fn reads_the_pinned_slots_bindings() {
    let p = plugin();
    let t = target(&p);
    let r = read(&p, &t, MAP);
    assert_eq!(
        r.devices
            .iter()
            .map(|d| (d.output.as_str(), d.game_device.as_str()))
            .collect::<Vec<_>>(),
        [("vjoy:1", "joy1")]
    );
    assert_eq!(
        pairs(&r.binds),
        [
            ("INPUT_ACTION_TOGGLE_AUTOPILOT", "key:0x2a+key:0x1e"),
            ("INPUT_ACTION_TOGGLE_AUTOPILOT", "vjoy:1:button:14"),
            ("INPUT_RANGE_STEERING_PRIMARY", "vjoy:1:axis:x"),
            ("INPUT_STATE_FIRE_PRIMARY_WEAPON", "vjoy:1:button:1"),
            ("INPUT_STATE_MATCH_SPEED", "key:0x2a+key:0x2d"),
        ]
    );
}

#[test]
fn writes_the_profiles_lines_and_keeps_the_rest() {
    let p = plugin();
    let t = target(&p);
    let binds = [
        // Autopilot moves from button 14 to button 3 (X); Shift+A stays the game's.
        bind("INPUT_ACTION_TOGGLE_AUTOPILOT", "vjoy:1:button:3"),
        // Fire stays on button 1.
        bind("INPUT_STATE_FIRE_PRIMARY_WEAPON", "vjoy:1:button:1"),
        // Pitch on the Y axis, boost on hat 1 up, a quicksave on button 30.
        bind("INPUT_RANGE_STEERING_PITCH", "vjoy:1:axis:y"),
        bind("INPUT_STATE_BOOST", "vjoy:1:hat:1:up"),
        bind("INPUT_ACTION_QUICKSAVE", "vjoy:1:button:30"),
    ];
    let w = write(&p, &t, Some(CONFIG), &[], &binds, &[]).unwrap();
    let text = &w.files[&map_key()];
    let has = |line: &str| text.contains(line);
    assert!(
        has(
            r#"<action id="INPUT_ACTION_TOGGLE_AUTOPILOT" source="INPUT_SOURCE_JOYBUTTONS" code="INPUT_XBUTTON_X"/>"#
        ),
        "{text}"
    );
    assert!(!has("INPUT_XBUTTON_14"), "{text}");
    assert!(has(r#"code="INPUT_KEYCODE_A_SHIFT""#));
    assert!(has(
        r#"<state id="INPUT_STATE_FIRE_PRIMARY_WEAPON" source="INPUT_SOURCE_JOYBUTTONS" code="INPUT_XBUTTON_A"/>"#
    ));
    assert!(
        has(
            r#"<range id="INPUT_RANGE_STEERING_PITCH" source="INPUT_SOURCE_JOYAXES" code="INPUT_JOYAXIS_Y"/>"#
        ),
        "{text}"
    );
    assert!(has(
        r#"<state id="INPUT_STATE_BOOST" source="INPUT_SOURCE_JOYBUTTONS" code="INPUT_XBUTTON_DPAD_UP"/>"#
    ));
    assert!(has(
        r#"<action id="INPUT_ACTION_QUICKSAVE" source="INPUT_SOURCE_JOYBUTTONS" code="INPUT_XBUTTON_30"/>"#
    ));
    // The steering axis is gone from the profile; slot 2, the half axis, the diagonal and the
    // latched keys stay.
    assert!(!has("INPUT_RANGE_STEERING_PRIMARY"), "{text}");
    assert!(has(
        r#"source="INPUT_SOURCE_JOYBUTTONS_2" code="INPUT_XBUTTON_BACK""#
    ));
    assert!(has(r#"code="INPUT_JOYAXIS_SLIDER1" sgn="1""#));
    assert!(has("INPUT_XBUTTON_DPAD_UP_LEFT"));
    assert!(has(r#"toggle="1" code="INPUT_KEYCODE_X_SHIFT""#));
    // New lines sit in their section: the pitch range among the ranges, the quicksave among the
    // actions, each indented like its neighbours.
    let at = |s: &str| text.find(s).unwrap();
    assert!(at("INPUT_RANGE_STEERING_PITCH") > at("<!-- input ranges -->"));
    assert!(at("INPUT_ACTION_QUICKSAVE") < at("<!-- input states -->"));
    assert!(has("\n  <action id=\"INPUT_ACTION_QUICKSAVE\""));
    assert!(text.contains("<param name=\"invert_steering_pitch\" value=\"true\"/>"));
    // Read back: what was written.
    let back = read(&p, &t, text);
    let back = pairs(&back.binds);
    assert!(back.contains(&("INPUT_STATE_BOOST", "vjoy:1:hat:1:up")));
    assert!(back.contains(&("INPUT_ACTION_QUICKSAVE", "vjoy:1:button:30")));
    assert!(!back.contains(&("INPUT_RANGE_STEERING_PRIMARY", "vjoy:1:axis:x")));
    let changed: Vec<&str> = w.changes.iter().map(|c| c.action.as_str()).collect();
    assert!(changed.contains(&"INPUT_ACTION_TOGGLE_AUTOPILOT"));
    assert!(
        !changed.contains(&"INPUT_STATE_FIRE_PRIMARY_WEAPON"),
        "{changed:?}"
    );
}

#[test]
fn keys_of_controls_held_here_replace_the_games() {
    let p = plugin();
    let t = target(&p);
    // The profile holds the autopilot's keys: Shift+A goes, Ctrl+Shift+P comes.
    let w = write(
        &p,
        &t,
        Some(CONFIG),
        &[],
        &[bind(
            "INPUT_ACTION_TOGGLE_AUTOPILOT",
            "key:0x1d+key:0x2a+key:0x19",
        )],
        &[],
    )
    .unwrap();
    let text = &w.files[&map_key()];
    assert!(
        text.contains(r#"code="INPUT_KEYCODE_P_SHIFT_CONTROL""#),
        "{text}"
    );
    assert!(!text.contains("INPUT_KEYCODE_A_SHIFT"));
    // Match speed is not held here: its latched Shift+X stays.
    assert!(text.contains("INPUT_KEYCODE_X_SHIFT"));
}

#[test]
fn slots_typed_by_the_user_work_without_config() {
    let p = plugin();
    let t = target(&p);
    let links = [DeviceLink {
        output: "vjoy:2".into(),
        game_device: "joy3".into(),
    }];
    let w = write(
        &p,
        &t,
        None,
        &links,
        &[bind("INPUT_RANGE_THROTTLE", "vjoy:2:axis:slider")],
        &[],
    )
    .unwrap();
    let text = &w.files[&map_key()];
    assert!(text.contains(r#"<range id="INPUT_RANGE_THROTTLE" source="INPUT_SOURCE_JOYAXES_3" code="INPUT_JOYAXIS_SLIDER1"/>"#), "{text}");
    // Without config.xml or links, the slot is unknown.
    let err = write(
        &p,
        &t,
        None,
        &[],
        &[bind("INPUT_RANGE_THROTTLE", "vjoy:1:axis:z")],
        &[],
    )
    .unwrap_err();
    assert!(err.to_string().contains("Manage controllers"), "{err}");
}
