//! Prints HidHide state (read-only): `cargo run -p gob-hidhide --example hidhide_info [-- --all]`.
fn main() {
    let hh = match gob_hidhide::HidHide::find() {
        Ok(h) => h,
        Err(e) => return eprintln!("{e}"),
    };
    println!(
        "HidHide driver {} cloaked={:?} inverse={:?}",
        hh.version,
        hh.cloaked(),
        hh.inverse()
    );
    let hidden = hh.hidden().unwrap_or_default();
    for h in &hidden {
        println!("hidden {h}");
    }
    let gaming_only = !std::env::args().any(|a| a == "--all");
    let start = std::time::Instant::now();
    let groups = hh.device_groups(gaming_only).unwrap_or_default();
    println!("{} groups in {:?}", groups.len(), start.elapsed());
    for g in &groups {
        println!(
            "{:<40} present={} gaming={} hidden={}",
            g.name(),
            g.present(),
            g.gaming(),
            g.is_hidden(&hidden)
        );
        for d in &g.devices {
            println!(
                "    {} | {} | {}",
                d.device_instance_path, d.base_container_device_instance_path, d.usage
            );
        }
    }
    for app in hh.apps().unwrap_or_default() {
        println!("app {}", app.display());
    }
}
