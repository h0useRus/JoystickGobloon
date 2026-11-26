//! The window driven like a user would (egui events, no real mouse): chips dragged onto the
//! picture, tags moved, on a copy of gob-devices' sample plugin in a temporary folder.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Pos2, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gob_devices::{Direction, Entry, Input, Inputs, Manifest};

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

fn harness(folders: &Folders, recent: Vec<Input>) -> Harness<'static, MapApp> {
    let store = Store::with(vec![folders.shipped(), folders.user()], folders.user());
    let i18n = gob_i18n::I18n::new(crate::LANGUAGES, None);
    let mut app = MapApp::with(i18n, store, None, Own::default());
    app.open(SAMPLE);
    app.recent = recent;
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

/// Where a control's tag is on screen.
fn tag(h: &Harness<'_, MapApp>, control: &str) -> Pos2 {
    let app = h.state();
    let at = app.plugin().unwrap().control(control).unwrap().at;
    app.layout.to_screen(at).unwrap()
}

fn chip(h: &Harness<'_, MapApp>, input: Input) -> Pos2 {
    h.state().layout.chip(input).expect("chip shown").center()
}

#[test]
fn chip_on_a_part_slot() {
    let f = Folders::new("slot");
    let mut h = harness(&f, vec![Input::Button(5)]);
    // The trigger's second stage sits right of its tag.
    let to = tag(&h, "trigger") + Vec2::new(32.0, 0.0);
    let from = chip(&h, Input::Button(5));
    drag(&mut h, from, to);
    assert_eq!(h.state().error, None);
    let inputs = f.inputs();
    assert_eq!(inputs.get("trigger", "second"), Some(Input::Button(5)));
    assert_eq!(inputs.get("trigger", "first"), Some(Input::Button(1)));
    // The shipped plugin stays as it was; Map Gobloon now shows the copy.
    let shipped = Inputs::load(&f.shipped().join("sample/sample-stick/inputs.gob")).unwrap();
    assert_eq!(shipped.get("trigger", "second"), Some(Input::Button(2)));
    assert!(h.state().store.is_own(SAMPLE));
}

#[test]
fn an_input_moves_from_its_old_part() {
    let f = Folders::new("move-input");
    let mut h = harness(&f, vec![Input::Button(2)]);
    // The hat's push sits to its lower right.
    let d = std::f32::consts::FRAC_1_SQRT_2 * 32.0;
    let to = tag(&h, "hat") + Vec2::new(d, d);
    let from = chip(&h, Input::Button(2));
    drag(&mut h, from, to);
    let inputs = f.inputs();
    assert_eq!(inputs.get("hat", "push"), Some(Input::Button(2)));
    assert_eq!(inputs.get("trigger", "second"), None);
}

#[test]
fn hat_direction_on_a_hat_takes_the_whole_hat() {
    let f = Folders::new("hat");
    let input = Input::HatDirection(2, Direction::Left);
    let mut h = harness(&f, vec![input]);
    let (from, to) = (chip(&h, input), tag(&h, "hat"));
    drag(&mut h, from, to);
    let inputs = f.inputs();
    for d in Direction::ALL {
        assert_eq!(inputs.get("hat", d.name()), Some(Input::HatDirection(2, d)));
    }
    assert_eq!(inputs.get("hat", "push"), Some(Input::Button(3)));
    assert!(matches!(inputs.0.get("hat"), Some(Entry::Parts(_))));
}

#[test]
fn axis_on_a_stick() {
    let f = Folders::new("axis");
    let mut h = harness(&f, vec![Input::Axis(3)]);
    let (from, to) = (chip(&h, Input::Axis(3)), tag(&h, "stick"));
    drag(&mut h, from, to);
    // Both axis parts are taken: the first one gets it.
    assert_eq!(f.inputs().get("stick", "x"), Some(Input::Axis(3)));
}

#[test]
fn chip_on_an_empty_spot_asks_for_a_new_control() {
    let f = Folders::new("new");
    let mut h = harness(&f, vec![Input::Button(9)]);
    let to = h.state().layout.to_screen([95.0, 55.0]).unwrap();
    let from = chip(&h, Input::Button(9));
    drag(&mut h, from, to);
    assert!(h.state().new_control.is_some());
    h.get_by_label("Add").click();
    h.run_steps(3);
    let manifest = f.manifest();
    let added = manifest.controls.last().unwrap();
    assert_eq!(
        (added.id.as_str(), added.name.as_str()),
        ("button_9", "Button 9")
    );
    assert_eq!(added.at, [95.0, 55.0]);
    assert_eq!(f.inputs().get("button_9", ""), Some(Input::Button(9)));
    assert_eq!(h.state().selected.as_deref(), Some("button_9"));
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

#[test]
fn tags_move() {
    let f = Folders::new("tag");
    let mut h = harness(&f, Vec::new());
    let from = tag(&h, "trigger");
    let to = h.state().layout.to_screen([20.0, 60.0]).unwrap();
    drag(&mut h, from, to);
    let manifest = f.manifest();
    let trigger = manifest
        .controls
        .iter()
        .find(|c| c.id == "trigger")
        .unwrap();
    assert_eq!(trigger.at, [20.0, 60.0]);
    assert_eq!(h.state().selected.as_deref(), Some("trigger"));
}
