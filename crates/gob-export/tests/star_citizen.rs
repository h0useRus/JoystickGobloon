//! The Star Citizen plugin (plugins/export/star-citizen) on a small synthetic actionmaps.xml.

use std::collections::BTreeMap;
use std::path::Path;

use gob_export::*;

const ACTIONMAPS: &str = "<ActionMaps>\r\n <ActionProfiles version=\"1\" optionsVersion=\"2\" rebindVersion=\"2\" profileName=\"default\">\r\n  <options type=\"keyboard\" instance=\"1\" Product=\"Keyboard  {6F1D2B61-D5A0-11CF-BFC7-444553540000}\"/>\r\n  <options type=\"joystick\" instance=\"1\" Product=\"vJoy Device  {BEAD1234-0000-0000-0000-504944564944}\"/>\r\n  <options type=\"joystick\" instance=\"2\"/>\r\n  <modifiers />\r\n  <actionmap name=\"seat_general\">\r\n   <action name=\"v_eject\">\r\n    <rebind input=\"js1_button47\" activationMode=\"double_tap\"/>\r\n   </action>\r\n   <action name=\"v_emergency_exit\">\r\n    <rebind input=\"kb1_up\"/>\r\n    <rebind input=\"js1_button17\"/>\r\n   </action>\r\n  </actionmap>\r\n  <actionmap name=\"spaceship_movement\">\r\n   <action name=\"v_strafe_up\">\r\n    <rebind input=\"js1_ \"/>\r\n   </action>\r\n  </actionmap>\r\n </ActionProfiles>\r\n</ActionMaps>\r\n";

fn plugin() -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export/star-citizen");
    Plugin::load(&dir).expect("Star Citizen plugin loads")
}

fn target(p: &Plugin) -> Target {
    p.targets(&Env {
        platform: "windows".into(),
        roots: vec!["G:\\RSI\\StarCitizen".into()],
        documents: String::new(),
        local_app_data: String::new(),
    })
    .unwrap()
    .into_iter()
    .find(|t| t.id == "G:/RSI/StarCitizen/LIVE")
    .expect("LIVE target")
}

