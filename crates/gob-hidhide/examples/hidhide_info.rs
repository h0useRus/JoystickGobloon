//! Prints HidHide state (read-only): `cargo run -p gob-hidhide --example hidhide_info`.
fn main() {
    let hh = match gob_hidhide::HidHide::find() {
        Ok(h) => h,
        Err(e) => return eprintln!("{e}"),
    };
    println!("HidHide {} cloaked={:?}", hh.version, hh.cloaked());
    let hidden = hh.hidden().unwrap_or_default();
    for d in hh.gaming_devices().unwrap_or_default() {
        let is_hidden = hidden
            .iter()
            .any(|h| h.eq_ignore_ascii_case(&d.device_instance_path));
        println!(
            "{:<28} present={} hidden={} {:?}",
            d.name(),
            d.present,
            is_hidden,
            d.vid_pid()
        );
    }
    for app in hh.apps().unwrap_or_default() {
        println!("app {}", app.display());
    }
}
