//! Runs an export plugin against the real game files, only reading:
//!
//!     cargo run -p gob-export --example export_check -- <plugin folder> <root>... [--vjoy N] [--roundtrip [--dump <folder>]]
//!
//! Lists the installed targets with their game version and catalog, reads the bindings already
//! there (our devices: N vJoy devices, default 2) and with `--roundtrip` writes them back in
//! memory to show what a write would change; `--dump` saves those files into a folder of your
//! choice. Nothing is written to the game.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gob_export::{Bind, DeviceLink, Env, OurDevice, Plugin, ReadContext, WriteContext, host};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let roundtrip = take_flag(&mut args, "--roundtrip");
    let dump = take_value(&mut args, "--dump").map(PathBuf::from);
    let vjoy: u32 = take_value(&mut args, "--vjoy").map_or(2, |v| v.parse().expect("--vjoy N"));
    let Some(dir) = args.first().cloned() else {
        eprintln!(
            "usage: export_check <plugin folder> <root>... [--vjoy N] [--roundtrip [--dump <folder>]]"
        );
        std::process::exit(2);
    };
    let plugin = Plugin::load(&PathBuf::from(&dir)).unwrap_or_else(|e| fail(&e.to_string()));
    println!(
        "{} {} (games {:?}..{:?}), catalogs: {:?}",
        plugin.manifest.name,
        plugin.manifest.version,
        plugin.manifest.game.min,
        plugin.manifest.game.max,
        plugin.catalogs.versions().collect::<Vec<_>>()
    );
    let ours: Vec<OurDevice> = (1..=vjoy)
        .map(|n| OurDevice {
            output: format!("vjoy:{n}"),
            name: "vJoy Device".into(),
            guid: "{BEAD1234-0000-0000-0000-504944564944}".into(),
            instance: String::new(),
        })
        .collect();
    let env = Env {
        platform: if cfg!(windows) { "windows" } else { "linux" }.into(),
        roots: args[1..].to_vec(),
        documents: String::new(),
    };
    let targets = plugin
        .targets(&env)
        .unwrap_or_else(|e| fail(&e.to_string()));
    let installed: Vec<_> = targets.into_iter().filter(host::installed).collect();
    println!("{} installed target(s)", installed.len());
    for target in installed {
        let files = host::read_files(&target);
        let version = plugin
            .version(&target, &files)
            .unwrap_or_else(|e| fail(&e.to_string()));
        let Some((catalog, fit)) = plugin.catalogs.pick(version.as_deref()) else {
            println!("- {} ({}): no catalog", target.name, target.id);
            continue;
        };
        println!(
            "- {} at {}: version {:?}, catalog {} ({fit:?}, {} actions), supported: {}",
            target.name,
            target.id,
            version,
            catalog.version,
            catalog.actions.len(),
            version
                .as_deref()
                .is_some_and(|v| plugin.manifest.game.contains(v))
        );
        let ctx = ReadContext {
            target: &target,
            files: &files,
            catalog,
            ours: &ours,
            devices: &[],
        };
        let read = plugin.read(&ctx).unwrap_or_else(|e| fail(&e.to_string()));
        println!("  devices: {:?}", read.devices);
        println!("  {} binds, for example:", read.binds.len());
        for b in read.binds.iter().take(8) {
            let name = catalog.action(&b.action).map_or("?", |a| a.name.as_str());
            println!("    {} = {} ({name})", b.output, b.action);
        }
        let unknown = read
            .binds
            .iter()
            .filter(|b| catalog.action(&b.action).is_none())
            .count();
        println!("  {unknown} bound actions not in the catalog");
        if roundtrip {
            // The game's own defaults are not the profile's to write.
            let binds: Vec<Bind> = read.binds.iter().filter(|b| !b.default).cloned().collect();
            roundtrip_write(&plugin, &ctx, &read.devices, &binds, dump.as_deref());
        }
    }
}

fn roundtrip_write(
    plugin: &Plugin,
    ctx: &ReadContext,
    devices: &[DeviceLink],
    binds: &[Bind],
    dump: Option<&std::path::Path>,
) {
    let options = plugin
        .option_values(&BTreeMap::new())
        .unwrap_or_else(|e| fail(&e.to_string()));
    let written = plugin
        .write(&WriteContext {
            target: ctx.target,
            files: ctx.files,
            catalog: ctx.catalog,
            ours: ctx.ours,
            devices,
            binds,
            cleared: &[],
            profile: "Roundtrip",
            options: &options,
        })
        .unwrap_or_else(|e| fail(&e.to_string()));
    println!(
        "  write of the same binds: {} change(s)",
        written.changes.len()
    );
    for c in written.changes.iter().take(10) {
        println!("    {:?} {} {:?} -> {:?}", c.kind, c.action, c.from, c.to);
    }
    if let Some(dir) = dump {
        let dir = dir.join(&ctx.target.name);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| fail(&e.to_string()));
        for (key, text) in &written.files {
            std::fs::write(dir.join(key), text).unwrap_or_else(|e| fail(&e.to_string()));
        }
        for new in &written.create {
            std::fs::write(dir.join(&new.name), &new.text).unwrap_or_else(|e| fail(&e.to_string()));
        }
        println!("  written to {}", dir.display());
    }
    for (key, text) in &written.files {
        let old = ctx
            .files
            .get(key)
            .and_then(|f| f.text.as_deref())
            .unwrap_or("");
        let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), text.lines().collect());
        let differ = a.iter().zip(&b).filter(|(x, y)| x != y).count() + a.len().abs_diff(b.len());
        println!(
            "  {key}: {} -> {} lines, {differ} differ{}",
            a.len(),
            b.len(),
            if old == text { " (identical)" } else { "" }
        );
    }
    for new in &written.create {
        println!(
            "  new {}/{}: {} lines",
            new.dir,
            new.name,
            new.text.lines().count()
        );
    }
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    let before = args.len();
    args.retain(|a| a != flag);
    args.len() != before
}

fn take_value(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.remove(i);
    (i < args.len()).then(|| args.remove(i))
}

fn fail(e: &str) -> ! {
    eprintln!("{e}");
    std::process::exit(1);
}
