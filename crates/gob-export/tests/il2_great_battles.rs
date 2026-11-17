//! The IL-2 Great Battles plugin (plugins/export/il2-great-battles) on a real key mapping file
//! made for one vJoy device (tests/data/il2-great-battles/global.actions, from the CC0
//! ControllerBuddy-Profiles project) and a devices.txt as the game writes it.

use std::collections::BTreeMap;
use std::path::Path;

use gob_export::*;

const ACTIONS: &str = include_str!("data/il2-great-battles/global.actions");
/// The game ends these lines with CR CR LF.
const DEVICES: &str = "configId,guid,model|\r\r\n0,%22d04d97a0-e9af-11f0-0000545345440180%22,vJoy%20Device|\r\r\n1,%22da66fd80-e955-11f0-0000545345440180%22,Some%20Pedals|\r\r\n2,%22da66fd81-e955-11f0-0000545345440280%22,vJoy%20Device|\r\r\n";

fn plugin() -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export/il2-great-battles");
    Plugin::load(&dir).expect("the plugin loads")
}

fn target(p: &Plugin) -> Target {
    p.targets(&Env {
        platform: "windows".into(),
        roots: vec!["D:\\Games".into()],
    })
    .unwrap()
    .into_iter()
    .find(|t| t.id == "D:/Games/IL-2 Sturmovik Great Battles")
    .expect("the install under the root")
}

fn files(t: &Target, actions: Option<&str>, devices: Option<&str>) -> Files {
    let file = |key: &str, text: Option<&str>| {
        (
            key.to_string(),
            GameFile {
                path: t.files[key].clone(),
                text: text.map(str::to_string),
            },
        )
    };
    [file("actions", actions), file("devices", devices)].into()
}

fn ours(n: u32) -> Vec<OurDevice> {
    (1..=n)
        .map(|i| OurDevice {
            output: format!("vjoy:{i}"),
            name: "vJoy Device".into(),
            guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
        })
        .collect()
}

fn bind(action: &str, output: &str) -> Bind {
    Bind {
        action: action.into(),
        output: output.into(),
        invert: false,
        default: false,
    }
}

fn read(p: &Plugin, t: &Target, actions: &str) -> ReadResult {
    p.read(&ReadContext {
        target: t,
        files: &files(t, Some(actions), Some(DEVICES)),
        catalog: p.catalogs.pick(None).unwrap().0,
        ours: &ours(2),
    })
    .unwrap()
}

fn write(
    p: &Plugin,
    t: &Target,
    actions: &str,
    binds: &[Bind],
    cleared: &[String],
) -> Result<WriteResult, PluginError> {
    p.write(&WriteContext {
        target: t,
        files: &files(t, Some(actions), Some(DEVICES)),
        catalog: p.catalogs.pick(None).unwrap().0,
        ours: &ours(2),
        devices: &[],
        binds,
        cleared,
        profile: "p",
        options: &BTreeMap::new(),
    })
}

/// The bindings on the virtual joysticks, as an export gives them back.
fn joystick(read: &ReadResult) -> Vec<Bind> {
    read.binds
        .iter()
        .filter(|b| b.output.starts_with("vjoy:"))
        .cloned()
        .collect()
}

#[test]
fn finds_the_install_and_its_files() {
    let p = plugin();
    let t = target(&p);
    assert_eq!(
        t.files["actions"],
        "D:/Games/IL-2 Sturmovik Great Battles/data/input/global.actions"
    );
    assert_eq!(
        t.files["devices"],
        "D:/Games/IL-2 Sturmovik Great Battles/data/input/devices.txt"
    );
    // A folder chosen by the user is the install itself.
    let chosen = p
        .targets(&Env {
            platform: "windows".into(),
            roots: vec!["E:/IL2".into()],
        })
        .unwrap();
    assert!(chosen.iter().any(|t| t.id == "E:/IL2"));
}

