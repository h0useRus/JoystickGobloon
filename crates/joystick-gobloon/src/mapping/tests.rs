//! Mapping driven like a user would (egui events, no real mouse): worked inputs get numbers
//! that are dragged onto the picture, placed tags move and come off again; on a copy of
//! gob-devices' sample plugin in a temporary folder.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Pos2, Vec2};
use egui_kittest::Harness;
use gob_devices::{Direction, Entry, Input, Inputs, Kind, Manifest};

use super::Mapper;
use super::store::Store;

const SAMPLE: &str = "sample-stick";

/// A temporary folder with the sample plugin "shipped" in `shipped/sample/sample-stick`.
struct Folders {
    root: PathBuf,
}

impl Folders {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gobloon-mapping-{name}-{}", std::process::id()));
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

fn app(folders: &Folders) -> Mapper {
    let store = Store::with(vec![folders.shipped(), folders.user()], folders.user());
    let mut app = Mapper::with(store);
    app.open(SAMPLE);
    app.editing = true;
    app
}

fn harness(app: Mapper) -> Harness<'static, Mapper> {
    let i18n = gob_i18n::I18n::new(crate::LANGUAGES, None);
    let mut h = Harness::builder()
        .with_size(Vec2::new(1280.0, 820.0))
        .with_context_setup(|ctx| crate::style::install_fonts(ctx, false))
        .build_ui_state(move |ui, app: &mut Mapper| app.show(ui, &i18n), app);
    h.run_steps(3);
    h
}

