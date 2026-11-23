//! The Microsoft Flight Simulator plugin (plugins/export/msfs) on real control profiles of one
//! vJoy device (tests/data/msfs, from the CC0 ControllerBuddy-Profiles project): a 2020 profile
//! (only bound actions) and the 2024 general and airplanes profiles (every action).

use std::collections::BTreeMap;
use std::path::Path;

use gob_export::*;

const P2020: &str = include_str!("data/msfs/inputprofile_2020");
const GENERAL: &str = include_str!("data/msfs/general_2024.xml");
const AIRPLANES: &str = include_str!("data/msfs/airplanes_2024.xml");
const GUID_2020: &str = "{DC899B70-5FB0-11EA-8003-444553540000}";
const GUID_2024: &str = "{75BB9150-3AD0-11EF-800E-444553540000}";

fn plugin() -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export/msfs");
    Plugin::load(&dir).expect("the plugin loads")
}

fn targets(p: &Plugin) -> Vec<Target> {
    p.targets(&Env {
        platform: "windows".into(),
        roots: vec!["C:/".into(), "C:\\Users\\h0use".into()],
        documents: String::new(),
        local_app_data: String::new(),
    })
    .unwrap()
}

fn target(p: &Plugin, id: &str) -> Target {
    targets(p)
        .into_iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("no target {id}"))
}

fn steam(p: &Plugin, version: &str) -> Target {
    target(p, &format!("steam{version}:C:/Program Files (x86)/Steam/"))
}

fn store(p: &Plugin, version: &str) -> Target {
    target(p, &format!("store{version}:C:/Users/h0use/"))
}

/// The fixture as the sim writes it, with LF line ends whatever the checkout made of them.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The target's profile files, named as globs name them; LF line ends.
fn files(profiles: &[(&str, &str)]) -> Files {
    files_as_is(
        &profiles
            .iter()
            .map(|(name, text)| (*name, lf(text)))
            .collect::<Vec<_>>(),
    )
}

fn files_as_is(profiles: &[(&str, String)]) -> Files {
    profiles
        .iter()
        .map(|(name, text)| {
            let path = format!("C:/Steam/userdata/1/remote/{name}");
            (
                format!("profile:{path}"),
                GameFile {
                    path,
                    text: Some(text.clone()),
                },
            )
        })
        .collect()
}

