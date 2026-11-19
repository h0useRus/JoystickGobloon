//! The Elite Dangerous plugin (plugins/export/elite-dangerous) on a preset made from the
//! public-domain empty 4.2 preset of the elite-dangerous-bindings project, with a few bindings
//! put in (tests/data/elite-dangerous/Custom.4.2.binds): vJoy 1 yaw (inverted) and hat 1 up,
//! vJoy 2 (DeviceIndex 1) throttle and button 5, Shift+L, another stick's trigger and a half axis.

use std::collections::BTreeMap;
use std::path::Path;

use gob_export::*;

const CUSTOM: &str = include_str!("data/elite-dangerous/Custom.4.2.binds");
const START: &str = "Custom\r\nCustom\r\nKeyboardMouseOnly\r\nKeyboardMouseOnly\r\n";
const DIR: &str =
    "C:/Users/h0use/AppData/Local/Frontier Developments/Elite Dangerous/Options/Bindings";

fn plugin() -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export/elite-dangerous");
    Plugin::load(&dir).expect("the plugin loads")
}

fn target(p: &Plugin) -> Target {
    p.targets(&Env {
        platform: "windows".into(),
        roots: vec!["C:/".into()],
        documents: "C:/Users/h0use/Documents/".into(),
        local_app_data: "C:/Users/h0use/AppData/Local/".into(),
    })
    .unwrap()
    .into_iter()
    .find(|t| t.id == DIR)
    .expect("the bindings folder")
}

fn custom_key() -> String {
    format!("binds:{DIR}/Custom.4.2.binds")
}

/// The files as the host gives them: the start file and the presets its glob found.
fn files(t: &Target, start: Option<&str>, custom: &str) -> Files {
    let mut files = Files::new();
    files.insert(
        "start".into(),
        GameFile {
            path: t.files["start"].clone(),
            text: start.map(str::to_string),
        },
    );
    files.insert(
        custom_key(),
        GameFile {
            path: format!("{DIR}/Custom.4.2.binds"),
            text: Some(custom.to_string()),
        },
    );
    // An older preset version of the same name, and another preset.
    for name in ["Custom.4.1.binds", "Mine.4.2.binds"] {
        files.insert(
            format!("binds:{DIR}/{name}"),
            GameFile {
                path: format!("{DIR}/{name}"),
                text: Some("<Root/>".into()),
            },
        );
    }
    files
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

fn read(p: &Plugin, t: &Target, custom: &str) -> ReadResult {
    p.read(&ReadContext {
        target: t,
        files: &files(t, Some(START), custom),
        catalog: p.catalogs.pick(Some("4.2")).unwrap().0,
        ours: &ours(2),
        devices: &[],
    })
    .unwrap()
}

fn write(
    p: &Plugin,
    t: &Target,
    start: Option<&str>,
    binds: &[Bind],
    cleared: &[String],
) -> Result<WriteResult, PluginError> {
    p.write(&WriteContext {
        target: t,
        files: &files(t, start, CUSTOM),
        catalog: p.catalogs.pick(Some("4.2")).unwrap().0,
        ours: &ours(2),
        devices: &[],
        binds,
        cleared,
        profile: "p",
        options: &BTreeMap::new(),
    })
}

/// The text of element `tag` (its start tag to its end tag) in a preset.
fn element<'a>(text: &'a str, tag: &str) -> &'a str {
    let start = text.find(&format!("<{tag}>")).expect(tag);
    let end = text[start..].find(&format!("</{tag}>")).unwrap() + start;
    &text[start..end]
}

fn sorted(mut binds: Vec<Bind>) -> Vec<(String, String, Option<bool>)> {
    binds.sort_by(|a, b| (&a.action, &a.output).cmp(&(&b.action, &b.output)));
    binds
        .into_iter()
        .map(|b| (b.action, b.output, Some(b.invert)))
        .collect()
}

