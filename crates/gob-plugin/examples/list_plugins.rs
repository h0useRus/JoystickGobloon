//! Loads the plugin DLLs of a folder and prints what they report.
//! `cargo run -p gob-plugin --example list_plugins -- target\debug\examples`

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "plugins".into());
    let mut registry = gob_plugin::Registry::default();
    for e in registry.load_dir(std::path::Path::new(&dir)) {
        eprintln!("skipped {e}");
    }
    for p in registry.loaded() {
        println!("{} {} ({})", p.name, p.version, p.path.display());
    }
}
