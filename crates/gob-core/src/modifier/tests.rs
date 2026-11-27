use super::*;

fn modifier(name: &str, button: &str) -> Modifier {
    Modifier {
        name: name.into(),
        device_id: "{DEV-A}".into(),
        input_type: "button".into(),
        input_id: button.into(),
    }
}

#[test]
fn layers_are_sets() {
    let l = Layer::parse(" M2 + M1+M1 ");
    assert_eq!(l.to_string(), "M1+M2");
    assert_eq!(l, Layer::new(["M1", "M2"]));
    assert!(Layer::parse("").is_base());
    assert_eq!(Layer::base().to_string(), "");
}

/// Base, M1, M2, M1+M2 with the given additive flags; the layers that run for each held set.
fn selected(additive: [bool; 4]) -> Vec<Vec<String>> {
    let layers = ["", "M1", "M2", "M1+M2"].map(Layer::parse);
    layers
        .iter()
        .map(|held| {
            let s = Selection::new(layers.iter().zip(additive), held);
            layers
                .iter()
                .filter(|l| s.runs(l))
                .map(|l| {
                    if l.is_base() {
                        "base".into()
                    } else {
                        l.to_string()
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn exact_and_additive_layers() {
    // Issue #45: only the held layer, plus the base when it adds to it.
    let rows = |r: [&[&str]; 4]| -> Vec<Vec<String>> {
        r.iter()
            .map(|v| v.iter().map(|s| s.to_string()).collect())
            .collect()
    };
    assert_eq!(
        selected([false; 4]),
        rows([&["base"], &["M1"], &["M2"], &["M1+M2"]])
    );
    assert_eq!(
        selected([false, true, false, false]),
        rows([&["base"], &["base", "M1"], &["M2"], &["M1+M2"]])
    );
    assert_eq!(
        selected([false, false, true, false]),
        rows([&["base"], &["M1"], &["base", "M2"], &["M1+M2"]])
    );
    assert_eq!(
        selected([false, false, false, true]),
        rows([&["base"], &["M1"], &["M2"], &["base", "M1+M2"]])
    );
    assert_eq!(
        selected([false, true, true, true]),
        rows([
            &["base"],
            &["base", "M1"],
            &["base", "M2"],
            &["base", "M1+M2"]
        ])
    );
}

#[test]
fn undefined_combinations() {
    let (base, m1, m2) = (Layer::base(), Layer::parse("M1"), Layer::parse("M2"));
    // Exclusive M1: M2 held does nothing.
    let s = Selection::new([(&base, false), (&m1, false)], &m2);
    assert!(!s.runs(&base) && !s.runs(&m1));
    // Only additive layers (or none): the base keeps running.
    let s = Selection::new([(&base, false), (&m1, true)], &m2);
    assert!(s.runs(&base) && !s.runs(&m1));
    assert!(Selection::new([(&base, false)], &m1).runs(&base));
}

#[test]
fn add_rename_remove() {
    let mut p = Profile::default();
    assert!(p.add_modifier(modifier("M1", "2")));
    assert!(!p.add_modifier(modifier("M1", "3")), "name taken");
    assert!(!p.add_modifier(modifier("M2", "2")), "input taken");
    assert!(!p.add_modifier(modifier("A+B", "3")), "separator");
    assert!(p.add_modifier(modifier("M2", "3")));
    assert!(p.order.contains(&Section::Modifiers));
    assert_eq!(p.modifier_at("{dev-a}", "button", "3").unwrap().name, "M2");

    let base = p.bind("{DEV-B}", "button", "4", "Default");
    let layered = p.bind_layer("{DEV-B}", "button", "4", "Default", &Layer::parse("M1+M2"));
    assert_ne!(base, layered);
    assert_eq!(p.inputs[0].configs.len(), 2);

    assert!(p.rename_modifier("M2", "Shift"));
    assert_eq!(p.inputs[0].configs[1].layer.to_string(), "M1+Shift");
    assert!(!p.rename_modifier("M1", "Shift"), "name taken");

    assert!(!p.set_modifier_input("M1", "{DEV-A}", "button", "3"));
    assert!(p.set_modifier_input("M1", "{DEV-A}", "button", "5"));

    p.set_layer_additive(
        "{DEV-B}",
        "button",
        "4",
        "Default",
        &Layer::parse("M1+Shift"),
        true,
    );
    assert!(p.inputs[0].configs[1].additive);
    p.set_layer_keep_value("{dev-b}", "button", "4", "Default", &Layer::base(), true);
    assert!(p.inputs[0].configs[0].keep_value);
    assert!(!p.inputs[0].configs[1].keep_value);

    assert!(p.remove_modifier("Shift"));
    assert_eq!(p.inputs[0].configs.len(), 1);
    assert!(p.action(&layered).is_none(), "its actions are gone");
    assert!(p.action(&base).is_some());

    let m1 = p.bind_layer("{DEV-B}", "button", "4", "Default", &Layer::parse("M1"));
    p.unbind_layer("{DEV-B}", "button", "4", "Default", &Layer::base());
    assert!(p.action(&base).is_none());
    assert_eq!(p.inputs[0].configs.len(), 1, "the M1 layer stays");
    p.unbind_layer("{DEV-B}", "button", "4", "Default", &Layer::parse("M1"));
    assert!(p.inputs.is_empty() && p.action(&m1).is_none());
}
