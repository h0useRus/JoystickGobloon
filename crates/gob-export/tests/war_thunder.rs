//! The War Thunder plugin (plugins/export/war-thunder) on controls files players exported from the
//! game and shared publicly (tests/data/war-thunder): vjoy.blk has a vJoy device (25 buttons, one
//! hat, eight axes) as the first joystick of its device list, hotas.blk a stick and a throttle.

use std::collections::BTreeMap;
use std::path::Path;

use gob_export::*;

const VJOY: &str = include_str!("data/war-thunder/vjoy.blk");
const HOTAS: &str = include_str!("data/war-thunder/hotas.blk");
const DOCS: &str = "C:/Users/h0use/Documents/My Games/WarThunder";

fn plugin() -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export/war-thunder");
    Plugin::load(&dir).expect("the plugin loads")
}

fn target(p: &Plugin) -> Target {
    let mut list = p
        .targets(&Env {
            platform: "windows".into(),
            roots: vec!["C:/".into()],
            documents: "C:/Users/h0use/Documents/".into(),
            local_app_data: "C:/Users/h0use/AppData/Local/".into(),
        })
        .unwrap();
    assert_eq!(list.len(), 1);
    list.remove(0)
}

/// The exported file as the host gives it: found by one of the target's globs.
fn files(t: &Target, text: &str) -> Files {
    let key = t
        .globs
        .iter()
        .find(|(_, g)| g.as_str() == format!("{DOCS}/*.blk"))
        .map(|(k, _)| k.clone())
        .expect("a glob for the game's Documents folder");
    let mut files = Files::new();
    files.insert(
        format!("{key}:{DOCS}/controls.blk"),
        GameFile {
            path: format!("{DOCS}/controls.blk"),
            text: Some(text.to_string()),
        },
    );
    files
}

fn ours() -> Vec<OurDevice> {
    vec![OurDevice {
        output: "vjoy:1".into(),
        name: "vJoy Device".into(),
        guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
        axes: ["x", "y", "z", "rx", "ry", "rz", "slider", "dial"]
            .iter()
            .map(|a| a.to_string())
            .collect(),
        buttons: 25,
        hats: 1,
        ..Default::default()
    }]
}

fn bind(action: &str, output: &str) -> Bind {
    Bind {
        action: action.into(),
        output: output.into(),
        invert: false,
        default: false,
    }
}

fn read(p: &Plugin, t: &Target, text: &str) -> ReadResult {
    p.read(&ReadContext {
        target: t,
        files: &files(t, text),
        catalog: p.catalogs.pick(None).unwrap().0,
        ours: &ours(),
        devices: &[],
    })
    .unwrap()
}

fn write(
    p: &Plugin,
    t: &Target,
    text: &str,
    devices: &[DeviceLink],
    binds: &[Bind],
) -> Result<WriteResult, PluginError> {
    p.write(&WriteContext {
        target: t,
        files: &files(t, text),
        catalog: p.catalogs.pick(None).unwrap().0,
        ours: &ours(),
        devices,
        binds,
        cleared: &[],
        profile: "p",
        options: &BTreeMap::new(),
    })
}

fn written(r: &WriteResult) -> String {
    r.files
        .values()
        .next()
        .expect("the file is written")
        .clone()
}

fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[test]
fn looks_in_documents_and_the_game_folders() {
    let p = plugin();
    let t = target(&p);
    let globs: Vec<&str> = t.globs.values().map(String::as_str).collect();
    for want in [
        "C:/Users/h0use/Documents/My Games/WarThunder/*.blk",
        "C:/Users/h0use/AppData/Local/WarThunder/*.blk",
        "C:/Program Files (x86)/Steam/steamapps/common/War Thunder/*.blk",
    ] {
        assert!(globs.contains(&want), "{want} in {globs:?}");
    }
    assert!(t.glob_text.values().all(|text| text == "controls{"));
}

#[test]
fn reads_the_vjoy_device_and_keys() {
    let p = plugin();
    let t = target(&p);
    let r = read(&p, &t, VJOY);
    assert_eq!(
        r.devices,
        vec![DeviceLink {
            output: "vjoy:1".into(),
            game_device: "joystick 1".into(),
        }]
    );
    let has = |action: &str, output: &str| {
        r.binds
            .iter()
            .any(|b| b.action == action && b.output == output)
    };
    // The vJoy device's axes start at 7: 10 is its fourth axis, Rx.
    assert!(has("human_uav_pitch", "vjoy:1:axis:rx"), "{:?}", r.binds);
    assert!(has("human_uav_throttle", "vjoy:1:axis:slider"));
    // An axis of another device is not ours.
    assert!(!r.binds.iter().any(|b| b.action == "elevator"));
    // Keys: DirectInput codes as our scan codes.
    assert!(r.binds.iter().any(|b| b.output.starts_with("key:0x")));
}

