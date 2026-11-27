use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

/// Serves `root`'s files over HTTP on a local port; `tamper` changes every byte served.
fn serve(root: PathBuf, tamper: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                    break;
                }
            }
            let path = line
                .split(' ')
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/');
            let mut stream = reader.into_inner();
            match std::fs::read(root.join(path)) {
                Ok(mut body) => {
                    if tamper {
                        body.iter_mut().for_each(|b| *b ^= 1);
                    }
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(&body);
                }
                Err(_) => {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            }
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/devices")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "JoystickGobloon-download-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn entry(id: &str) -> Entry {
    let text = std::fs::read_to_string(repo().join(INDEX)).unwrap();
    let index = Index::parse(&text).unwrap();
    index.devices.into_iter().find(|e| e.id == id).unwrap()
}

#[test]
fn downloads_a_plugin_and_keeps_another_in_its_folder() {
    let base = serve(repo(), false);
    let into = scratch("ok");
    let e = entry("vkb-t-rudder-mk5");
    // A different plugin of the user's in the same folder stays.
    let theirs = into.join(&e.path);
    std::fs::create_dir_all(&theirs).unwrap();
    std::fs::write(
        theirs.join(MANIFEST),
        "id = \"mine\"\nname = \"Mine\"\nbrand = \"B\"\n",
    )
    .unwrap();
    let dir = install(&base, &e, &into).unwrap();
    assert_eq!(dir, into.join(format!("{}-2", e.path)));
    assert!(holds(&theirs, "mine"));
    assert_eq!(gob_devices::DevicePlugin::load(&dir).unwrap().id(), e.id);
    // Downloaded again: the same plugin's folder is replaced.
    assert_eq!(install(&base, &e, &into).unwrap(), dir);
    let _ = std::fs::remove_dir_all(&into);
}

#[test]
fn a_changed_file_is_not_installed() {
    let base = serve(repo(), true);
    let into = scratch("tampered");
    let e = entry("vkb-t-rudder-mk5");
    assert!(install(&base, &e, &into).is_err());
    let left: Vec<_> = std::fs::read_dir(&into).unwrap().collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = std::fs::remove_dir_all(&into);
}
