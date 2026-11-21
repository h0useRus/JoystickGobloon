//! The DCS World plugin (plugins/export/dcs-world) on made-up files in the game's formats: an
//! aircraft's diff files for a vJoy device and the keyboard, and the device's "Make HTML" table
//! (tests/data/dcs-world), put into a Saved Games folder in a temporary folder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gob_export::*;

const JOYSTICK: &str = include_str!("data/dcs-world/joystick.diff.lua");
const KEYBOARD: &str = include_str!("data/dcs-world/keyboard.diff.lua");
const HTML: &str = include_str!("data/dcs-world/joystick.html");
const GUID: &str = "{D04D97A0-E9AF-11F0-8001-444553540000}";
const GUID2: &str = "{DA66FD80-E955-11F0-8002-444553540000}";
const AIRCRAFT: &str = "Made-up aircraft";

fn plugin() -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/export/dcs-world");
    Plugin::load(&dir).expect("the plugin loads")
}

/// A user folder with the game's files; removed when dropped.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("gob-dcs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let saved = dir.join("Saved Games/DCS");
        let input = saved.join("Config/Input").join(AIRCRAFT);
        for d in ["joystick", "keyboard"] {
            std::fs::create_dir_all(input.join(d)).unwrap();
        }
        std::fs::create_dir_all(dir.join("Saved Games/DCS.openbeta/Config/Input/UiLayer")).unwrap();
        std::fs::create_dir_all(dir.join("AppData/Local")).unwrap();
        std::fs::create_dir_all(saved.join("Logs")).unwrap();
        std::fs::write(
            saved.join("Logs/dcs.log"),
            "2026-11-21 10:00:00.000 INFO    EDCORE (Main): DCS/2.9.11.4686 (x86_64; MT; Windows NT 10.0.26100)\n",
        )
        .unwrap();
        std::fs::write(
            input.join(format!("joystick/vJoy Device {GUID}.diff.lua")),
            JOYSTICK,
        )
        .unwrap();
        std::fs::write(input.join("keyboard/Keyboard.diff.lua"), KEYBOARD).unwrap();
        let html = saved.join("InputLayoutsTxt").join(AIRCRAFT);
        std::fs::create_dir_all(&html).unwrap();
        std::fs::write(html.join(format!("vJoy Device {GUID}.html")), HTML).unwrap();
        Self(dir)
    }

    fn env(&self) -> Env {
        let base = self.0.to_string_lossy().replace('\\', "/");
        Env {
            platform: "windows".into(),
            roots: vec!["C:/".into()],
            documents: format!("{base}/Documents/"),
            local_app_data: format!("{base}/AppData/Local/"),
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn target(p: &Plugin, home: &Home) -> Target {
    p.targets(&home.env())
        .unwrap()
        .into_iter()
        .find(|t| t.id == format!("DCS/{AIRCRAFT}"))
        .expect("the aircraft's target")
}

fn ours(instances: &[&str]) -> Vec<OurDevice> {
    instances
        .iter()
        .enumerate()
        .map(|(i, instance)| OurDevice {
            output: format!("vjoy:{}", i + 1),
            name: "vJoy Device".into(),
            guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
            instance: instance.to_string(),
        })
        .collect()
}

fn bind(action: &str, output: &str) -> Bind {
    Bind {
        action: format!("{AIRCRAFT}/{action}"),
        output: output.into(),
        invert: false,
        default: false,
    }
}

fn read(p: &Plugin, t: &Target, devices: &[OurDevice]) -> ReadResult {
    p.read(&ReadContext {
        target: t,
        files: &host::read_files(t),
        catalog: &Catalog::default(),
        ours: devices,
        devices: &[],
    })
    .unwrap()
}

fn write(
    p: &Plugin,
    t: &Target,
    devices: &[OurDevice],
    binds: &[Bind],
    cleared: &[String],
) -> Result<WriteResult, PluginError> {
    p.write(&WriteContext {
        target: t,
        files: &host::read_files(t),
        catalog: &Catalog::default(),
        ours: devices,
        devices: &[],
        binds,
        cleared,
        profile: "p",
        options: &BTreeMap::new(),
    })
}

/// The read bindings, the way the profile keeps them (defaults included).
fn as_profile(read: &ReadResult) -> Vec<Bind> {
    read.binds
        .iter()
        .map(|b| Bind {
            default: false,
            ..b.clone()
        })
        .collect()
}

fn sorted(mut binds: Vec<Bind>) -> Vec<(String, String, bool)> {
    let mut out: Vec<_> = binds
        .drain(..)
        .map(|b| (b.action, b.output, b.invert))
        .collect();
    out.sort();
    out
}

#[test]
fn finds_every_aircraft_and_the_version() {
    let p = plugin();
    let home = Home::new("targets");
    let targets = p.targets(&home.env()).unwrap();
    let mut ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids,
        ["DCS.openbeta/UiLayer", &format!("DCS/{AIRCRAFT}") as &str]
    );
    let t = target(&p, &home);
    assert_eq!(t.name, AIRCRAFT);
    assert!(host::installed(&t));
    assert!(t.files.keys().any(|k| k.starts_with("joystick:")));
    assert!(t.files.keys().any(|k| k.starts_with("html:")));
    let version = p.version(&t, &host::read_files(&t)).unwrap();
    assert_eq!(version.as_deref(), Some("2.9.11.4686"));
}

#[test]
fn reads_the_actions_and_bindings() {
    let p = plugin();
    let home = Home::new("read");
    let t = target(&p, &home);
    let r = read(&p, &t, &ours(&[GUID]));
    assert_eq!(r.devices.len(), 1);
    assert_eq!(r.devices[0].game_device, GUID);
    // Every action of the HTML table, with its category and kind.
    assert_eq!(r.actions.len(), 6);
    let rudder = r
        .actions
        .iter()
        .find(|a| a.id == format!("{AIRCRAFT}/a2003cdnil"))
        .unwrap();
    assert_eq!(rudder.name, "Rudder & nose wheel");
    assert_eq!(rudder.category, "Flight Control");
    assert_eq!(rudder.kind, ActionKind::Axis);
    let a = |id: &str| format!("{AIRCRAFT}/{id}");
    let find = |action: &str, output: &str| {
        r.binds
            .iter()
            .find(|b| b.action == a(action) && b.output == output)
            .cloned()
    };
    // Added in the diff, inverted by its filter.
    let pitch = find("a2001cdnil", "vjoy:1:axis:y").unwrap();
    assert!(pitch.invert && !pitch.default);
    // The game's defaults that stay.
    assert!(find("a2002cdnil", "vjoy:1:axis:x").unwrap().default);
    assert!(
        find("d350pnilunilcdnilvdnilvpnilvunil", "vjoy:1:button:2")
            .unwrap()
            .default
    );
    // Removed in the diff.
    assert!(
        !r.binds
            .iter()
            .any(|b| b.action == a("d3001pnilu3001cd12vd1vpnilvu0"))
    );
    assert!(
        !find("d68pnilunilcdnilvdnilvpnilvunil", "vjoy:1:button:5")
            .unwrap()
            .default
    );
    // The keyboard: LCtrl + G.
    assert!(find("d68pnilunilcdnilvdnilvpnilvunil", "key:0x1d+key:0x22").is_some());
    assert_eq!(r.binds.len(), 5);
}

#[test]
fn writing_what_was_read_changes_nothing() {
    let p = plugin();
    let home = Home::new("same");
    let t = target(&p, &home);
    let devices = ours(&[GUID]);
    let binds = as_profile(&read(&p, &t, &devices));
    let w = write(&p, &t, &devices, &binds, &[]).unwrap();
    assert!(w.changes.is_empty(), "{:?}", w.changes);
    assert!(w.files.is_empty() && w.create.is_empty());
}

#[test]
fn writes_changes_and_reads_them_back() {
    let p = plugin();
    let home = Home::new("write");
    let t = target(&p, &home);
    let devices = ours(&[GUID]);
    let binds = vec![
        // Pitch stays inverted on Y.
        Bind {
            invert: true,
            ..bind("a2001cdnil", "vjoy:1:axis:y")
        },
        // Roll moves from the default X to Rz.
        bind("a2002cdnil", "vjoy:1:axis:rz"),
        // The gear moves from button 5 to 7 and keeps its keyboard combo.
        bind("d68pnilunilcdnilvdnilvpnilvunil", "vjoy:1:button:7"),
        bind("d68pnilunilcdnilvdnilvpnilvunil", "key:0x1d+key:0x22"),
        // Weapon release back on its default button 1; gun fire keeps button 2.
        bind("d3001pnilu3001cd12vd1vpnilvu0", "vjoy:1:button:1"),
        bind("d350pnilunilcdnilvdnilvpnilvunil", "vjoy:1:button:2"),
        // Rudder gets the slider.
        bind("a2003cdnil", "vjoy:1:axis:slider"),
    ];
    let w = write(&p, &t, &devices, &binds, &[]).unwrap();
    assert_eq!(w.changes.len(), 4, "{:?}", w.changes);
    let key = t
        .files
        .keys()
        .find(|k| k.starts_with("joystick:"))
        .unwrap()
        .clone();
    let text = &w.files[&key];
    assert!(!w.files.contains_key("keyboard"));
    // Pitch untouched, curve and all.
    assert!(text.contains("[\"deadzone\"] = 0.02,"));
    // Weapon release gets its default button back as an added one, and the gear's old button 5
    // is removed: the "Make HTML" table, older now, still shows the old bindings.
    assert!(text.contains(
        "[\"d3001pnilu3001cd12vd1vpnilvu0\"] = {\n\t\t\t[\"added\"] = {\n\t\t\t\t[1] = {\n\t\t\t\t\t[\"key\"] = \"JOY_BTN1\",\n\t\t\t\t},\n\t\t\t},\n\t\t\t[\"name\"] = \"Weapon release\",\n\t\t},"
    ));
    assert!(
        text.contains("[\"removed\"] = {\n\t\t\t\t[1] = {\n\t\t\t\t\t[\"key\"] = \"JOY_BTN5\",")
    );
    assert!(text.contains("\"JOY_RZ\"") && text.contains("\"JOY_SLIDER1\""));
    assert!(text.starts_with("local diff = {\n\t[\"axisDiffs\"] = {\n"));
    assert!(text.ends_with("}\nreturn diff\n"));

    host::apply(&t, &host::read_files(&t), &w).unwrap();
    let back = read(&p, &t, &devices);
    assert_eq!(sorted(as_profile(&back)), sorted(binds.clone()));
    // And a second export finds nothing to do.
    let again = write(&p, &t, &devices, &binds, &[]).unwrap();
    assert!(again.changes.is_empty(), "{:?}", again.changes);
}

#[test]
fn a_device_without_a_file_gets_one() {
    let p = plugin();
    let home = Home::new("new");
    let t = target(&p, &home);
    let devices = ours(&[GUID, GUID2]);
    let binds = vec![bind("d350pnilunilcdnilvdnilvpnilvunil", "vjoy:2:button:3")];
    let mut all = as_profile(&read(&p, &t, &devices));
    all.extend(binds);
    let w = write(&p, &t, &devices, &all, &[]).unwrap();
    assert!(w.files.is_empty());
    assert_eq!(w.create.len(), 1);
    assert_eq!(w.create[0].dir, "joystick");
    assert_eq!(w.create[0].name, format!("vJoy Device {GUID2}.diff.lua"));
    assert!(w.create[0].text.contains("[\"key\"] = \"JOY_BTN3\","));
    assert!(w.create[0].text.contains("[\"name\"] = \"Gun fire\","));
}

#[test]
fn keyboard_bindings_follow_the_profile() {
    let p = plugin();
    let home = Home::new("keys");
    let t = target(&p, &home);
    let devices = ours(&[GUID]);
    let mut binds: Vec<Bind> = as_profile(&read(&p, &t, &devices))
        .into_iter()
        .filter(|b| b.output.starts_with("vjoy:"))
        .collect();
    // Gear on RShift + G instead of LCtrl + G; gun fire gets Space.
    binds.push(bind("d68pnilunilcdnilvdnilvpnilvunil", "key:0x36+key:0x22"));
    binds.push(bind("d350pnilunilcdnilvdnilvpnilvunil", "key:0x39"));
    let w = write(&p, &t, &devices, &binds, &[]).unwrap();
    let text = &w.files["keyboard"];
    assert!(text.contains("\"RShift\"") && !text.contains("\"LCtrl\""));
    assert!(text.contains("[\"key\"] = \"Space\","));
    assert_eq!(w.changes.len(), 2, "{:?}", w.changes);

    // An action cleared in the profile loses its keys.
    let joystick: Vec<Bind> = binds
        .iter()
        .filter(|b| b.output.starts_with("vjoy:"))
        .cloned()
        .collect();
    let cleared = vec![format!("{AIRCRAFT}/d68pnilunilcdnilvdnilvpnilvunil")];
    let w = write(&p, &t, &devices, &joystick, &cleared).unwrap();
    assert!(!w.files["keyboard"].contains("d68pnilunilcdnilvdnilvpnilvunil"));
}

#[test]
fn unnamed_devices_are_an_error() {
    let p = plugin();
    let home = Home::new("unlinked");
    let t = target(&p, &home);
    // Two devices with the same name and no instance GUIDs: the game's files cannot tell them.
    let devices = ours(&["", ""]);
    let binds = vec![bind("d350pnilunilcdnilvdnilvpnilvunil", "vjoy:2:button:3")];
    assert!(write(&p, &t, &devices, &binds, &[]).is_err());
}