#[test]
fn an_untouched_file_comes_back_as_it_was() {
    let p = plugin();
    let t = target(&p);
    // The profile holds what the file has: nothing changes.
    let binds = read(&p, &t, VJOY).binds;
    let r = write(&p, &t, VJOY, &[], &binds).unwrap();
    assert_eq!(lf(&written(&r)), lf(VJOY));
    assert!(r.changes.is_empty(), "{:?}", r.changes);
    // hotas.blk has no vJoy device: linked by hand to a stick of the file, with no bindings
    // on it, nothing changes either.
    let r = write(
        &p,
        &t,
        HOTAS,
        &[DeviceLink {
            output: "vjoy:1".into(),
            game_device: "joystick 6".into(),
        }],
        &[],
    )
    .unwrap();
    assert_eq!(lf(&written(&r)), lf(HOTAS));
}

#[test]
fn writes_buttons_hats_axes_and_keys() {
    let p = plugin();
    let t = target(&p);
    let r = write(
        &p,
        &t,
        VJOY,
        &[],
        &[
            bind("ID_GEAR", "vjoy:1:button:5"),
            bind("ID_FLAPS_DOWN", "vjoy:1:hat:1:down"),
            bind("ID_AIR_BRAKE", "key:0x2a+key:0x137"),
            Bind {
                invert: true,
                ..bind("elevator", "vjoy:1:axis:y")
            },
        ],
    )
    .unwrap();
    let text = lf(&written(&r));
    // vJoy buttons start at 28: button 5 is 32, the hat's down (after 25 buttons) 28 + 25 + 2.
    assert!(
        text.contains("    ID_GEAR{\n      joyButton:i=32\n    }"),
        "{text}"
    );
    assert!(text.contains("    ID_FLAPS_DOWN{\n      joyButton:i=55\n    }"));
    // Shift + SysRq: DirectInput 42 and 0xB7.
    assert!(
        text.contains("    ID_AIR_BRAKE{\n      keyboardKey:i=42\n      keyboardKey:i=183\n    }")
    );
    // The elevator moves from the other device's axis 0 to vJoy's Y (7 + 1), inverted.
    let elevator = text.split("    elevator{\n").nth(1).unwrap();
    let elevator = &elevator[..elevator.find("    }").unwrap()];
    assert!(elevator.contains("axisId:i=8"), "{elevator}");
    assert!(elevator.contains("inverse:b=yes"));
    assert!(r.changes.iter().any(|c| c.action == "ID_GEAR"));
    // Read back, the same bindings.
    let back = read(&p, &t, &text);
    for (action, output) in [
        ("ID_GEAR", "vjoy:1:button:5"),
        ("ID_FLAPS_DOWN", "vjoy:1:hat:1:down"),
        ("ID_AIR_BRAKE", "key:0x2a+key:0x137"),
        ("elevator", "vjoy:1:axis:y"),
    ] {
        assert!(
            back.binds
                .iter()
                .any(|b| b.action == action && b.output == output),
            "{action} {output}: {:?}",
            back.binds
        );
    }
}

#[test]
fn removes_its_own_bindings_and_leaves_others() {
    let p = plugin();
    let t = target(&p);
    let first = write(&p, &t, VJOY, &[], &[bind("ID_GEAR", "vjoy:1:button:5")]).unwrap();
    let text = written(&first);
    let second = write(&p, &t, &text, &[], &[]).unwrap();
    let out = lf(&written(&second));
    assert!(!out.contains("joyButton:i=32"), "{out}");
    // No binding left: the game's default stays off.
    assert!(out.contains("    ID_GEAR{}"));
    assert!(
        second
            .changes
            .iter()
            .any(|c| c.action == "ID_GEAR" && c.kind == ChangeKind::Removed)
    );
}

#[test]
fn a_device_the_file_does_not_know_stops_the_export() {
    let p = plugin();
    let t = target(&p);
    let err = write(&p, &t, HOTAS, &[], &[bind("ID_GEAR", "vjoy:1:button:5")]).unwrap_err();
    assert!(
        err.to_string()
            .contains("had not seen the virtual joysticks"),
        "{err}"
    );
}