#[test]
fn finds_the_bindings_folder() {
    let p = plugin();
    let t = target(&p);
    assert_eq!(t.files["start"], format!("{DIR}/StartPreset.4.start"));
    assert_eq!(t.globs["binds"], format!("{DIR}/*.binds"));
    // Linux: the Proton prefix's drive_c is a root.
    let proton = p
        .targets(&Env {
            platform: "linux".into(),
            roots: vec!["/home/h0use/.steam/steam/steamapps/compatdata/359320/pfx/drive_c".into()],
            documents: String::new(),
            local_app_data: String::new(),
        })
        .unwrap();
    assert!(proton.iter().any(|t| t.id.ends_with(
        "pfx/drive_c/users/steamuser/AppData/Local/Frontier Developments/Elite Dangerous/Options/Bindings"
    )));
}

#[test]
fn the_catalog_lists_axes_and_buttons() {
    let p = plugin();
    let (catalog, _) = p.catalogs.pick(Some("4.2")).unwrap();
    let kind = |id: &str| catalog.actions.iter().find(|a| a.id == id).map(|a| a.kind);
    assert_eq!(kind("YawAxisRaw"), Some(ActionKind::Axis));
    assert_eq!(kind("LandingGearToggle"), Some(ActionKind::Button));
    assert!(catalog.actions.len() > 400, "{}", catalog.actions.len());
}

#[test]
fn reads_the_active_presets_bindings() {
    let p = plugin();
    let t = target(&p);
    let ctx_files = files(&t, Some(START), CUSTOM);
    assert_eq!(p.version(&t, &ctx_files).unwrap().as_deref(), Some("4.2"));
    let r = read(&p, &t, CUSTOM);
    assert_eq!(
        r.devices
            .iter()
            .map(|d| (d.output.as_str(), d.game_device.as_str()))
            .collect::<Vec<_>>(),
        [("vjoy:1", "1234BEAD"), ("vjoy:2", "1234BEAD:1")]
    );
    let binds = sorted(r.binds);
    let has = |action: &str, output: &str| binds.iter().any(|b| b.0 == action && b.1 == output);
    assert!(has("YawAxisRaw", "vjoy:1:axis:x"));
    assert!(
        binds
            .iter()
            .any(|b| b.0 == "YawAxisRaw" && b.2 == Some(true)),
        "inverted"
    );
    assert!(has("ThrottleAxis", "vjoy:2:axis:z"));
    assert!(has("LandingGearToggle", "vjoy:2:button:5"));
    assert!(has("LandingGearToggle", "key:0x2a+key:0x26"), "{binds:?}");
    assert!(has("CycleFireGroupNext", "vjoy:1:hat:1:up"));
    // Another stick and a half axis are not ours.
    assert!(
        !binds
            .iter()
            .any(|b| b.0 == "PrimaryFire" || b.0 == "ShipSpotLightToggle")
    );
}

