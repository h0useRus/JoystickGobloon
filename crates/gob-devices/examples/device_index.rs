//! Writes `plugins/devices/index.gob`, the list of device plugins the program downloads, from the
//! plugins' folders. Run after adding or changing a plugin: `cargo run -p gob-devices --example
//! device_index [<plugins root>]`.

use std::path::PathBuf;

use gob_devices::Index;
use gob_devices::index::INDEX;

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices"));
    let index = match Index::build(&root) {
        Ok(index) => index,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let path = root.join(INDEX);
    if let Err(e) = std::fs::write(&path, index.to_text()) {
        eprintln!("{}: {e}", path.display());
        std::process::exit(1);
    }
    println!("{}: {} plugins", path.display(), index.devices.len());
}