fn files(t: &Target, actionmaps: &str) -> Files {
    let file = |key: &str, text: Option<&str>| {
        (
            key.to_string(),
            GameFile {
                path: t.files[key].clone(),
                text: text.map(str::to_string),
            },
        )
    };
    [
        file(
            "manifest",
            Some(r#"{"Data": {"Version": "4.10.197.2878"}}"#),
        ),
        file("actionmaps", Some(actionmaps)),
    ]
    .into()
}

fn ours(n: u32) -> Vec<OurDevice> {
    (1..=n)
        .map(|i| OurDevice {
            output: format!("vjoy:{i}"),
            name: "vJoy Device".into(),
            guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
            instance: String::new(),
            ..Default::default()
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

#[test]
fn targets_and_version() {
    let p = plugin();
    let t = target(&p);
    assert_eq!(t.name, "LIVE");
    assert_eq!(
        t.files["actionmaps"],
        "G:/RSI/StarCitizen/LIVE/user/client/0/Profiles/default/actionmaps.xml"
    );
    assert_eq!(
        t.dirs["mappings"],
        "G:/RSI/StarCitizen/LIVE/user/client/0/controls/mappings"
    );
    let v = p.version(&t, &files(&t, ACTIONMAPS)).unwrap();
    assert_eq!(v.as_deref(), Some("4.10.197.2878"));
    assert!(p.manifest.game.contains("4.10.197.2878"));
}

#[test]
fn reads_our_devices_and_bindings() {
    let p = plugin();
    let t = target(&p);
    let catalog = p.catalogs.pick(Some("4.10.197.2878")).unwrap().0;
    let read = p
        .read(&ReadContext {
            target: &t,
            files: &files(&t, ACTIONMAPS),
            catalog,
            ours: &ours(2),
            devices: &[],
        })
        .unwrap();
    assert_eq!(
        read.devices,
        [DeviceLink {
            output: "vjoy:1".into(),
            game_device: "js1".into()
        }]
    );
    let set: Vec<&Bind> = read.binds.iter().filter(|b| !b.default).collect();
    assert_eq!(
        set,
        [
            // Pressed twice, as its activation mode says.
            &bind("seat_general/v_eject", "vjoy:1:button:47@double"),
            &bind("seat_general/v_emergency_exit", "key:0x148"),
            &bind("seat_general/v_emergency_exit", "vjoy:1:button:17"),
        ]
    );
    // The game's own keys where the player set none: eject has Right Alt + Y, the emergency
    // exit's default gave way to the key set.
    let default = |action: &str| -> Vec<&str> {
        read.binds
            .iter()
            .filter(|b| b.default && b.action == action)
            .map(|b| b.output.as_str())
            .collect()
    };
    assert_eq!(default("seat_general/v_eject"), ["key:0x138+key:0x15"]);
    assert!(default("seat_general/v_emergency_exit").is_empty());
    assert!(catalog.action("seat_general/v_eject").is_some());
}

#[test]
fn writes_actionmaps_and_a_layout() {
    let p = plugin();
    let t = target(&p);
    let catalog = p.catalogs.pick(Some("4.10.197.2878")).unwrap().0;
    let devices = [
        DeviceLink {
            output: "vjoy:1".into(),
            game_device: "js1".into(),
        },
        DeviceLink {
            output: "vjoy:2".into(),
            game_device: "js2".into(),
        },
    ];
    let binds = [
        bind("seat_general/v_eject", "vjoy:1:button:47@double"),
        bind("spaceship_movement/v_strafe_up", "vjoy:2:hat:1:up"),
        bind("spaceship_movement/v_roll", "vjoy:2:axis:rz"),
        bind("seat_general/v_emergency_exit", "key:0x148"),
    ];
    let mut options = BTreeMap::new();
    options.insert("clear_defaults".to_string(), OptionValue::Bool(false));
    let options = p.option_values(&options).unwrap();
    let files = files(&t, ACTIONMAPS);
    let out = p
        .write(&WriteContext {
            target: &t,
            files: &files,
            catalog,
            ours: &ours(2),
            devices: &devices,
            binds: &binds,
            cleared: &[],
            profile: "My profile",
            options: &options,
        })
        .unwrap();
    let text = &out.files["actionmaps"];
    // Kept as it was: the eject binding with its activation mode, the key binding.
    assert!(
        text.contains("<rebind input=\"js1_button47\" activationMode=\"double_tap\"/>"),
        "{text}"
    );
    assert!(text.contains("<rebind input=\"kb1_up\"/>"), "{text}");
    // Gone: the emergency exit button (its key stays); the unbind on js1 stays.
    assert!(!text.contains("js1_button17"), "{text}");
    assert!(text.contains("<rebind input=\"js1_ \"/>"), "{text}");
    // New: js2's product, hat and axis bindings, indented like the rest.
    assert!(
        text.contains("<options type=\"joystick\" instance=\"2\" Product=\"vJoy Device  {BEAD1234-0000-0000-0000-504944564944}\"/>"),
        "{text}"
    );
    assert!(
        text.contains("\r\n    <rebind input=\"js2_hat1_up\"/>"),
        "{text}"
    );
    assert!(
        text.contains(
            "\r\n   <action name=\"v_roll\">\r\n    <rebind input=\"js2_rotz\"/>\r\n   </action>"
        ),
        "{text}"
    );
    let kinds: Vec<_> = out
        .changes
        .iter()
        .map(|c| (c.kind, c.action.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (ChangeKind::Changed, "seat_general/v_emergency_exit"),
            (ChangeKind::Added, "spaceship_movement/v_roll"),
            (ChangeKind::Added, "spaceship_movement/v_strafe_up"),
        ]
    );
    assert_eq!(out.create.len(), 1);
    assert_eq!(out.create[0].dir, "mappings");
    assert_eq!(out.create[0].name, "layout_My_profile_exported.xml");
    assert!(out.create[0].text.contains("<rebind input=\"js2_rotz\"/>"));
    assert!(out.create[0].text.contains("<rebind input=\"kb1_up\"/>"));

    // Reading what was written gives the same joystick bindings.
    let again = files_with(&t, text);
    let read = p
        .read(&ReadContext {
            target: &t,
            files: &again,
            catalog,
            ours: &ours(2),
            devices: &[],
        })
        .unwrap();
    for b in &binds {
        assert!(read.binds.contains(b), "{b:?} in {:?}", read.binds);
    }
}

/// A button held long or pressed twice gets its activation mode; one pressed another way now
/// changes its mode, and a plain press drops it.
#[test]
fn writes_long_and_double_presses() {
    let p = plugin();
    let t = target(&p);
    let catalog = p.catalogs.pick(Some("4.10.197.2878")).unwrap().0;
    let devices = [DeviceLink {
        output: "vjoy:1".into(),
        game_device: "js1".into(),
    }];
    let binds = [
        // Was a double tap: now plain.
        bind("seat_general/v_eject", "vjoy:1:button:47"),
        // Was plain: now held long.
        bind("seat_general/v_emergency_exit", "vjoy:1:button:17@long"),
        bind("spaceship_movement/v_strafe_up", "vjoy:1:hat:1:up@double"),
    ];
    let mut options = BTreeMap::new();
    options.insert("clear_defaults".to_string(), OptionValue::Bool(false));
    let options = p.option_values(&options).unwrap();
    let files = files(&t, ACTIONMAPS);
    let out = p
        .write(&WriteContext {
            target: &t,
            files: &files,
            catalog,
            ours: &ours(1),
            devices: &devices,
            binds: &binds,
            cleared: &[],
            profile: "My profile",
            options: &options,
        })
        .unwrap();
    let text = &out.files["actionmaps"];
    assert!(text.contains("<rebind input=\"js1_button47\"/>"), "{text}");
    assert!(
        text.contains("<rebind input=\"js1_button17\" activationMode=\"delayed_press\"/>"),
        "{text}"
    );
    assert!(
        text.contains("<rebind input=\"js1_hat1_up\" activationMode=\"double_tap\"/>"),
        "{text}"
    );
    let layout = &out.create[0].text;
    assert!(
        layout.contains("<rebind input=\"js1_button17\" activationMode=\"delayed_press\"/>"),
        "{layout}"
    );
    let eject = out
        .changes
        .iter()
        .find(|c| c.action == "seat_general/v_eject")
        .unwrap();
    assert_eq!(eject.from.as_deref(), Some("js1_button47 (double)"));
    assert_eq!(eject.to.as_deref(), Some("js1_button47"));
    // Read back, every press is as written.
    let read = p
        .read(&ReadContext {
            target: &t,
            files: &files_with(&t, text),
            catalog,
            ours: &ours(1),
            devices: &[],
        })
        .unwrap();
    for b in &binds {
        assert!(read.binds.contains(b), "{b:?} in {:?}", read.binds);
    }
}

#[test]
fn writes_keys_and_mouse_only_for_the_actions_held() {
    let p = plugin();
    let t = target(&p);
    let catalog = p.catalogs.pick(Some("4.10.197.2878")).unwrap().0;
    let mut options = BTreeMap::new();
    options.insert("clear_defaults".to_string(), OptionValue::Bool(false));
    options.insert("layout".to_string(), OptionValue::Bool(false));
    let options = p.option_values(&options).unwrap();
    let files = files(&t, ACTIONMAPS);
    let write = |files: &Files, binds: &[Bind], cleared: &[String]| {
        p.write(&WriteContext {
            target: &t,
            files,
            catalog,
            ours: &ours(1),
            devices: &[],
            binds,
            cleared,
            profile: "p",
            options: &options,
        })
        .unwrap()
    };
    let joystick = [
        bind("seat_general/v_eject", "vjoy:1:button:47@double"),
        bind("seat_general/v_emergency_exit", "vjoy:1:button:17"),
    ];
    // Nothing held: the keyboard binding in the file stays, nothing changes.
    let out = write(&files, &joystick, &[]);
    assert_eq!(out.files["actionmaps"], ACTIONMAPS);
    assert!(out.changes.is_empty(), "{:?}", out.changes);

    // Eject on Left Alt + right mouse button and on the wheel; the emergency exit without keys.
    let mut binds = joystick.to_vec();
    binds.push(bind("seat_general/v_eject", "key:0x38+mouse:right"));
    binds.push(bind("seat_general/v_eject", "mouse:wheel-up"));
    let cleared = ["seat_general/v_emergency_exit".to_string()];
    let out = write(&files, &binds, &cleared);
    let text = &out.files["actionmaps"];
    assert!(
        text.contains("<rebind input=\"kb1_lalt+mouse2\"/>"),
        "{text}"
    );
    assert!(text.contains("<rebind input=\"mo1_mwheel_up\"/>"), "{text}");
    assert!(!text.contains("kb1_up"), "{text}");
    // The emergency exit's own key would come back: unbound.
    assert!(text.contains("<rebind input=\"kb1_ \"/>"), "{text}");
    let changes: Vec<_> = out
        .changes
        .iter()
        .map(|c| {
            (
                c.kind,
                c.action.as_str(),
                c.from.as_deref(),
                c.to.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        changes,
        [
            (
                ChangeKind::Changed,
                "seat_general/v_eject",
                Some("js1_button47 (double), kb1_ralt+y"),
                Some("js1_button47 (double), kb1_lalt+mouse2, mo1_mwheel_up")
            ),
            (
                ChangeKind::Changed,
                "seat_general/v_emergency_exit",
                Some("js1_button17, kb1_up"),
                Some("js1_button17")
            ),
        ]
    );

    // Read back: the same keys, none of them the game's default any more.
    let written = files_with(&t, text);
    let read = p
        .read(&ReadContext {
            target: &t,
            files: &written,
            catalog,
            ours: &ours(1),
            devices: &[],
        })
        .unwrap();
    for b in &binds {
        assert!(read.binds.contains(b), "{b:?} in {:?}", read.binds);
    }
    assert!(
        !read
            .binds
            .iter()
            .any(|b| b.action == "seat_general/v_emergency_exit" && b.output.starts_with("key:"))
    );
    // Written again over its own result, nothing changes.
    let again = write(&written, &binds, &cleared);
    assert_eq!(&again.files["actionmaps"], text);
    assert!(again.changes.is_empty(), "{:?}", again.changes);
}

#[test]
fn clears_the_game_defaults_on_js1() {
    let p = plugin();
    let t = target(&p);
    let catalog = p.catalogs.pick(Some("4.10.197.2878")).unwrap().0;
    let options = p.option_values(&BTreeMap::new()).unwrap();
    let files = files(&t, ACTIONMAPS);
    let out = p
        .write(&WriteContext {
            target: &t,
            files: &files,
            catalog,
            ours: &ours(1),
            devices: &[],
            binds: &[],
            cleared: &[],
            profile: "p",
            options: &options,
        })
        .unwrap();
    let defaults_on_js1 = catalog
        .actions
        .iter()
        .filter(|a| a.default.iter().any(|d| d.starts_with("js1_")))
        .count();
    assert!(defaults_on_js1 > 0);
    let unbinds = out.files["actionmaps"]
        .matches("<rebind input=\"js1_ \"/>")
        .count();
    assert_eq!(unbinds, defaults_on_js1);
}

fn files_with(t: &Target, actionmaps: &str) -> Files {
    files(t, actionmaps)
}

#[test]
fn every_plugin_in_the_repository_loads() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export");
    let found = discover(&[(dir, Origin::Bundled)]);
    assert!(!found.is_empty());
    for f in found {
        let p = f
            .plugin
            .unwrap_or_else(|e| panic!("{}: {e}", f.dir.display()));
        // DCS World lists every aircraft's actions from the game's own files instead.
        assert!(
            !p.catalogs.is_empty() || p.id() == "dcs-world",
            "{} has no catalog",
            p.id()
        );
        for hook in [Hook::Targets, Hook::Version, Hook::Read, Hook::Write] {
            assert!(p.has(hook), "{} lacks {hook:?}", p.id());
        }
    }
}