/// Presses at `from`, moves to `to` in a few steps and lets go there.
fn drag(h: &mut Harness<'_, Mapper>, from: Pos2, to: Pos2) {
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

fn spot(h: &Harness<'_, Mapper>, at: [f32; 2]) -> Pos2 {
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

/// Where `part` of group `id` opens while something is dragged near it.
fn slot(h: &Harness<'_, Mapper>, id: &str, offset: Vec2) -> Pos2 {
    let at = h.state().plugin().unwrap().control(id).unwrap().at;
    spot(h, at) + offset * 32.0
}

#[test]
fn a_button_becomes_a_hat_filled_by_dragging() {
    let f = Folders::new("hat");
    let mut app = app(&f);
    app.note(Input::Button(7));
    app.place(Input::Button(7), [80.0, 40.0], 100.0);
    app.make_hat("button_7");
    let hat = f.manifest().controls.into_iter().last().unwrap();
    assert_eq!((hat.id.as_str(), hat.kind), ("hat_2", Kind::Hat));
    // Its own button waits to go onto its direction.
    assert_eq!(app.pending, [Input::Button(7)]);
    app.note(Input::Button(8));
    // A button placed elsewhere goes in too.
    app.place(Input::Button(8), [10.0, 90.0], 100.0);
    let mut h = harness(app);
    let from = h.state().layout.pending(Input::Button(7)).unwrap();
    let to = slot(&h, "hat_2", Vec2::new(-1.0, 0.0));
    drag(&mut h, from, to);
    let from = spot(&h, [10.0, 90.0]);
    let to = slot(&h, "hat_2", Vec2::ZERO);
    drag(&mut h, from, to);
    let inputs = f.inputs();
    assert_eq!(inputs.get("hat_2", "left"), Some(Input::Button(7)));
    assert_eq!(inputs.get("hat_2", "push"), Some(Input::Button(8)));
    assert!(f.manifest().controls.iter().all(|c| c.id != "button_8"));
    assert!(h.state().pending.is_empty());
    // Split up, its buttons wait again.
    h.state_mut().split("hat_2");
    assert_eq!(h.state().pending, [Input::Button(7), Input::Button(8)]);
}

#[test]
fn an_axis_dropped_on_an_axis_makes_a_stick() {
    let f = Folders::new("stick");
    let mut app = app(&f);
    app.note(Input::Axis(4));
    app.place(Input::Axis(4), [85.0, 50.0], 100.0);
    app.note(Input::Axis(5));
    let mut h = harness(app);
    let from = h.state().layout.pending(Input::Axis(5)).unwrap();
    let to = slot(&h, "axis_rx", Vec2::new(0.0, 1.0));
    drag(&mut h, from, to);
    let manifest = f.manifest();
    let stick = manifest
        .controls
        .iter()
        .find(|c| c.id == "axis_rx")
        .unwrap();
    assert_eq!((stick.kind, stick.name.as_str()), (Kind::Stick, "Stick"));
    let inputs = f.inputs();
    assert_eq!(inputs.get("axis_rx", "x"), Some(Input::Axis(4)));
    assert_eq!(inputs.get("axis_rx", "y"), Some(Input::Axis(5)));
}

/// Buttons placed right around a hat (as the hat's directions sit on the grip) go in too.
#[test]
fn buttons_next_to_a_hat_go_into_it() {
    let f = Folders::new("close");
    let mut app = app(&f);
    for (n, at) in [(7, [80.0, 40.0]), (8, [80.0, 33.0]), (9, [87.0, 40.0])] {
        app.note(Input::Button(n));
        app.place(Input::Button(n), at, 100.0);
    }
    app.make_hat("button_7");
    let mut h = harness(app);
    // Button 8 sits just above the hat: dragged onto the up slot.
    let from = spot(&h, [80.0, 33.0]);
    let to = slot(&h, "hat_2", Vec2::new(0.0, -1.0));
    drag(&mut h, from, to);
    let from = spot(&h, [87.0, 40.0]);
    let to = slot(&h, "hat_2", Vec2::new(1.0, 0.0));
    drag(&mut h, from, to);
    let inputs = f.inputs();
    assert_eq!(inputs.get("hat_2", "up"), Some(Input::Button(8)));
    assert_eq!(inputs.get("hat_2", "right"), Some(Input::Button(9)));
    let ids: Vec<String> = f.manifest().controls.into_iter().map(|c| c.id).collect();
    assert!(!ids.contains(&"button_8".to_string()) && !ids.contains(&"button_9".to_string()));
}

/// "Make it a hat" on a button with buttons right around it (a hat's push in the middle of its
/// directions) takes them in by where they sit.
#[test]
fn a_hat_gathers_the_buttons_around_it() {
    let f = Folders::new("gather");
    let mut app = app(&f);
    let around = [
        (7, [80.0, 40.0]),
        (8, [80.0, 36.0]),
        (9, [84.0, 41.0]),
        (10, [79.0, 44.0]),
        (11, [76.0, 39.0]),
        // Too far to be one of its directions.
        (12, [80.0, 60.0]),
    ];
    for (n, at) in around {
        app.note(Input::Button(n));
        app.place(Input::Button(n), at, 100.0);
    }
    app.make_hat("button_7");
    let inputs = f.inputs();
    assert_eq!(inputs.get("hat_2", "up"), Some(Input::Button(8)));
    assert_eq!(inputs.get("hat_2", "right"), Some(Input::Button(9)));
    assert_eq!(inputs.get("hat_2", "down"), Some(Input::Button(10)));
    assert_eq!(inputs.get("hat_2", "left"), Some(Input::Button(11)));
    assert_eq!(inputs.get("hat_2", "push"), Some(Input::Button(7)));
    let ids: Vec<String> = f.manifest().controls.into_iter().map(|c| c.id).collect();
    assert!(ids.contains(&"button_12".to_string()));
    for gone in ["button_7", "button_8", "button_9", "button_10", "button_11"] {
        assert!(!ids.contains(&gone.to_string()), "{gone}");
    }
    assert!(app.pending.is_empty());
}

#[test]
fn controls_copy_to_another_device() {
    let f = Folders::new("copy");
    let mut store = Store::with(vec![f.shipped(), f.user()], f.user());
    let picture = f.shipped().join("sample/sample-stick/views/main.svg");
    let id = store
        .create("Sample", "Other hand", &picture, None)
        .unwrap();
    store.copy_controls(SAMPLE, &id, true).unwrap();
    let copy = store.get(&id).unwrap();
    let trigger = copy.control("trigger").unwrap();
    // The sample's trigger at 40, 30 on a 100 x 100 picture, mirrored.
    assert_eq!(trigger.at, [60.0, 30.0]);
    assert_eq!(trigger.side, gob_devices::Side::Right);
    assert_eq!(copy.input_of("trigger", "first"), Some(Input::Button(1)));
    assert_eq!(copy.manifest.controls.len(), 3);
    // Copying again replaces, it does not add.
    store.copy_controls(SAMPLE, &id, false).unwrap();
    let copy = store.get(&id).unwrap();
    assert_eq!(copy.manifest.controls.len(), 3);
    assert_eq!(copy.control("trigger").unwrap().at, [40.0, 30.0]);
}

#[test]
fn a_device_shared_as_a_file_comes_in_on_another_computer() {
    let f = Folders::new("share");
    let store = Store::with(vec![f.shipped(), f.user()], f.user());
    let file = f.root.join("shared.zip");
    super::share::export(&store, SAMPLE, &file).unwrap();
    // Another computer: nothing of the sample yet.
    let other = f.root.join("other");
    let mut there = Store::with(vec![other.clone()], other.clone());
    let unpacked = super::share::unpack(&there, &file).unwrap();
    assert_eq!(unpacked.id, SAMPLE);
    assert!(unpacked.replaces.is_none());
    unpacked.install(&mut there).unwrap();
    let plugin = there.get(SAMPLE).unwrap();
    assert_eq!(plugin.dir, other.join("sample/sample-stick"));
    assert_eq!(plugin.manifest.controls.len(), 3);
    assert_eq!(plugin.input_of("trigger", "first"), Some(Input::Button(1)));
    // Taken in again: it replaces that copy, and says so first.
    let again = super::share::unpack(&there, &file).unwrap();
    assert_eq!(again.replaces.as_deref(), Some(plugin.dir.as_path()));
    again.install(&mut there).unwrap();
    assert!(there.get(SAMPLE).is_some());
    assert!(
        !other
            .join(format!(".import-{}", std::process::id()))
            .exists()
    );
}

#[test]
fn a_file_without_a_device_is_refused() {
    let f = Folders::new("refuse");
    let store = Store::with(vec![f.user()], f.user());
    let file = f.root.join("other.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&file).unwrap());
    zip.start_file("../evil.gob", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut zip, b"x").unwrap();
    zip.finish().unwrap();
    assert!(super::share::unpack(&store, &file).is_err());
    assert!(!f.root.join("evil.gob").exists());
}
