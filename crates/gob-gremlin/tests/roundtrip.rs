use gob_core::{
    ActionKind, ActivationMode, InputType, Item, MacroRepeat, MacroStep, Profile, Section,
};
use gob_gremlin::{Warning, profile_to_element, report, xmltext};

const SAMPLE: &str = include_str!("../../../tests/fixtures/sample_r14.xml");
const LOCAL_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/local");

/// Import -> export must reproduce the same XML tree (with the header its content calls for),
/// and export must be stable.
fn assert_round_trip(text: &str) -> Profile {
    let mut original = xmltext::parse(text).unwrap();
    let profile = gob_gremlin::from_str(text).unwrap();
    original.attrs = if profile.needs_own_format() {
        vec![
            ("app".into(), gob_gremlin::APP.into()),
            ("version".into(), gob_gremlin::FORMAT_VERSION.to_string()),
        ]
    } else {
        vec![("version".into(), gob_gremlin::MIN_VERSION.to_string())]
    };
    assert_eq!(
        profile_to_element(&profile),
        original,
        "tree changed on round-trip"
    );

    let written = gob_gremlin::to_string(&profile);
    assert_eq!(
        xmltext::parse(&written).unwrap(),
        original,
        "written XML parses differently"
    );
    assert_eq!(
        gob_gremlin::to_string(&gob_gremlin::from_str(&written).unwrap()),
        written,
        "export not stable"
    );
    profile
}

#[test]
fn sample_round_trips() {
    let p = assert_round_trip(SAMPLE);
    assert_eq!(p.inputs.len(), 2);
    assert_eq!(p.library.len(), 9);
    assert_eq!(p.startup_mode().as_deref(), Some("Default"));
    assert_eq!(p.modes[1].parent.as_deref(), Some("Default"));
    assert_eq!(p.devices[0].name, " Test Stick  ");
    assert_eq!(p.device_name(&p.devices[0].id), Some("Test Stick"));
    assert!(p.has_scripts());
    assert_eq!(
        p.order
            .iter()
            .filter(|s| matches!(s, Section::Extra(_)))
            .count(),
        1
    );
}

