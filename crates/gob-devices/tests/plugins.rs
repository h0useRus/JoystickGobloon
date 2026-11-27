use std::path::{Path, PathBuf};

use gob_devices::{Catalog, Direction, Input, Inputs, Level, check, taught_path};

fn sample_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/plugins")
}

#[test]
fn loads_and_finds() {
    let catalog = Catalog::load(&[sample_dir()]);
    assert!(catalog.errors.is_empty(), "{:?}", catalog.errors);
    let plugin = catalog.find(0x0001, 0x0099, "The Sample Stick v2").unwrap();
    assert_eq!(plugin.id(), "sample-stick");
    assert_eq!(
        plugin.manifest.credit.as_deref(),
        Some("Drawn for the tests")
    );
    assert!(catalog.find(0x0001, 0x0099, "Other").is_none());
    assert!(catalog.find(0x0001, 0x0002, "Other").is_some());
    assert_eq!(plugin.input_of("hat", "push"), Some(Input::Button(3)));
    let (control, part) = plugin
        .control_of(Input::HatDirection(1, Direction::Left))
        .unwrap();
    assert_eq!((control.id.as_str(), part), ("hat", "left"));
    let (control, part) = plugin.control_of(Input::Axis(2)).unwrap();
    assert_eq!((control.id.as_str(), part), ("stick", "y"));
    assert!(check(plugin).is_empty(), "{:?}", check(plugin));
}

#[test]
fn taught_numbering_wins() {
    let dir = std::env::temp_dir().join(format!("gob-devices-taught-{}", std::process::id()));
    let mut taught = Inputs::default();
    taught.set("trigger", "first", Input::Button(7));
    taught.save(&taught_path(&dir, "sample-stick")).unwrap();
    let mut catalog = Catalog::load(&[sample_dir()]);
    assert!(catalog.load_taught(&dir).is_empty());
    let plugin = catalog.get("sample-stick").unwrap();
    assert_eq!(plugin.input_of("trigger", "first"), Some(Input::Button(7)));
    // A taught control replaces the whole control: its other parts are taught again too.
    assert_eq!(plugin.input_of("trigger", "second"), None);
    assert_eq!(plugin.input_of("hat", "push"), Some(Input::Button(3)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn finds_mistakes() {
    let mut plugin = Catalog::load(&[sample_dir()]).plugins.remove(0);
    plugin.inputs.set("trigger", "second", Input::Button(3));
    plugin.inputs.set("stick", "x", Input::Button(9));
    plugin.manifest.controls[1].at = [500.0, 10.0];
    let errors: Vec<String> = check(&plugin)
        .into_iter()
        .filter(|i| i.level == Level::Error)
        .map(|i| i.message)
        .collect();
    assert!(
        errors.iter().any(|e| e.contains("used by both")),
        "{errors:?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("needs an axis")),
        "{errors:?}"
    );
    assert!(errors.iter().any(|e| e.contains("outside")), "{errors:?}");
}

/// The plugins the program ships load and have no errors.
#[test]
fn shipped_plugins() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices");
    let catalog = Catalog::load(&[dir]);
    assert!(catalog.errors.is_empty(), "{:?}", catalog.errors);
    assert!(!catalog.plugins.is_empty());
    for plugin in &catalog.plugins {
        let errors: Vec<_> = check(plugin)
            .into_iter()
            .filter(|i| i.level == Level::Error)
            .collect();
        assert!(errors.is_empty(), "{}: {errors:?}", plugin.id());
    }
}

/// Product names VKB devices report find their own plugin, not a shorter look-alike.
#[test]
fn vkb_names_find_their_plugin() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices");
    let catalog = Catalog::load(&[dir]);
    for (name, id) in [
        (" VKBsim Gladiator EVO R  ", "vkb-gladiator-nxt-evo-r"),
        (" VKBsim Gladiator EVO L  ", "vkb-gladiator-nxt-evo-l"),
        (" VKBsim Gladiator EVO OT R  ", "vkb-gladiator-nxt-evo-ot-r"),
        (" VKBsim Gladiator EVO F14  ", "vkb-gladiator-nxt-evo-f14"),
        (
            " VKBSim Gunfighter MCG Ultimate ",
            "vkb-gunfighter-mk4-mcg-ultimate",
        ),
        (
            "VKBsim Gunfighter Modern Combat PRO",
            "vkb-gunfighter-mk4-mcg-pro",
        ),
        ("VKBSim Space Gunfighter", "vkb-gunfighter-mk4-scg-r"),
        ("VKBSim Space Gunfighter L", "vkb-gunfighter-mk4-scg-l"),
        ("S-TECS MODERN THROTTLE MINI", "vkb-stecs-modern-mini"),
        (
            "S-TECS MODERN THROTTLE MINI PLUS  ",
            "vkb-stecs-modern-mini-plus",
        ),
        (
            "S-TECS MODERN THROTTLE STANDARD STEM ",
            "vkb-stecs-modern-standard",
        ),
        ("S-TECS MODERN THROTTLE MAX STEM ", "vkb-stecs-modern-max"),
        (
            "S-TECS SPACE-L THROTTLE STANDARD  STEM ",
            "vkb-stecs-space-standard-l",
        ),
        (
            "S-TECS SPACE-R THROTTLE MINI PLUS",
            "vkb-stecs-space-mini-plus-r",
        ),
        (" VKBSim T-Rudder  ", "vkb-t-rudder-mk5"),
    ] {
        let found = catalog.find(0x231D, 0x0001, name).map(|p| p.id());
        assert_eq!(found, Some(id), "{name}");
    }
}

/// `plugins/devices/index.gob` lists every shipped plugin with its files as they are now.
#[test]
fn index_is_current() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices");
    let built = gob_devices::Index::build(&dir).unwrap();
    let text = std::fs::read_to_string(dir.join(gob_devices::index::INDEX)).unwrap();
    let saved = gob_devices::Index::parse(&text).unwrap();
    assert!(
        built == saved,
        "plugins/devices/index.gob is out of date: run `cargo run -p gob-devices --example device_index`"
    );
    let found = saved.find(0x231D, 0x0001, " VKBsim Gladiator EVO OT L ");
    assert_eq!(
        found.map(|e| e.id.as_str()),
        Some("vkb-gladiator-nxt-evo-ot-l")
    );
}

/// Product names VIRPIL devices report find their own plugin.
#[test]
fn virpil_names_find_their_plugin() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices");
    let catalog = Catalog::load(&[dir]);
    for (name, id) in [
        ("R-VPC Stick CDT-AEROMAX", "virpil-cdt-aeromax-r"),
        ("L-VPC Stick CDT-AEROMAX", "virpil-cdt-aeromax-l"),
        ("VPC Throttle CDT-VMAX", "virpil-cdt-vmax-throttle"),
        ("VPC Throttle VMAX Prime", "virpil-vmax-prime-throttle"),
        ("VPC Throttle MT-50CM4", "virpil-mongoost-50cm4-throttle"),
        ("VPC Control Panel #2", "virpil-control-panel-2"),
        ("VPC Rudder R1-FALCON", "virpil-r1-falcon"),
    ] {
        let found = catalog.find(0x3344, 0x0001, name).map(|p| p.id());
        assert_eq!(found, Some(id), "{name}");
    }
}