#[test]
fn writes_into_the_profiles_slots_and_keeps_the_rest() {
    let p = plugin();
    let t = target(&p);
    let binds = [
        // Yaw moves to vJoy 2's X axis.
        bind("YawAxisRaw", "vjoy:2:axis:x"),
        bind("ThrottleAxis", "vjoy:2:axis:z"),
        // Landing gear keeps vJoy 2 button 5; Shift+L stays the game's (no keys in the profile).
        bind("LandingGearToggle", "vjoy:2:button:5"),
        // Fire: the other stick keeps Primary, ours goes into Secondary.
        bind("PrimaryFire", "vjoy:1:button:1"),
        // Lights: Right Ctrl + K.
        bind("ShipSpotLightToggle", "key:0x11d+key:0x25"),
    ];
    let w = write(&p, &t, Some(START), &binds, &[]).unwrap();
    let text = &w.files[&custom_key()];
    // Untouched elements stay as they were (line ends aside).
    let same =
        |tag: &str| element(text, tag).replace('\r', "") == element(CUSTOM, tag).replace('\r', "");
    assert!(same("ThrottleAxis"));
    assert!(same("LandingGearToggle"));
    assert!(same("PitchAxisRaw"));
    assert!(text.starts_with("<?xml"));
    // Yaw: vJoy 2 (DeviceIndex 1), not inverted any more.
    let yaw = element(text, "YawAxisRaw");
    assert!(
        yaw.contains(r#"Device="1234BEAD""#) && yaw.contains(r#"DeviceIndex="1""#),
        "{yaw}"
    );
    assert!(yaw.contains(r#"Key="Joy_XAxis""#));
    assert!(yaw.contains(r#"<Inverted Value="0""#), "{yaw}");
    // Fire: both inputs.
    let fire = element(text, "PrimaryFire");
    assert!(
        fire.contains(r#"<Primary Device="231D0200" Key="Joy_1""#),
        "{fire}"
    );
    assert!(
        fire.contains(r#"Secondary Device="1234BEAD" Key="Joy_1""#),
        "{fire}"
    );
    // Lights: the half axis stays in Primary, the keys go into Secondary.
    let lights = element(text, "ShipSpotLightToggle");
    assert!(
        lights.contains("Pos_Joy_UAxis") && lights.contains("<Hold"),
        "{lights}"
    );
    assert!(
        lights.contains(r#"Secondary Device="Keyboard" Key="Key_K""#),
        "{lights}"
    );
    assert!(
        lights.contains(r#"<Modifier Device="Keyboard" Key="Key_RightControl""#),
        "{lights}"
    );
    // Hat 1 up is gone from the profile: its slot is emptied.
    let cycle = element(text, "CycleFireGroupNext");
    assert!(
        cycle.contains(r#"<Primary Device="{NoDevice}" Key="""#),
        "{cycle}"
    );
    let kinds: Vec<(&str, ChangeKind)> = w
        .changes
        .iter()
        .map(|c| (c.action.as_str(), c.kind))
        .collect();
    assert!(
        kinds.contains(&("YawAxisRaw", ChangeKind::Changed)),
        "{kinds:?}"
    );
    assert!(kinds.contains(&("PrimaryFire", ChangeKind::Added)));
    assert!(kinds.contains(&("CycleFireGroupNext", ChangeKind::Removed)));
    assert!(
        !kinds
            .iter()
            .any(|(a, _)| *a == "ThrottleAxis" || *a == "LandingGearToggle")
    );
    // Read back: what was written.
    let again = read(&p, &t, text);
    let back = sorted(again.binds);
    assert!(
        back.iter()
            .any(|b| b.0 == "PrimaryFire" && b.1 == "vjoy:1:button:1")
    );
    assert!(
        back.iter()
            .any(|b| b.0 == "ShipSpotLightToggle" && b.1 == "key:0x11d+key:0x25")
    );
}

#[test]
fn keys_of_actions_held_here_replace_the_games() {
    let p = plugin();
    let t = target(&p);
    // The profile holds the landing gear's keys: Shift+L goes, G comes.
    let w = write(
        &p,
        &t,
        Some(START),
        &[
            bind("LandingGearToggle", "vjoy:2:button:5"),
            bind("LandingGearToggle", "key:0x22"),
        ],
        &[],
    )
    .unwrap();
    let gear = element(&w.files[&custom_key()], "LandingGearToggle");
    assert!(
        gear.contains(r#"Secondary Device="Keyboard" Key="Key_G""#),
        "{gear}"
    );
    assert!(
        !gear.contains("Key_L") && !gear.contains("Modifier"),
        "{gear}"
    );
    // Cleared: the game's keys go and nothing comes.
    let w = write(&p, &t, Some(START), &[], &["LandingGearToggle".to_string()]).unwrap();
    let gear = element(&w.files[&custom_key()], "LandingGearToggle");
    assert!(!gear.contains("Key_L"), "{gear}");
}

#[test]
fn a_built_in_preset_is_never_written() {
    let p = plugin();
    let t = target(&p);
    let stock = "KeyboardMouseOnly\r\nThrustMasterHOTASWarthog\r\nKeyboardMouseOnly\r\nKeyboardMouseOnly\r\n";
    let err = write(
        &p,
        &t,
        Some(stock),
        &[bind("YawAxisRaw", "vjoy:1:axis:x")],
        &[],
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("ThrustMasterHOTASWarthog"),
        "{err}"
    );
}
