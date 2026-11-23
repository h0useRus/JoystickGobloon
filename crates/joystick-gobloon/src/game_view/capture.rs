//! Taking the control the user presses: keys and mouse buttons as one combination once they are
//! all let go, or the first joystick button, hat or axis that moves.

use std::collections::HashSet;

use gob_core::{Combo, ComboPart, Key, MouseButton};

use super::*;

/// An axis counts once it moved this far from where it rested.
const AXIS_MOVE: f32 = 0.5;

const ESCAPE: Key = Key {
    scan_code: 0x01,
    extended: false,
};

const POINTER: [(egui::PointerButton, MouseButton); 5] = [
    (egui::PointerButton::Primary, MouseButton::Left),
    (egui::PointerButton::Secondary, MouseButton::Right),
    (egui::PointerButton::Middle, MouseButton::Middle),
    (egui::PointerButton::Extra1, MouseButton::Back),
    (egui::PointerButton::Extra2, MouseButton::Forward),
];

/// A physical joystick input.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Pressed {
    /// The device's id on this computer.
    pub(super) device: String,
    pub(super) device_name: String,
    pub(super) kind: InputType,
    /// 1-based button or hat number, axis slot.
    pub(super) id: u32,
    /// Where a hat was pushed.
    pub(super) hat: Option<HatDirection>,
}

pub(super) enum Captured {
    Keys(Combo),
    Input(Pressed),
}

/// One wait for a press.
#[derive(Default)]
pub(super) struct Capture {
    parts: Vec<ComboPart>,
    /// Keys held when the wait began: they count once let go and pressed again.
    ignore: Option<Vec<Key>>,
    watch: crate::key_capture::KeyWatch,
    /// Mouse buttons of `parts` still down.
    mouse_down: Vec<MouseButton>,
    /// Joystick buttons and hats on when the wait began, and where the axes rested.
    rest: Option<Rest>,
}

#[derive(Default)]
struct Rest {
    /// (device, hat?, index).
    on: HashSet<(String, bool, usize)>,
    axes: HashMap<(String, usize), f32>,
}

fn hat_direction(angle: u16) -> HatDirection {
    match ((u32::from(angle) + 4500) / 9000) % 4 {
        0 => HatDirection::Up,
        1 => HatDirection::Right,
        2 => HatDirection::Down,
        _ => HatDirection::Left,
    }
}

impl Capture {
    /// The keys and mouse buttons taken so far.
    pub(super) fn parts(&self) -> &[ComboPart] {
        &self.parts
    }

