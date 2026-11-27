use gob_core::{CardSide, PictureCard, Profile, Section};

#[test]
fn picture_cards_survive_save() {
    let mut p = Profile::default();
    p.set_picture_card(PictureCard {
        plugin: "vkb-gladiator".into(),
        view: "main".into(),
        control: "hat-2".into(),
        side: CardSide::Right,
        y: 120.0,
    });
    let text = gob_gremlin::to_string(&p);
    assert!(
        text.contains(
            r#"<card plugin="vkb-gladiator" view="main" control="hat-2" side="right" y="120"/>"#
        ),
        "{text}"
    );
    let back = gob_gremlin::from_str(&text).unwrap();
    assert_eq!(back.picture_cards, p.picture_cards);
    assert!(back.order.contains(&Section::PictureCards));
}
