//! Prints HidHide state (read-only): `cargo run -p gob-hidhide --example hidhide_info [-- --all]`.
fn main() {
    let hh = match gob_hidhide::HidHide::find() {
        Ok(h) => h,
        Err(e) => return eprintln!("{e}"),
    };
    println!("HidHide {} cloaked={:?}", hh.version, hh.cloaked());
    let hidden = hh.hidden().unwrap_or_default();
    let gaming_only = !std::env::args().any(|a| a == "--all");
    for g in hh.device_groups(gaming_only).unwrap_or_default() {
        println!(
            "{:<40} present={} gaming={} hidden={}",
            g.name(),
            g.present(),
            g.gaming(),
            g.is_hidden(&hidden)
        );
        for d in &g.devices {
            println!("    {} {:?}", d.device_instance_path, d.vid_pid());
        }
    }
    for app in hh.apps().unwrap_or_default() {
        println!("app {}", app.display());
    }
}
