//! Where the cards of a controller's picture go (Joystick Gobloon only): the Game page's
//! Controller tab puts a card per control beside the picture; a card dragged elsewhere keeps its
//! side and height here, per device plugin, view and control (`<picture-cards>`).

use crate::profile::{Profile, Section};

/// The column a card goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardSide {
    Left,
    Right,
}

/// One placed card (`<card>` in `<picture-cards>`).
#[derive(Debug, Clone, PartialEq)]
pub struct PictureCard {
    /// The device plugin's id.
    pub plugin: String,
    pub view: String,
    /// The control's id in the plugin.
    pub control: String,
    pub side: CardSide,
    /// The card's top, in the picture's units (it may lie above or below the picture).
    pub y: f32,
}

impl Profile {
    /// Where the card of `control` was placed, if it was.
    pub fn picture_card(&self, plugin: &str, view: &str, control: &str) -> Option<&PictureCard> {
        self.picture_cards
            .iter()
            .find(|c| c.plugin == plugin && c.view == view && c.control == control)
    }

    /// Places a card (replacing where it was).
    pub fn set_picture_card(&mut self, card: PictureCard) {
        match self
            .picture_cards
            .iter_mut()
            .find(|c| c.plugin == card.plugin && c.view == card.view && c.control == card.control)
        {
            Some(c) => *c = card,
            None => self.picture_cards.push(card),
        }
        self.add_section(Section::PictureCards);
    }

    /// The cards of a plugin's view back where they go by themselves.
    pub fn reset_picture_cards(&mut self, plugin: &str, view: &str) {
        self.picture_cards
            .retain(|c| c.plugin != plugin || c.view != view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(control: &str, y: f32) -> PictureCard {
        PictureCard {
            plugin: "p".into(),
            view: "main".into(),
            control: control.into(),
            side: CardSide::Left,
            y,
        }
    }

    #[test]
    fn set_replaces_and_reset_clears() {
        let mut p = Profile::default();
        p.set_picture_card(card("a", 10.0));
        p.set_picture_card(card("a", 20.0));
        p.set_picture_card(card("b", 5.0));
        assert_eq!(p.picture_cards.len(), 2);
        assert_eq!(p.picture_card("p", "main", "a").map(|c| c.y), Some(20.0));
        assert!(p.order.contains(&Section::PictureCards));
        p.reset_picture_cards("p", "main");
        assert!(p.picture_cards.is_empty());
    }
}