    /// Call every frame of the wait. Mouse buttons and the wheel count only over `mouse`, so
    /// the dialog around it stays clickable.
    pub(super) fn poll(
        &mut self,
        ui: &egui::Ui,
        devices: &[DeviceState],
        mouse: Option<Rect>,
    ) -> Option<Captured> {
        crate::key_capture::mark_recording(ui.ctx());
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        if let Some(pressed) = self.joystick(devices) {
            return Some(Captured::Input(pressed));
        }
        let held = crate::key_capture::held(ui, &mut self.watch);
        let ignore = self.ignore.get_or_insert_with(|| held.clone());
        ignore.retain(|k| held.contains(k));
        let held: Vec<Key> = held
            .into_iter()
            .filter(|k| !ignore.contains(k) && *k != ESCAPE)
            .collect();
        for k in &held {
            self.add(ComboPart::Key(*k));
        }
        if mouse.is_some_and(|r| ui.rect_contains_pointer(r)) {
            for (pointer, button) in POINTER {
                if ui.input(|i| i.pointer.button_pressed(pointer)) {
                    self.add(ComboPart::Mouse(button));
                    if !self.mouse_down.contains(&button) {
                        self.mouse_down.push(button);
                    }
                }
            }
            let wheel = ui.input(|i| {
                i.events
                    .iter()
                    .find_map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                        _ => None,
                    })
                    .unwrap_or(0.0)
            });
            if wheel > 0.0 {
                self.add(ComboPart::Mouse(MouseButton::WheelUp));
            } else if wheel < 0.0 {
                self.add(ComboPart::Mouse(MouseButton::WheelDown));
            }
        }
        self.mouse_down.retain(|b| {
            POINTER
                .iter()
                .find(|(_, m)| m == b)
                .is_some_and(|(p, _)| ui.input(|i| i.pointer.button_down(*p)))
        });
        if !self.parts.is_empty() && held.is_empty() && self.mouse_down.is_empty() {
            return Combo::new(self.parts.drain(..)).map(Captured::Keys);
        }
        None
    }

    fn add(&mut self, part: ComboPart) {
        if !self.parts.contains(&part) {
            self.parts.push(part);
        }
    }

    /// The first button or hat that went on, or axis that moved, since the wait began; the
    /// program's own virtual devices do not count (they only repeat what the profile sends).
    fn joystick(&mut self, devices: &[DeviceState]) -> Option<Pressed> {
        let first = self.rest.is_none();
        let rest = self.rest.get_or_insert_with(Rest::default);
        let mut found = None;
        for d in devices.iter().filter(|d| !d.info.is_own()) {
            let guid = &d.info.guid;
            let pressed = |kind, id: usize, hat| Pressed {
                device: guid.clone(),
                device_name: d.info.name.clone(),
                kind,
                id: id as u32 + 1,
                hat,
            };
            for (n, on) in d.buttons.iter().enumerate() {
                let key = (guid.clone(), false, n);
                if !*on {
                    rest.on.remove(&key);
                } else if first {
                    rest.on.insert(key);
                } else if !rest.on.contains(&key) && found.is_none() {
                    found = Some(pressed(InputType::Button, n, None));
                }
            }
            for (n, angle) in d.hats.iter().enumerate() {
                let key = (guid.clone(), true, n);
                match angle {
                    None => {
                        rest.on.remove(&key);
                    }
                    Some(_) if first => {
                        rest.on.insert(key);
                    }
                    Some(angle) if !rest.on.contains(&key) && found.is_none() => {
                        found = Some(pressed(InputType::Hat, n, Some(hat_direction(*angle))));
                    }
                    Some(_) => {}
                }
            }
            for &slot in &d.info.axes {
                let n = usize::from(slot).wrapping_sub(1);
                let Some(v) = d.axes.get(n).copied() else {
                    continue;
                };
                let base = *rest.axes.entry((guid.clone(), n)).or_insert(v);
                if (v - base).abs() > AXIS_MOVE && found.is_none() {
                    found = Some(pressed(InputType::Axis, n, None));
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(buttons: usize) -> DeviceState {
        DeviceState::new(gob_input::DeviceInfo {
            guid: "dev".into(),
            name: "Stick".into(),
            axes: vec![1, 2],
            buttons,
            hats: 1,
            ..Default::default()
        })
    }

    #[test]
    fn a_held_button_counts_once_let_go() {
        let mut d = device(4);
        d.buttons[1] = true;
        d.axes[0] = 0.8;
        let mut c = Capture::default();
        // What is on at the start is not a press; a resting axis off center neither.
        assert_eq!(c.joystick(std::slice::from_ref(&d)), None);
        assert_eq!(c.joystick(std::slice::from_ref(&d)), None);
        d.buttons[1] = false;
        assert_eq!(c.joystick(std::slice::from_ref(&d)), None);
        d.buttons[1] = true;
        let p = c.joystick(std::slice::from_ref(&d)).unwrap();
        assert_eq!((p.kind, p.id), (InputType::Button, 2));
    }

    #[test]
    fn hats_and_axes() {
        let mut d = device(1);
        let mut c = Capture::default();
        assert_eq!(c.joystick(std::slice::from_ref(&d)), None);
        d.hats[0] = Some(9000);
        let p = c.joystick(std::slice::from_ref(&d)).unwrap();
        assert_eq!(
            (p.kind, p.id, p.hat),
            (InputType::Hat, 1, Some(HatDirection::Right))
        );
        let mut d = device(1);
        let mut c = Capture::default();
        c.joystick(std::slice::from_ref(&d));
        d.axes[1] = -0.7;
        let p = c.joystick(std::slice::from_ref(&d)).unwrap();
        assert_eq!((p.kind, p.id), (InputType::Axis, 2));
        assert_eq!(hat_direction(31500), HatDirection::Up);
        assert_eq!(hat_direction(22500), HatDirection::Left);
    }
}