fn ours(n: u32) -> Vec<OurDevice> {
    (1..=n)
        .map(|i| OurDevice {
            output: format!("vjoy:{i}"),
            name: "vJoy Device".into(),
            guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
            instance: String::new(),
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

fn catalog(p: &Plugin, version: &str) -> Catalog {
    p.catalogs.pick(Some(version)).unwrap().0.clone()
}

fn read(p: &Plugin, t: &Target, version: &str, files: &Files) -> ReadResult {
    read_with(p, t, version, files, &ours(1), &[])
}

fn read_with(
    p: &Plugin,
    t: &Target,
    version: &str,
    files: &Files,
    ours: &[OurDevice],
    devices: &[DeviceLink],
) -> ReadResult {
    p.read(&ReadContext {
        target: t,
        files,
        catalog: &catalog(p, version),
        ours,
        devices,
    })
    .unwrap()
}

fn write(
    p: &Plugin,
    t: &Target,
    version: &str,
    files: &Files,
    ours: &[OurDevice],
    binds: &[Bind],
    options: &BTreeMap<String, OptionValue>,
) -> Result<WriteResult, PluginError> {
    write_with(p, t, version, files, ours, &[], binds, options)
}

#[allow(clippy::too_many_arguments)]
fn write_with(
    p: &Plugin,
    t: &Target,
    version: &str,
    files: &Files,
    ours: &[OurDevice],
    devices: &[DeviceLink],
    binds: &[Bind],
    options: &BTreeMap<String, OptionValue>,
) -> Result<WriteResult, PluginError> {
    p.write(&WriteContext {
        target: t,
        files,
        catalog: &catalog(p, version),
        ours,
        devices,
        binds,
        cleared: &[],
        profile: "Test",
        options,
    })
}

fn sorted(mut binds: Vec<Bind>) -> Vec<Bind> {
    binds.sort_by(|a, b| (&a.action, &a.output).cmp(&(&b.action, &b.output)));
    binds
}

#[test]
fn finds_steam_and_store_installs() {
    let p = plugin();
    let s = steam(&p, "2020");
    assert_eq!(s.name, "2020 (Steam)");
    assert_eq!(
        s.globs["profile"],
        "C:/Program Files (x86)/Steam/userdata/*/1250410/remote/inputprofile_*"
    );
    assert!(steam(&p, "2024").globs["profile"].contains("/2537590/remote/"));
    let st = store(&p, "2024");
    assert_eq!(
        st.globs["profile"],
        "C:/Users/h0use/AppData/Local/Packages/Microsoft.Limitless_8wekyb3d8bbwe/SystemAppData/wgs/*/*/*"
    );
    assert_eq!(
        st.dirs["import"],
        "C:/Users/h0use/Documents/Joystick Gobloon/Flight Simulator 2024"
    );
    assert!(store(&p, "2020").dirs.is_empty());
    for (t, v) in [(s, "2020"), (st, "2024")] {
        assert_eq!(p.version(&t, &Files::new()).unwrap().as_deref(), Some(v));
    }
    // Linux has no Store version.
    let linux = p
        .targets(&Env {
            platform: "linux".into(),
            roots: vec!["/home/h0use".into()],
            documents: String::new(),
            local_app_data: String::new(),
        })
        .unwrap();
    assert!(linux.iter().all(|t| t.id.starts_with("steam")));
    assert!(linux.iter().any(|t| t.globs["profile"]
        == "/home/h0use/.local/share/Steam/userdata/*/2537590/remote/inputprofile_*"));
}

#[test]
fn the_host_finds_the_profiles() {
    let dir = std::env::temp_dir().join(format!("gob-export-msfs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let remote = dir.join("Steam/userdata/12345/1250410/remote");
    std::fs::create_dir_all(&remote).unwrap();
    std::fs::write(remote.join("inputprofile_0a1b2c"), P2020).unwrap();
    std::fs::write(remote.join("UserCfg.opt"), "x").unwrap();
    let p = plugin();
    let all = p
        .targets(&Env {
            platform: "windows".into(),
            roots: vec![dir.to_string_lossy().into_owned()],
            documents: String::new(),
            local_app_data: String::new(),
        })
        .unwrap();
    let installed: Vec<_> = all.into_iter().filter(host::installed).collect();
    assert_eq!(installed.len(), 1);
    let t = &installed[0];
    assert!(t.id.starts_with("steam2020:"));
    let files = host::read_files(t);
    assert_eq!(files.len(), 1);
    let read = read(&p, t, "2020", &files);
    assert_eq!(read.devices.len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn reads_the_2020_profile() {
    let p = plugin();
    let t = steam(&p, "2020");
    let r = read(&p, &t, "2020", &files(&[("inputprofile_a", P2020)]));
    assert_eq!(r.devices.len(), 1);
    assert_eq!(r.devices[0].output, "vjoy:1");
    assert_eq!(r.devices[0].game_device, GUID_2020);
    let has = |action: &str, output: &str, invert: bool| {
        assert!(
            r.binds
                .iter()
                .any(|b| b.action == action && b.output == output && b.invert == invert),
            "{action} {output} {invert}: {:?}",
            r.binds
        );
    };
    has("PLANE/KEY_AXIS_RUDDER_SET", "vjoy:1:axis:rz", false);
    has("PLANE/KEY_BRAKES_LEFT", "vjoy:1:button:1", false);
    has("PLANE/KEY_MIXTURE_SET", "vjoy:1:axis:dial", false);
    has(
        "PLANE/KEY_PROP_PITCH_AXIS_SET_EX1",
        "vjoy:1:axis:slider",
        false,
    );
    has("SURFACES/KEY_AXIS_AILERONS_SET", "vjoy:1:axis:x", false);
    // Flag 132 = an axis reversed.
    has(
        "COCKPIT_GLOBAL_CAMERA/KEY_COCKPIT_FREELOOK_VRTC_SET",
        "vjoy:1:axis:ry",
        true,
    );
    // Half axes are no output the profile sends.
    assert!(!r.binds.iter().any(|b| b.action.contains("KEY_SLEW_")));
    assert!(!r.binds.iter().any(|b| b.default));
}

#[test]
fn reads_the_2024_profiles_by_category() {
    let p = plugin();
    let t = steam(&p, "2024");
    let r = read(
        &p,
        &t,
        "2024",
        &files(&[("inputprofile_a", GENERAL), ("inputprofile_b", AIRPLANES)]),
    );
    assert_eq!(r.devices.len(), 1);
    assert_eq!(r.devices[0].game_device, GUID_2024);
    let c = catalog(&p, "2024");
    for b in &r.binds {
        assert!(c.action(&b.action).is_some(), "{} in the catalog", b.action);
    }
    assert!(
        r.binds
            .iter()
            .any(|b| b.action == "AIRPLANE:SURFACES/KEY_AXIS_ELEVATOR_SET"
                && b.output == "vjoy:1:axis:y")
    );
    assert!(r.binds.iter().any(
        |b| b.action == "PLAYER_CHARACTER_MOVEMENT/KEY_PC_MOVE_FORWARD"
            && b.output == "vjoy:1:button:10"
    ));
}

#[test]
fn writing_what_was_read_changes_nothing() {
    let p = plugin();
    for (version, profiles) in [
        ("2020", vec![("inputprofile_a", P2020)]),
        (
            "2024",
            vec![("inputprofile_a", GENERAL), ("inputprofile_b", AIRPLANES)],
        ),
    ] {
        for eol in ["\n", "\r\n"] {
            let t = steam(&p, version);
            let f = files_as_is(
                &profiles
                    .iter()
                    .map(|(name, text)| (*name, lf(text).replace('\n', eol)))
                    .collect::<Vec<_>>(),
            );
            let r = read(&p, &t, version, &f);
            assert!(!r.binds.is_empty());
            let out = write(&p, &t, version, &f, &ours(1), &r.binds, &BTreeMap::new()).unwrap();
            assert!(out.changes.is_empty(), "{version}: {:?}", out.changes);
            for (key, text) in &out.files {
                assert_eq!(Some(text), f[key].text.as_ref(), "{version}: {key}");
            }
        }
    }
}

#[test]
fn writes_2024_bindings_in_place() {
    let p = plugin();
    let t = steam(&p, "2024");
    let f = files(&[("inputprofile_a", GENERAL), ("inputprofile_b", AIRPLANES)]);
    let mut binds: Vec<Bind> = read(&p, &t, "2024", &f)
        .binds
        .into_iter()
        .filter(|b| b.action != "AIRPLANE:SURFACES/KEY_AXIS_ELEVATOR_SET")
        .collect();
    binds.push(bind("AIRPLANE:AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:1:button:5"));
    binds.push(bind(
        "AIRPLANE:AIRCRAFT/KEY_GEAR_TOGGLE",
        "vjoy:1:hat:1:left",
    ));
    let ail = binds
        .iter_mut()
        .find(|b| b.action == "AIRPLANE:SURFACES/KEY_AXIS_AILERONS_SET")
        .unwrap();
    ail.invert = true;
    // Keys belong to the sim's keyboard profile.
    binds.push(bind("AIRPLANE:AIRCRAFT/KEY_PARKING_BRAKES", "key:0x2e"));
    let out = write(&p, &t, "2024", &f, &ours(1), &binds, &BTreeMap::new()).unwrap();
    assert!(out.create.is_empty());
    let general = "profile:C:/Steam/userdata/1/remote/inputprofile_a";
    let air = "profile:C:/Steam/userdata/1/remote/inputprofile_b";
    // The general profile did not change.
    assert_eq!(out.files[general], lf(GENERAL));
    let text = &out.files[air];
    assert!(
        text.contains(
            "\t\t<Action ActionName=\"KEY_GEAR_TOGGLE\" ValueEvent=\"0.000000\" Delay=\"2.000000\" Flag=\"2\">\n\t\t\t<Primary>\n\t\t\t\t<KEY Information=\"Joystick Button 5\">4</KEY>\n\t\t\t</Primary>\n\t\t\t<Secondary>\n\t\t\t\t<KEY Information=\"JoystickPov Left\">259</KEY>\n\t\t\t</Secondary>\n\t\t</Action>\n"
        ),
        "{}",
        &text[text.find("KEY_GEAR_TOGGLE").unwrap()..][..400]
    );
    // The elevator has no binding left; its own axis settings stay.
    let elevator = &text[text.find("\"KEY_AXIS_ELEVATOR_SET\"").unwrap()..];
    let elevator = &elevator[..elevator.find("</Action>").unwrap()];
    assert!(!elevator.contains("<Primary>"), "{elevator}");
    assert!(elevator.contains("<Axis "), "{elevator}");
    // The ailerons reversed: Flag 4100 + 128.
    assert!(
        text.contains("ActionName=\"KEY_AXIS_AILERONS_SET\" ValueEvent=\"0.000000\" Delay=\"0.000000\" Flag=\"4228\""),
        "{}",
        &text[text.find("KEY_AXIS_AILERONS_SET").unwrap()..][..200]
    );
    // Only those three actions changed.
    let changed: Vec<_> = out.changes.iter().map(|c| c.action.as_str()).collect();
    assert_eq!(
        changed,
        [
            "AIRPLANE:AIRCRAFT/KEY_GEAR_TOGGLE",
            "AIRPLANE:SURFACES/KEY_AXIS_AILERONS_SET",
            "AIRPLANE:SURFACES/KEY_AXIS_ELEVATOR_SET",
        ]
    );
    let gear = &out.changes[0];
    assert_eq!(gear.kind, ChangeKind::Added);
    assert_eq!(
        gear.to.as_deref(),
        Some("Joystick Button 5, JoystickPov Left")
    );
    assert_eq!(out.changes[2].kind, ChangeKind::Removed);
    assert_eq!(out.changes[2].from.as_deref(), Some("Joystick L-Axis Y"));
    // Reading the new files gives what was written.
    let mut again = f.clone();
    again.get_mut(air).unwrap().text = Some(text.clone());
    let wanted: Vec<Bind> = binds
        .into_iter()
        .filter(|b| b.output.starts_with("vjoy:"))
        .collect();
    assert_eq!(sorted(read(&p, &t, "2024", &again).binds), sorted(wanted));
}

#[test]
fn an_unbound_2024_action_closes_again() {
    let p = plugin();
    let t = steam(&p, "2024");
    let f = files(&[("inputprofile_a", GENERAL)]);
    let binds: Vec<Bind> = read(&p, &t, "2024", &f)
        .binds
        .into_iter()
        .filter(|b| b.action != "PLAYER_CHARACTER_MOVEMENT/KEY_PC_MOVE_FORWARD")
        .collect();
    let out = write(&p, &t, "2024", &f, &ours(1), &binds, &BTreeMap::new()).unwrap();
    let text = out.files.values().next().unwrap();
    assert!(
        text.contains("\t\t<Action ActionName=\"KEY_PC_MOVE_FORWARD\" ValueEvent=\"0.000000\" Delay=\"0.000000\" Flag=\"2\"/>\n\t\t<Action "),
        "{}",
        &text[text.find("KEY_PC_MOVE_FORWARD").unwrap() - 40..][..300]
    );
    // Nothing else changed.
    let before = "<Action ActionName=\"KEY_PC_MOVE_FORWARD\" ValueEvent=\"0.000000\" Delay=\"2.000000\" Flag=\"1\">\n\t\t\t<Primary>\n\t\t\t\t<KEY Information=\"Joystick Button 10\">9</KEY>\n\t\t\t</Primary>\n\t\t</Action>";
    let after = "<Action ActionName=\"KEY_PC_MOVE_FORWARD\" ValueEvent=\"0.000000\" Delay=\"0.000000\" Flag=\"2\"/>";
    assert_eq!(lf(text), lf(GENERAL).replacen(before, after, 1));
}

#[test]
fn writes_2020_bindings_in_place() {
    let p = plugin();
    let t = steam(&p, "2020");
    let f = files(&[("inputprofile_a", P2020)]);
    let mut binds: Vec<Bind> = read(&p, &t, "2020", &f)
        .binds
        .into_iter()
        .filter(|b| b.action != "PLANE/KEY_BRAKES_LEFT")
        .collect();
    binds.push(bind("AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:1:button:12"));
    let out = write(&p, &t, "2020", &f, &ours(1), &binds, &BTreeMap::new()).unwrap();
    let text = &out.files["profile:C:/Steam/userdata/1/remote/inputprofile_a"];
    // An unbound action leaves the file, as the 2020 sim writes it.
    assert!(!text.contains("KEY_BRAKES_LEFT"));
    assert!(text.contains("KEY_BRAKES_RIGHT"));
    // A new context at the end of the device, written the 2020 way.
    assert!(
        text.contains("\t<Context ContextName=\"AIRCRAFT\">\n\t\t<Action ActionName=\"KEY_GEAR_TOGGLE\" Flag=\"2\">\n\t\t\t<Primary>\n\t\t\t\t<KEY Information=\"Joystick Button 12\">11</KEY>\n\t\t\t</Primary>\n\t\t</Action>\n\t</Context>\n</Device>"),
        "{text}"
    );
    // The half axes of the slew mode stay.
    assert!(text.contains("Joystick L-Axis Y+\">1041"));
    let mut again = f.clone();
    again.values_mut().next().unwrap().text = Some(text.clone());
    assert_eq!(sorted(read(&p, &t, "2020", &again).binds), sorted(binds));
}

#[test]
fn the_store_version_gets_files_to_import() {
    let p = plugin();
    let t = store(&p, "2024");
    let f = files(&[("x/y/C0FFEE", AIRPLANES)]);
    let binds = [bind("AIRPLANE:AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:1:button:5")];
    let out = write(&p, &t, "2024", &f, &ours(1), &binds, &BTreeMap::new()).unwrap();
    assert!(out.files.is_empty(), "cloud saves are never written");
    assert_eq!(out.create.len(), 1);
    assert_eq!(out.create[0].dir, "import");
    assert_eq!(out.create[0].name, "ControllerBuddy - Airplanes.xml");
    assert!(out.create[0].text.contains("Joystick Button 5"));

    let old = store(&p, "2020");
    let e = write(
        &p,
        &old,
        "2020",
        &files(&[("x/y/C0FFEE", P2020)]),
        &ours(1),
        &[],
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert!(e.message.contains("cloud saves"), "{}", e.message);
}

#[test]
fn tells_what_is_missing() {
    let p = plugin();
    let t = steam(&p, "2024");
    let f = files(&[("inputprofile_a", GENERAL), ("inputprofile_b", AIRPLANES)]);
    let none = BTreeMap::new();
    // No helicopter profile.
    let e = write(
        &p,
        &t,
        "2024",
        &f,
        &ours(1),
        &[bind(
            "HELICOPTER:SURFACES/KEY_AXIS_ELEVATOR_SET",
            "vjoy:1:axis:y",
        )],
        &none,
    )
    .unwrap_err();
    assert!(e.message.contains("Helicopters"), "{}", e.message);
    // A second virtual joystick without a profile.
    let e = write(
        &p,
        &t,
        "2024",
        &f,
        &ours(2),
        &[bind("AIRPLANE:AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:2:button:1")],
        &none,
    )
    .unwrap_err();
    assert!(e.message.contains("no control profile"), "{}", e.message);
    // Only the presets named so.
    let named = BTreeMap::from([("preset".to_string(), OptionValue::Text("Other".into()))]);
    let e = write(
        &p,
        &t,
        "2024",
        &f,
        &ours(1),
        &[bind("AIRPLANE:AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:1:button:1")],
        &named,
    )
    .unwrap_err();
    assert!(e.message.contains("\"Other\""), "{}", e.message);
    let named = BTreeMap::from([(
        "preset".to_string(),
        OptionValue::Text("controllerbuddy".into()),
    )]);
    assert!(
        write(
            &p,
            &t,
            "2024",
            &f,
            &ours(1),
            &[bind("AIRPLANE:AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:1:button:1")],
            &named,
        )
        .is_ok()
    );
}

/// A second vJoy device's general profile: the fixture under another GUID.
const GUID_2: &str = "{11111111-2222-11EF-8001-444553540000}";

fn second() -> String {
    lf(GENERAL).replace(GUID_2024, GUID_2)
}

fn with_instances(instances: &[&str]) -> Vec<OurDevice> {
    let mut list = ours(instances.len() as u32);
    for (o, i) in list.iter_mut().zip(instances) {
        o.instance = i.to_string();
    }
    list
}

fn link(output: &str, game_device: &str) -> DeviceLink {
    DeviceLink {
        output: output.into(),
        game_device: game_device.into(),
    }
}

#[test]
fn devices_alike_are_told_apart_by_their_instance() {
    let p = plugin();
    let t = steam(&p, "2024");
    // File names sort the second device first.
    let f = files_as_is(&[
        ("inputprofile_0a", second()),
        ("inputprofile_f0", lf(GENERAL)),
    ]);
    let r = read_with(
        &p,
        &t,
        "2024",
        &f,
        &with_instances(&[&GUID_2024.to_lowercase(), GUID_2]),
        &[],
    );
    assert_eq!(
        r.devices,
        [link("vjoy:1", GUID_2024), link("vjoy:2", GUID_2)]
    );
    assert!(r.binds.iter().any(|b| b.output.starts_with("vjoy:2:")));
    // Without instances two devices of one name are not guessed.
    let r = read_with(&p, &t, "2024", &f, &ours(2), &[]);
    assert!(r.devices.is_empty(), "{:?}", r.devices);
    assert!(r.binds.is_empty());
    // One device of that name: its profile, whatever the name.
    assert_eq!(
        read(&p, &t, "2024", &files(&[("inputprofile_a", GENERAL)]))
            .devices
            .len(),
        1
    );
}

#[test]
fn links_the_profile_keeps_win() {
    let p = plugin();
    let t = steam(&p, "2024");
    let f = files_as_is(&[
        ("inputprofile_0a", second()),
        ("inputprofile_f0", lf(GENERAL)),
    ]);
    let found = with_instances(&[GUID_2024, GUID_2]);
    // The user swapped the devices: reading follows.
    let kept = [link("vjoy:1", GUID_2), link("vjoy:2", GUID_2024)];
    let r = read_with(&p, &t, "2024", &f, &found, &kept);
    assert_eq!(
        r.devices,
        [link("vjoy:1", GUID_2), link("vjoy:2", GUID_2024)]
    );
    // One link typed: the other device keeps the one found.
    let kept = [link("vjoy:3", GUID_2.trim_matches(['{', '}']))];
    let mut three = with_instances(&[GUID_2024, GUID_2, ""]);
    three[2].name = "Other".into();
    let r = read_with(&p, &t, "2024", &f, &three, &kept);
    assert_eq!(
        r.devices,
        [
            link("vjoy:1", GUID_2024),
            link("vjoy:3", GUID_2.trim_matches(['{', '}']))
        ]
    );
    // Writing a found device next to a typed one works.
    let binds = [
        bind("AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:1:button:7"),
        bind("AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:3:button:8"),
    ];
    let out = write_with(&p, &t, "2024", &f, &three, &kept, &binds, &BTreeMap::new()).unwrap();
    let first = &out.files["profile:C:/Steam/userdata/1/remote/inputprofile_f0"];
    let other = &out.files["profile:C:/Steam/userdata/1/remote/inputprofile_0a"];
    assert!(first.contains("Joystick Button 7\">6<"));
    assert!(other.contains("Joystick Button 8\">7<"));
}

#[test]
fn more_than_two_bindings_change_nothing_the_second_time() {
    let p = plugin();
    let t = steam(&p, "2024");
    let f = files(&[("inputprofile_a", GENERAL)]);
    let mut binds = read(&p, &t, "2024", &f).binds;
    for n in [20, 21, 22] {
        binds.push(bind(
            "AIRCRAFT/KEY_GEAR_TOGGLE",
            &format!("vjoy:1:button:{n}"),
        ));
    }
    let none = BTreeMap::new();
    let out = write(&p, &t, "2024", &f, &ours(1), &binds, &none).unwrap();
    assert_eq!(out.changes.len(), 1, "{:?}", out.changes);
    let mut again = f.clone();
    for (k, text) in &out.files {
        again.get_mut(k).unwrap().text = Some(text.clone());
    }
    let out = write(&p, &t, "2024", &again, &ours(1), &binds, &none).unwrap();
    assert!(out.changes.is_empty(), "{:?}", out.changes);
}

#[test]
fn profiles_without_bindings_in_the_profile_stay() {
    let p = plugin();
    let t = steam(&p, "2024");
    let f = files(&[("inputprofile_a", GENERAL), ("inputprofile_b", AIRPLANES)]);
    // Only general actions: the airplanes profile keeps its vJoy bindings.
    let binds = [bind("AIRCRAFT/KEY_GEAR_TOGGLE", "vjoy:1:button:7")];
    let out = write(&p, &t, "2024", &f, &ours(1), &binds, &BTreeMap::new()).unwrap();
    let air = "profile:C:/Steam/userdata/1/remote/inputprofile_b";
    assert!(out.files.get(air).is_none_or(|t| *t == lf(AIRPLANES)));
    assert!(
        out.changes
            .iter()
            .all(|c| !c.action.starts_with("AIRPLANE:"))
    );
}

#[test]
fn files_to_import_go_to_the_documents_folder() {
    let p = plugin();
    let all = p
        .targets(&Env {
            platform: "windows".into(),
            roots: vec!["C:/Users/h0use/".into()],
            documents: "C:/Users/h0use/OneDrive/Documents/".into(),
            local_app_data: String::new(),
        })
        .unwrap();
    let st = all
        .iter()
        .find(|t| t.id == "store2024:C:/Users/h0use/")
        .unwrap();
    assert_eq!(
        st.dirs["import"],
        "C:/Users/h0use/OneDrive/Documents/Joystick Gobloon/Flight Simulator 2024"
    );
    assert_eq!(st.glob_text["profile"], "<Device");
}
