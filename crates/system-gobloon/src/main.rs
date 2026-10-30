//! `SystemGobloon [--socket <path>]`: runs the service (as root, started by systemd).

#[cfg(target_os = "linux")]
fn main() {
    use std::path::PathBuf;
    use std::sync::Arc;

    use system_gobloon::{journal, seat, server};

    journal::init();
    let mut args = std::env::args().skip(1);
    let mut socket = PathBuf::from(gob_system::SOCKET_PATH);
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--socket", Some(path)) => socket = path.into(),
            _ => {
                eprintln!("usage: SystemGobloon [--socket <path>]");
                std::process::exit(2);
            }
        }
    }
    let listener = match server::bind(&socket) {
        Ok(l) => l,
        Err(e) => {
            log::error!("{}: {e}", socket.display());
            std::process::exit(1);
        }
    };
    log::info!(
        "SystemGobloon {} listening on {}",
        system_gobloon::VERSION,
        socket.display()
    );
    if let Err(e) = server::serve(listener, Arc::new(seat::allowed)) {
        log::error!("{e}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("SystemGobloon runs on Linux only");
    std::process::exit(1);
}
