use gob_core::{GameDevice, GameOutput, Profile};

fn reload(p: &Profile) -> Profile {
    gob_gremlin::from_str(&gob_gremlin::to_string(p)).unwrap()
}

#[test]
fn game_bindings_round_trip() {
    let mut p = Profile::default();
    let out = |s| GameOutput::parse(s).unwrap();
    p.bind_game_action("sample", "gear", out("vjoy:1:button:5"));
    p.bind_game_action("sample", "roll", out("vjoy:2:axis:x"));
    p.bind_game_action("sample", "eject", out("key:0x12e"));
    p.bind_game_action("sample", "exit", out("key:0x2a+key:0x16"));
    p.bind_game_action("sample", "fire", out("key:0x38+mouse:right"));
    p.set_game_keys("sample", "lights", Some(Vec::new()));
    p.set_game_devices(
        "sample",
        vec![GameDevice {
            output: "vjoy:1".into(),
            game_device: "js1".into(),
        }],
    );
    {
        let g = p.game_bindings_mut("sample");
        g.target = "main".into();
        g.version = Some("1.1".into());
        g.binds[1].invert = true;
    }
    p.bind_game_action("other", "x", out("gamepad:1:a"));

    let text = gob_gremlin::to_string(&p);
    assert!(text.contains("<game-bindings>"), "{text}");
    assert!(
        text.contains("output=\"vjoy:2:axis:x\" invert=\"true\""),
        "{text}"
    );
    assert!(text.contains("<clear action=\"lights\"/>"), "{text}");
    assert!(text.contains("output=\"key:0x38+mouse:right\""), "{text}");
    assert!(text.contains("app=\"Gobloon\""), "{text}");
    let back = reload(&p);
    assert_eq!(back.game_bindings, p.game_bindings);
    assert_eq!(gob_gremlin::to_string(&back), text);
}

#[test]
fn unknown_outputs_are_skipped() {
    let text = r#"<profile app="Gobloon" version="1">
        <game-bindings>
            <game id="sample" target="main">
                <bind action="a" output="vjoy:1:button:2"/>
                <bind action="b" output="nonsense"/>
                <device output="vjoy:1"/>
            </game>
            <game target="no id"/>
        </game-bindings>
    </profile>"#;
    let p = gob_gremlin::from_str(text).unwrap();
    assert_eq!(p.game_bindings.len(), 1);
    let g = &p.game_bindings[0];
    assert_eq!(g.binds.len(), 1);
    assert!(g.devices.is_empty());
    assert_eq!(g.version, None);
}