#[test]
fn reads_devices_and_bindings() {
    let p = plugin();
    let t = target(&p);
    let read = read(&p, &t, ACTIONS);
    // The virtual joysticks by name, in the game's order: its devices 0 and 2.
    assert_eq!(
        read.devices,
        [
            DeviceLink {
                output: "vjoy:1".into(),
                game_device: "joy0".into()
            },
            DeviceLink {
                output: "vjoy:2".into(),
                game_device: "joy2".into()
            },
        ]
    );
    let has = |action: &str, output: &str, invert: bool| {
        read.binds.contains(&Bind {
            invert,
            ..bind(action, output)
        })
    };
    // Buttons count from 0 in the game, from 1 here; axes by the game's letters.
    assert!(has("bc_eject", "vjoy:1:button:4", false));
    assert!(has("rpc_pitch", "vjoy:1:axis:y", false));
    assert!(has("rpc_yaw", "vjoy:1:axis:rz", false));
    assert!(has("rpc_all_engines_throttles", "vjoy:1:axis:z", true));
    // An axis from two buttons: its (+) and (-) halves.
    assert!(has("rpc_pitch_trim:inc", "vjoy:1:button:27", false));
    assert!(has("rpc_pitch_trim:dec", "vjoy:1:button:26", false));
    // Keys, several together, and the two keys of an axis.
    assert!(has("gear", "key:0x22", false));
    assert!(has("record", "key:0x1d+key:0x13", false));
    assert!(has("rpc_all_engines_throttles:inc", "key:0xd", false));
    assert!(has("rpc_all_engines_throttles:dec", "key:0xc", false));
    assert!(read.binds.iter().all(|b| !b.default));
    // Every action read is one the catalog knows.
    let catalog = p.catalogs.pick(None).unwrap().0;
    for b in &read.binds {
        assert!(catalog.action(&b.action).is_some(), "{}", b.action);
    }
    assert_eq!(catalog.action("rpc_pitch").unwrap().kind, ActionKind::Axis);
    assert_eq!(catalog.action("gear").unwrap().category, "Plane controls");
}

#[test]
fn writing_what_was_read_changes_nothing() {
    let p = plugin();
    let t = target(&p);
    let binds = joystick(&read(&p, &t, ACTIONS));
    assert!(binds.len() > 50, "{}", binds.len());
    let out = write(&p, &t, ACTIONS, &binds, &[]).unwrap();
    assert!(out.changes.is_empty(), "{:?}", out.changes);
    assert_eq!(out.files["actions"], ACTIONS);
}

