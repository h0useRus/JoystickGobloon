use super::*;

#[test]
fn output_ids_round_trip() {
    for id in [
        "vjoy:1:button:5",
        "vjoy:2:axis:x",
        "vjoy:2:axis:dial",
        "vjoy:1:hat:1",
        "vjoy:1:hat:2:left",
        "gamepad:1:dpad:up",
        "gamepad:1:left-x",
        "logical:axis:2",
        "key:0x2e",
        "key:0x14b",
        "key:0x138+key:0x15",
        "mouse:left",
        "key:0x38+mouse:right",
        "mouse:wheel-up",
    ] {
        let o = GameOutput::parse(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(o.to_string(), id);
    }
    assert_eq!(
        GameOutput::parse("vjoy:1:axis:3").unwrap().to_string(),
        "vjoy:1:axis:z"
    );
    for bad in [
        "",
        "vjoy:0:button:1",
        "vjoy:1:axis:9",
        "key:46",
        "key:0x300",
        "x:1",
        "vjoy:1:button:1:up",
        "key:0x2e:up",
        "key:0x2e+",
        "mouse:up",
        "key:0x2e+vjoy:1:button:1",
    ] {
        assert!(GameOutput::parse(bad).is_none(), "{bad}");
    }
    let k = GameOutput::parse("key:0x14b").unwrap();
    assert_eq!(
        k,
        GameOutput::Keys(Combo::key(crate::Key {
            scan_code: 0x4b,
            extended: true
        }))
    );
    // The order pressed in does not matter: modifier keys come first.
    assert_eq!(
        GameOutput::parse("key:0x15+mouse:left+key:0x138+key:0x15")
            .unwrap()
            .to_string(),
        "key:0x138+key:0x15+mouse:left"
    );
    assert_eq!(
        GameOutput::parse("vjoy:3:button:1")
            .unwrap()
            .device()
            .unwrap(),
        "vjoy:3"
    );
}

#[test]
fn binding_and_unbinding() {
    let mut p = Profile::default();
    let out = GameOutput::parse("vjoy:1:button:5").unwrap();
    assert!(p.bind_game_action("g", "gear", out.clone()));
    assert!(!p.bind_game_action("g", "gear", out.clone()));
    assert!(p.bind_game_action("g", "lights", out.clone()));
    assert_eq!(p.order.last(), Some(&Section::GameBindings));
    let g = p.game_bindings("g").unwrap();
    assert_eq!(g.actions_of(&out).collect::<Vec<_>>(), ["gear", "lights"]);
    assert_eq!(g.outputs_of("gear").count(), 1);
    assert!(p.needs_own_format());
    p.unbind_game_action("g", "gear", None);
    p.unbind_game_action("g", "lights", Some(&out));
    assert!(p.game_bindings.is_empty());
}

#[test]
fn key_usage_lists_what_inputs_send() {
    use crate::edit::NewAction;
    let mut p = Profile::default();
    let key = |scan_code| Key {
        scan_code,
        extended: false,
    };
    let root = p.bind("dev", "button", "3", "Default");
    let id = p
        .add_child(
            &root,
            "actions",
            crate::Action::new(&NewAction::MapToKeyboard),
        )
        .unwrap();
    p.set_keys(&id, &[key(0x15), key(0x38)]);
    let usage = p.key_usage(None);
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].0.to_string(), "key:0x38+key:0x15");
    assert_eq!(usage[0].1[0].binding, 0);
}

#[test]
fn keys_are_the_games_until_set() {
    let mut p = Profile::default();
    let combo = |s| Combo::parse(s).unwrap();
    p.set_game_keys("g", "eject", Some(vec![combo("key:0x138+key:0x15")]));
    p.set_game_keys("g", "lights", Some(Vec::new()));
    let g = p.game_bindings("g").unwrap();
    assert_eq!(g.keys_of("eject").unwrap().len(), 1);
    assert_eq!(g.keys_of("lights"), Some(Vec::new()));
    assert_eq!(g.keys_of("gear"), None);
    // Binding a key ends "none at all".
    p.bind_game_action("g", "lights", GameOutput::Keys(combo("key:0x26")));
    assert!(p.game_bindings("g").unwrap().cleared.is_empty());
    p.set_game_keys("g", "lights", None);
    p.set_game_keys("g", "eject", None);
    assert!(p.game_bindings.is_empty());
}

#[test]
fn long_and_double_presses() {
    let long = GameOutput::parse("vjoy:1:button:5@long").unwrap();
    assert_eq!(long.press(), Some(crate::Press::Long));
    assert_eq!(long.base(), &GameOutput::parse("vjoy:1:button:5").unwrap());
    assert_eq!(long.to_string(), "vjoy:1:button:5@long");
    let hat = GameOutput::parse("vjoy:1:hat:1:up@double").unwrap();
    assert_eq!(hat.to_string(), "vjoy:1:hat:1:up@double");
    assert_eq!(hat.device().as_deref(), Some("vjoy:1"));
    // Axes and keys take no press; unknown presses are no output.
    assert_eq!(
        GameOutput::parse("vjoy:1:axis:x@long").unwrap().press(),
        None
    );
    assert_eq!(GameOutput::parse("key:0x2e@long").unwrap().press(), None);
    assert!(GameOutput::parse("vjoy:1:button:5@triple").is_none());
    assert_eq!(long.clone().with_press(None).to_string(), "vjoy:1:button:5");
    assert_eq!(
        long.with_press(Some(crate::Press::Double)).to_string(),
        "vjoy:1:button:5@double"
    );
}
