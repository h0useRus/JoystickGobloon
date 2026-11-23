use gob_core::{
    ActionKind, NewAction, OSC_GUID, OscArg, OscArgKind, OscKind, Profile, Section, is_osc_binding,
};

fn reload(p: &Profile) -> Profile {
    gob_gremlin::from_str(&gob_gremlin::to_string(p)).unwrap()
}

#[test]
fn osc_inputs_and_send_osc_survive_save() {
    let mut p = Profile::default();
    let fire = p.add_osc_input("/deck/fire", OscKind::Button).unwrap();
    let fader = p.add_osc_input("/panel/fader1", OscKind::Axis).unwrap();
    p.set_osc_range(fader, 0.0, 127.0);
    let root = p.bind(OSC_GUID, "button", &fire.to_string(), "Default");
    let send = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::SendOsc))
        .unwrap();
    p.action_mut(&send)
        .unwrap()
        .set("address", "string", "/location/1/0/0/style/text");
    p.set_osc_args(
        &send,
        &[
            OscArg::new(OscArgKind::Text, "Gear <down> & \"locked\""),
            OscArg::new(OscArgKind::Input, ""),
        ],
    );
    let text = gob_gremlin::to_string(&p);
    assert!(text.contains(r#"app="Gobloon""#), "own format: {text}");
    assert!(
        text.contains(r#"<input id="1" address="/deck/fire" type="button"/>"#)
            && text.contains(
                r#"<input id="2" address="/panel/fader1" type="axis" low="0" high="127"/>"#
            ),
        "{text}"
    );
    let back = reload(&p);
    assert_eq!(back.osc_inputs, p.osc_inputs);
    assert_eq!(back.inputs, p.inputs);
    assert_eq!(back.library, p.library);
    assert!(back.order.contains(&Section::OscInputs));
    match back.action(&send).unwrap().kind() {
        ActionKind::SendOsc { address, args, .. } => {
            assert_eq!(address, "/location/1/0/0/style/text");
            assert_eq!(args[0].value, "Gear <down> & \"locked\"");
            assert_eq!(args[1].kind, OscArgKind::Input);
        }
        other => panic!("{other:?}"),
    }
    // Saved again unchanged.
    assert_eq!(gob_gremlin::to_string(&back), text);
}

#[test]
fn broken_osc_inputs_are_skipped() {
    let xml = r#"<profile app="Gobloon" version="1"><osc-inputs>
        <input id="1" address="/a" type="axis" low="1" high="1"/>
        <input id="1" address="/b"/>
        <input id="2" address="/a"/>
        <input address="/c"/>
        <input id="3" address=" "/>
        <input id="4" address="d" type="nonsense"/>
    </osc-inputs></profile>"#;
    let p = gob_gremlin::from_str(xml).unwrap();
    let got: Vec<_> = p
        .osc_inputs
        .iter()
        .map(|i| (i.id, i.address.as_str(), i.kind, i.low, i.high))
        .collect();
    assert_eq!(
        got,
        [
            (1, "/a", OscKind::Axis, 0.0, 1.0),
            (4, "/d", OscKind::Button, 0.0, 1.0)
        ]
    );
    assert!(gob_core::is_builtin_device(OSC_GUID));
    assert!(is_osc_binding(&OSC_GUID.to_uppercase()));
}