#[test]
fn sample_typed_views() {
    let p = gob_gremlin::from_str(SAMPLE).unwrap();
    let get = |suffix: &str| p.library.iter().find(|a| a.id.ends_with(suffix)).unwrap();

    let root = get("r001");
    assert_eq!(root.activation_mode(), ActivationMode::Disallowed);
    assert!(matches!(root.kind(), ActionKind::Root { children } if children.len() == 2));

    let vjoy = get("v001");
    assert_eq!(vjoy.label(), Some(r#"Fire & "forget" <group>"#));
    assert_eq!(vjoy.activation_mode(), ActivationMode::Press);
    assert_eq!(
        vjoy.kind(),
        ActionKind::MapToVjoy {
            device: 1,
            input_type: InputType::Button,
            input_id: 3,
            inverted: true
        }
    );

    assert_eq!(
        get("c001").kind(),
        ActionKind::ChangeMode {
            change_type: "Temporary",
            target: Some("Combat")
        }
    );
    match get("t001").kind() {
        ActionKind::Tempo {
            short,
            long,
            threshold,
            activate_on,
        } => {
            assert_eq!(
                (short.len(), long.len(), threshold, activate_on),
                (1, 2, 0.5, "press")
            );
        }
        k => panic!("unexpected {k:?}"),
    }
    match get("k001").kind() {
        ActionKind::ResponseCurve {
            deadzone, points, ..
        } => {
            assert_eq!(deadzone, [-0.9, -0.05, 0.05, 0.9]);
            assert_eq!(points, vec![(-1.0, -1.0), (0.0, 0.2), (1.0, 1.0)]);
        }
        k => panic!("unexpected {k:?}"),
    }
    assert_eq!(get("d001").kind(), ActionKind::Description { text: "" });
    assert_eq!(
        get("m001").kind(),
        ActionKind::Macro {
            steps: vec![
                MacroStep::Key {
                    scan_code: 62,
                    extended: false,
                    pressed: false
                },
                MacroStep::Pause(0.1)
            ],
            repeat: MacroRepeat::Single,
            exclusive: false,
            preemptive: false
        }
    );

    let odd = get("x001");
    assert!(
        odd.items.iter().all(|i| matches!(i, Item::Raw(_))),
        "odd shapes must stay raw"
    );
}

#[test]
fn sample_report() {
    let r = report(&gob_gremlin::from_str(SAMPLE).unwrap());
    assert_eq!((r.inputs, r.actions, r.devices, r.modes), (2, 9, 1, 2));
    assert_eq!(r.by_kind["root"], 2);
    assert!(r.warnings.contains(&Warning::HasScripts));
    assert!(
        !r.warnings
            .iter()
            .any(|w| matches!(w, Warning::NotExecutable { kind, .. } if kind == "macro"))
    );
    assert!(r.warnings.contains(&Warning::NotExecutable {
        kind: "future-action".into(),
        count: 1
    }));
    assert!(
        !r.warnings
            .iter()
            .any(|w| matches!(w, Warning::DanglingRef { .. } | Warning::UnknownMode(_)))
    );
}

#[test]
fn report_finds_broken_references() {
    let broken = SAMPLE
        .replace("<mode>Combat</mode>", "<mode>Missing</mode>")
        .replace("00000000c001</action-id>", "deadbeefc001</action-id>");
    let r = report(&gob_gremlin::from_str(&broken).unwrap());
    assert!(r.warnings.contains(&Warning::UnknownMode("Missing".into())));
    assert!(
        r.warnings
            .iter()
            .any(|w| matches!(w, Warning::DanglingRef { to, .. } if to.ends_with("deadbeefc001")))
    );
}

#[test]
fn rejects_old_and_foreign_files() {
    assert!(matches!(
        gob_gremlin::from_str(r#"<profile version="9"><devices/></profile>"#),
        Err(gob_gremlin::Error::UnsupportedVersion(_))
    ));
    assert!(matches!(
        gob_gremlin::from_str("<html/>"),
        Err(gob_gremlin::Error::NotAProfile)
    ));
    assert!(gob_gremlin::from_str("<profile").is_err());
}

/// Real-world profiles in `tests/fixtures/local/` (gitignored) must round-trip too.
#[test]
fn local_profiles_round_trip() {
    let Ok(entries) = std::fs::read_dir(LOCAL_DIR) else {
        return;
    };
    for path in entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
    {
        let text = std::fs::read_to_string(&path).unwrap();
        let p = assert_round_trip(&text);
        let r = report(&p);
        assert!(
            !r.warnings
                .iter()
                .any(|w| matches!(w, Warning::DanglingRef { .. })),
            "{}: dangling references",
            path.display()
        );
        println!(
            "{}: {} inputs, {} actions {:?}",
            path.display(),
            r.inputs,
            r.actions,
            r.by_kind
        );
    }
}

#[test]
fn substitute_device_moves_all_references() {
    let old = "11111111-0000-0000-0000-000000000001";
    let new = "22222222-0000-0000-0000-000000000002";
    // A property referencing the device, as conditions and merge actions do.
    let text = SAMPLE.replace(
        "<name>repeat-mode</name>",
        &format!("<name>device</name>\n<value>{old}</value>\n</property>\n<property type=\"string\">\n<name>repeat-mode</name>"),
    );
    let mut p = gob_gremlin::from_str(&text).unwrap();
    let n = p.substitute_device(old, new, "Test Stick 2");
    assert_eq!(n, 3, "two inputs and one action property");
    assert!(p.inputs.iter().all(|b| b.device_id == new));
    assert_eq!(
        (p.devices[0].id.as_str(), p.devices[0].name.as_str()),
        (new, "Test Stick 2")
    );
    assert!(!gob_gremlin::to_string(&p).contains(old));
}

#[test]
fn device_names_match_ignoring_padding() {
    assert!(gob_core::same_device_name(
        " VKBsim Gladiator EVO  R  ",
        "VKBsim Gladiator EVO R"
    ));
    assert!(!gob_core::same_device_name(
        "VKBsim Gladiator EVO L",
        "VKBsim Gladiator EVO R"
    ));
}

#[test]
fn multiple_configs_and_virtual_buttons_round_trip() {
    let extra = r#"<action-configuration>
                <root-action>00000000-0000-0000-0000-00000000r001</root-action>
                <behavior>button</behavior>
                <virtual-button>
                    <lower-limit>0.5</lower-limit>
                    <upper-limit>1.0</upper-limit>
                    <axis-button-direction>above</axis-button-direction>
                </virtual-button>
            </action-configuration>
        </input>"#;
    // Give the axis input (second <input>) a second, button-like sequence.
    let at = SAMPLE.match_indices("</input>").nth(1).unwrap().0;
    let text = format!(
        "{}{}{}",
        &SAMPLE[..at],
        extra,
        &SAMPLE[at + "</input>".len()..]
    );
    let p = assert_round_trip(&text);
    let axis = &p.inputs[1];
    assert_eq!(axis.configs.len(), 2);
    assert_eq!(
        axis.configs[1].virtual_button(axis.input_type()),
        Some(gob_core::VirtualButton::Axis {
            lower: 0.5,
            upper: 1.0,
            direction: gob_core::AxisDirection::Above
        })
    );
    assert_eq!(axis.configs[0].virtual_button(axis.input_type()), None);
}

#[test]
fn modifiers_and_layers_round_trip() {
    use gob_core::{Layer, Modifier};
    let mut p = Profile::default();
    for (name, button) in [("M1", "2"), ("M2", "3")] {
        assert!(p.add_modifier(Modifier {
            name: name.into(),
            device_id: "{DEV-A}".into(),
            input_type: "button".into(),
            input_id: button.into(),
        }));
    }
    p.bind("{DEV-B}", "button", "4", "Default");
    p.bind_layer("{DEV-B}", "button", "4", "Default", &Layer::parse("M2+M1"));
    p.bind_layer("{DEV-B}", "axis", "2", "Default", &Layer::parse("M1"));
    p.inputs[1].configs[0].additive = true;
    p.inputs[1].configs[0].keep_value = true;

    let written = gob_gremlin::to_string(&p);
    assert!(
        written.contains(
            r#"<modifier name="M1" device-id="{DEV-A}" input-type="button" input-id="2"/>"#
        )
    );
    assert!(written.contains(r#"<action-configuration layer="M1+M2">"#));
    assert!(
        written.contains(r#"<action-configuration layer="M1" additive="true" keep-value="true">"#)
    );
    assert_round_trip(&written);
    assert_eq!(gob_gremlin::from_str(&written).unwrap(), p);

    // Without modifiers the section is left out.
    assert!(!gob_gremlin::to_string(&Profile::default()).contains("modifiers"));
}

#[test]
fn virtual_devices_round_trip() {
    use gob_core::{VirtualDevices, VirtualJoystick};
    let mut p = Profile::default();
    // The section is left out while there is nothing in it.
    assert!(!gob_gremlin::to_string(&p).contains("virtual-devices"));
    p.set_virtual_devices(VirtualDevices {
        joysticks: vec![
            VirtualJoystick {
                id: 1,
                axes: (1..=8).collect(),
                buttons: 128,
                hats: 4,
                discrete_hats: false,
            },
            VirtualJoystick {
                id: 3,
                axes: vec![1, 2, 6],
                buttons: 16,
                hats: 1,
                discrete_hats: true,
            },
        ],
        gamepads: vec![1, 2],
    });
    let written = gob_gremlin::to_string(&p);
    assert!(written.contains(
        r#"<joystick id="1" axes="X Y Z Rx Ry Rz Sl0 Sl1" buttons="128" hats="4" hat-type="continuous"/>"#
    ));
    assert!(
        written.contains(
            r#"<joystick id="3" axes="X Y Rz" buttons="16" hats="1" hat-type="discrete"/>"#
        )
    );
    assert!(written.contains(r#"<gamepad id="2"/>"#));
    assert!(written.contains(r#"<profile app="Gobloon" version="1">"#));
    let read = assert_round_trip(&written);
    assert_eq!(read.virtual_devices, p.virtual_devices);
    assert!(
        read.order
            .ends_with(&[Section::Devices, Section::VirtualDevices])
    );
}

#[test]
fn format_headers() {
    let body = "<inputs/><modes><mode>Default</mode></modes>";
    let parse = |root: &str| gob_gremlin::from_str(&format!("{root}{body}</profile>"));
    let mut p = parse(r#"<profile version="14">"#).unwrap();
    assert_eq!(p.source, "Joystick Gremlin 14");
    assert!(gob_gremlin::to_string(&p).contains(r#"<profile version="14">"#));
    p.add_modifier(gob_core::Modifier {
        name: "M1".into(),
        device_id: "{DEV-A}".into(),
        input_type: "button".into(),
        input_id: "2".into(),
    });
    assert!(gob_gremlin::to_string(&p).contains(r#"<profile app="Gobloon" version="1">"#));
    assert_eq!(
        parse(r#"<profile app="Gobloon" version="1">"#)
            .unwrap()
            .source,
        "Joystick Gobloon 1"
    );
    assert!(matches!(
        parse(r#"<profile app="Gobloon" version="2">"#),
        Err(gob_gremlin::Error::NewerFormat(_))
    ));
    assert!(matches!(
        parse(r#"<profile version="13">"#),
        Err(gob_gremlin::Error::UnsupportedVersion(_))
    ));
    assert!(matches!(
        parse(r#"<profile app="Other" version="1">"#),
        Err(gob_gremlin::Error::NotAProfile)
    ));
}