#[test]
fn writes_joystick_bindings_and_keeps_the_rest() {
    let p = plugin();
    let t = target(&p);
    let mut binds = joystick(&read(&p, &t, ACTIONS));
    // Eject moves to another button, the gear gets a button on the second joystick and a hat,
    // the elevator trim loses its buttons, the rudder trim gets two.
    binds.retain(|b| b.action != "bc_eject" && !b.action.starts_with("rpc_pitch_trim"));
    binds.retain(|b| !b.action.starts_with("rpc_yaw_trim"));
    binds.push(bind("bc_eject", "vjoy:1:button:40"));
    binds.push(bind("gear", "vjoy:2:button:1"));
    binds.push(bind("gear", "vjoy:1:hat:1:down"));
    binds.push(bind("rpc_yaw_trim:inc", "vjoy:2:button:7"));
    binds.push(bind("rpc_yaw_trim:dec", "vjoy:2:button:8"));
    let out = write(&p, &t, ACTIONS, &binds, &[]).unwrap();
    let text = &out.files["actions"];
    let line = |action: &str, command: &str, invert: u8, comment: &str| {
        let mut l = format!(
            "{:<50}{:<50}{invert}|",
            format!("{action},"),
            format!("{command},")
        );
        if !comment.is_empty() {
            l.push_str(" // ");
            l.push_str(comment);
        }
        l
    };
    assert!(
        text.contains(&line("bc_eject", "joy0_b39", 0, "")),
        "{text}"
    );
    assert!(!text.contains("joy0_b3,"), "{text}");
    // The gear's key stays first, with the game's text; ours follow.
    let gear = [
        line("gear", "key_g", 0, "Gear up/down"),
        line("gear", "joy2_b0", 0, ""),
        line("gear", "joy0_pov0_180", 0, ""),
    ]
    .join("\n");
    assert!(text.contains(&gear), "{text}");
    // The trim keeps its keys and loses the buttons.
    assert!(text.contains("key_rcontrol+key_down/key_rcontrol+key_up"));
    assert!(
        !text.contains(&line("rpc_pitch_trim", "joy0_b26/joy0_b25", 0, "")),
        "{text}"
    );
    assert!(
        text.contains(&line("rpc_yaw_trim", "joy2_b6/joy2_b7", 0, "")),
        "{text}"
    );
    // The inverted throttle axis is untouched.
    assert!(text.contains(&line("rpc_all_engines_throttles", "joy0_axis_z", 1, "")));
    let changed: Vec<_> = out
        .changes
        .iter()
        .map(|c| (c.kind, c.action.as_str()))
        .collect();
    assert_eq!(
        changed,
        [
            // In the order of the game's file.
            (ChangeKind::Changed, "rpc_yaw_trim"),
            (ChangeKind::Removed, "rpc_pitch_trim"),
            (ChangeKind::Added, "gear"),
            (ChangeKind::Changed, "bc_eject"),
        ]
    );
    // Only those actions' lines differ.
    let differ = ACTIONS
        .lines()
        .filter(|l| !text.lines().any(|n| n == *l))
        .count();
    assert_eq!(differ, 3, "{text}");
    // Read back, the same bindings.
    let back = joystick(&read(&p, &t, text));
    for b in &binds {
        assert!(back.contains(b), "{b:?}");
    }
    assert_eq!(back.len(), binds.len());
}

#[test]
fn writes_keys_only_for_the_actions_held() {
    let p = plugin();
    let t = target(&p);
    let mut binds = joystick(&read(&p, &t, ACTIONS));
    // The gear on Right Alt + G and the middle mouse button; flaps up without any key.
    binds.push(bind("gear", "key:0x138+key:0x22"));
    binds.push(bind("gear", "mouse:middle"));
    let cleared = ["flaps_up".to_string()];
    let out = write(&p, &t, ACTIONS, &binds, &cleared).unwrap();
    let text = &out.files["actions"];
    assert!(text.contains("key_rmenu+key_g,"), "{text}");
    assert!(text.contains("mouse_b2,"), "{text}");
    assert!(!text.contains("                key_g,"), "{text}");
    assert!(!text.contains("key_lshift+key_f,"), "{text}");
    // Flaps up keeps its joystick button, now on the line with the game's text.
    assert!(text.contains("joy0_b14,                                         0| // Flaps up"));
    // Other actions' keys are the game's: untouched.
    assert!(text.contains("key_lmenu+key_g,"));
    let changed: Vec<_> = out
        .changes
        .iter()
        .map(|c| (c.kind, c.action.as_str()))
        .collect();
    assert_eq!(
        changed,
        [
            (ChangeKind::Changed, "flaps_up"),
            (ChangeKind::Changed, "gear")
        ]
    );
}

#[test]
fn tells_what_is_missing() {
    let p = plugin();
    let t = target(&p);
    // A joystick the game does not list yet.
    let e = write(&p, &t, ACTIONS, &[bind("gear", "vjoy:3:button:1")], &[]).unwrap_err();
    assert!(e.to_string().contains("does not list"), "{e}");
    // No key mapping file yet.
    let e = p
        .write(&WriteContext {
            target: &t,
            files: &files(&t, None, None),
            catalog: p.catalogs.pick(None).unwrap().0,
            ours: &ours(1),
            devices: &[],
            binds: &[],
            cleared: &[],
            profile: "p",
            options: &BTreeMap::new(),
        })
        .unwrap_err();
    assert!(e.to_string().contains("Key mapping"), "{e}");
}
