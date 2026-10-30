//! `SystemGobloon [--socket <path>]`: runs the service (as root, started by systemd).
//! `SystemGobloon --cleanup`: undoes the hiding (the unit's `ExecStopPost`).

#[cfg(target_os = "linux")]
fn main() {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use system_gobloon::{Service, hide, journal, seat, server};

    journal::init();
    let rule = Path::new(hide::RULE_PATH);
    let mut args = std::env::args().skip(1);
    let mut socket = PathBuf::from(gob_system::SOCKET_PATH);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cleanup" => {
                if let Err(e) = hide::cleanup(rule, &hide::Udev) {
                    log::error!("cleanup: {e}");
                    std::process::exit(1);
                }
                log::info!("hidden devices visible again");
                return;
            }
            "--socket" => match args.next() {
                Some(path) => socket = path.into(),
                None => {
                    eprintln!("--socket needs a path");
                    std::process::exit(2);
                }
            },
            _ => {
                eprintln!("usage: SystemGobloon [--socket <path>] | --cleanup");
                std::process::exit(2);
            }
        }
    }
    let exit = |what: &str, e: std::io::Error| -> ! {
        log::error!("{what}: {e}");
        std::process::exit(1);
    };
    let listener = server::bind(&socket).unwrap_or_else(|e| exit(&socket.to_string_lossy(), e));
    log::info!(
        "SystemGobloon {} listening on {}",
        system_gobloon::VERSION,
        socket.display()
    );

    let mut hide = hide::Hide::load(
        Path::new(system_gobloon::STATE_DIR),
        rule,
        Box::new(hide::Udev),
    );
    // The rule lives in /run: written again after every start.
    if let Err(e) = hide.apply() {
        log::error!("hiding: {e}");
    }
    let service = Service::new(hide);
    if let Err(e) = hide::watch(service.hide.clone()) {
        log::error!("watching device nodes: {e}");
    }
    if let Err(e) = server::serve(listener, Arc::new(seat::allowed), service) {
        exit("socket", e);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("SystemGobloon runs on Linux only");
    std::process::exit(1);
}
