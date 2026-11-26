//! The window driven like a user would (egui events, no real mouse): worked inputs get numbers
//! that are dragged onto the picture, placed tags move and come off again; on a copy of
//! gob-devices' sample plugin in a temporary folder.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Pos2, Vec2};
use egui_kittest::Harness;
use gob_devices::{Direction, Entry, Input, Inputs, Kind, Manifest};

use crate::app::{MapApp, install_fonts};
use crate::settings::Own;
use crate::store::Store;

const SAMPLE: &str = "sample-stick";

/// A temporary folder with the sample plugin "shipped" in `shipped/sample/sample-stick`.
struct Folders {
    root: PathBuf,
}

impl Folders {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("map-gobloon-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let from =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../gob-devices/tests/plugins/sample");
        copy(&from, &root.join("shipped/sample/sample-stick"));
        Self { root }
    }

    fn shipped(&self) -> PathBuf {
        self.root.join("shipped")
    }

    fn user(&self) -> PathBuf {
        self.root.join("user")
    }

    /// The user's copy of the sample, where changes go.
    fn copy(&self) -> PathBuf {
        self.user().join("sample/sample-stick")
    }

    fn inputs(&self) -> Inputs {
        Inputs::load(&self.copy().join("inputs.gob")).unwrap()
    }

    fn manifest(&self) -> Manifest {
        toml::from_str(&std::fs::read_to_string(self.copy().join("device.gob")).unwrap()).unwrap()
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn app(folders: &Folders) -> MapApp {
    let store = Store::with(vec![folders.shipped(), folders.user()], folders.user());
    let i18n = gob_i18n::I18n::new(crate::LANGUAGES, None);
    let mut app = MapApp::with(i18n, store, None, Own::default());
    app.open(SAMPLE);
    app
}

fn harness(app: MapApp) -> Harness<'static, MapApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1280.0, 820.0))
        .with_context_setup(|ctx| install_fonts(ctx, false))
        .build_ui_state(|ui, app: &mut MapApp| app.show(ui), app);
    h.run_steps(3);
    h
}

/// Presses at `from`, moves to `to` in a few steps and lets go there.
fn drag(h: &mut Harness<'_, MapApp>, from: Pos2, to: Pos2) {
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    for i in 1..=6 {
        h.hover_at(from.lerp(to, i as f32 / 6.0));
        h.step();
    }
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.step();
}

fn spot(h: &Harness<'_, MapApp>, at: [f32; 2]) -> Pos2 {
    h.state().layout.to_screen(at).unwrap()
}

#[test]
fn worked_inputs_get_the_next_numbers() {
    let f = Folders::new("note");
    let mut app = app(&f);
    // Already on the sample's controls: they only light up.
    app.note(Input::Button(1));
    app.note(Input::HatDirection(1, Direction::Left));
    app.note(Input::Axis(2));
    assert!(app.pending.is_empty());
    app.note(Input::Button(7));
    app.note(Input::HatDirection(2, Direction::Up));
    app.note(Input::HatDirection(2, Direction::Down));
    app.note(Input::Button(7));
    assert_eq!(app.pending, [Input::Button(7), Input::Hat(2)]);
}

#[test]
fn a_waiting_number_dragged_onto_the_picture_becomes_a_control() {
    let f = Folders::new("place");
    let mut app = app(&f);
    app.note(Input::Button(7));
    app.note(Input::HatDirection(2, Direction::Right));
    let mut h = harness(app);
    let from = h.state().layout.pending(Input::Hat(2)).unwrap();
    let to = spot(&h, [80.0, 40.0]);
    drag(&mut h, from, to);
    assert_eq!(h.state().error, None);
    let manifest = f.manifest();
    let placed = manifest.controls.last().unwrap();
    assert_eq!(
        (placed.id.as_str(), placed.name.as_str(), placed.kind),
        ("hat_2", "Hat 2", Kind::Hat)
    );
    assert_eq!(placed.at, [80.0, 40.0]);
    assert_eq!(f.inputs().0.get("hat_2"), Some(&Entry::One(Input::Hat(2))));
    // The other number still waits; the shipped plugin stays as it was.
    assert_eq!(h.state().pending, [Input::Button(7)]);
    assert!(h.state().store.is_own(SAMPLE));
    let shipped: Manifest = toml::from_str(
        &std::fs::read_to_string(f.shipped().join("sample/sample-stick/device.gob")).unwrap(),
    )
    .unwrap();
    assert_eq!(shipped.controls.len(), 3);
}

#[test]
fn dropped_outside_the_picture_it_keeps_waiting() {
    let f = Folders::new("outside");
    let mut app = app(&f);
    app.note(Input::Axis(3));
    let mut h = harness(app);
    let from = h.state().layout.pending(Input::Axis(3)).unwrap();
    let (picture, _) = h.state().layout.picture.unwrap();
    let to = Pos2::new(picture.right() + 20.0, picture.center().y);
    drag(&mut h, from, to);
    assert_eq!(h.state().pending, [Input::Axis(3)]);
    assert!(!f.copy().exists());
}

#[test]
fn tags_move_and_come_off() {
    let f = Folders::new("tag");
    let mut h = harness(app(&f));
    let from = spot(&h, [40.0, 30.0]);
    let to = spot(&h, [20.0, 60.0]);
    drag(&mut h, from, to);
    let trigger = f
        .manifest()
        .controls
        .into_iter()
        .find(|c| c.id == "trigger")
        .unwrap();
    assert_eq!(trigger.at, [20.0, 60.0]);
    assert_eq!(h.state().selected.as_deref(), Some("trigger"));
    h.state_mut().delete_control("trigger");
    assert!(f.manifest().controls.iter().all(|c| c.id != "trigger"));
    assert_eq!(f.inputs().get("trigger", "first"), None);
    // Its inputs can be placed again.
    h.state_mut().note(Input::Button(1));
    assert_eq!(h.state().pending, [Input::Button(1)]);
}

#[test]
fn new_device_from_a_picture() {
    let f = Folders::new("device");
    let mut store = Store::with(vec![f.shipped(), f.user()], f.user());
    let picture = f.shipped().join("sample/sample-stick/views/main.svg");
    let id = store
        .create(
            "Acme Sim",
            "Space Stick (L)",
            &picture,
            Some((0x1234, "ACME Stick L")),
        )
        .unwrap();
    assert_eq!(id, "acme-sim-space-stick-l");
    let plugin = store.get(&id).unwrap();
    assert_eq!(plugin.dir, f.user().join("acme-sim/space-stick-l"));
    assert!(plugin.pictures.contains_key("main"));
    assert_eq!(plugin.score(0x1234, 1, "ACME Stick L rev 2"), Some(1));
    assert!(
        store
            .create("Acme Sim", "Space Stick (L)", &picture, None)
            .is_err()
    );
    assert_eq!(store.brands(), ["Acme Sim", "Sample"]);
}
