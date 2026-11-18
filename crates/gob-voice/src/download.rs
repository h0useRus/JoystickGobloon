//! Downloads of speech models, voices and the voice library: a .tar.bz2 from a URL, checked
//! against the SHA-256 pinned by the caller before it is unpacked, kept only when the caller's
//! check finds what it needs in it, so a broken or half-done download is never used.

use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::Error;

/// A .tar.bz2 to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    /// The archive's name without `.tar.bz2`, and the folder it is unpacked to.
    pub name: &'static str,
    /// Where it is downloaded from (the whole URL).
    pub url: String,
    /// Download size in bytes.
    pub bytes: u64,
    pub sha256: &'static str,
}

/// Downloads `archive` into `dir`, checks it and unpacks it into `dir/<name>`; `usable` says
/// whether the unpacked folder holds what is needed. Returns the folder. `progress` gets (bytes
/// so far, total); setting `cancel` stops it and leaves nothing behind. Blocks: call it from a
/// thread of its own.
pub fn install(
    dir: &Path,
    archive: &Archive,
    usable: fn(&Path) -> bool,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, Error> {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .build()
        .new_agent();
    let response = agent
        .get(&archive.url)
        .call()
        .map_err(|e| Error::Download(format!("{}: {e}", archive.url)))?;
    let total = response.body().content_length().unwrap_or(archive.bytes);
    let reader = response
        .into_body()
        .into_with_config()
        .limit(archive.bytes + archive.bytes / 2)
        .reader();
    install_from(
        Watched::new(reader, cancel),
        total,
        dir,
        archive,
        usable,
        progress,
        cancel,
    )
}

/// No data this long: the connection stalled.
const STALL: Duration = Duration::from_secs(60);

/// Reads `source` on a thread of its own, so a stalled connection ends after [`STALL`] and a
/// cancel is seen at once, however slow the line (a limit on the whole body would fail big
/// downloads on slow lines instead). A read left hanging ends with the connection.
struct Watched<'a> {
    chunks: std::sync::mpsc::Receiver<io::Result<Vec<u8>>>,
    rest: Vec<u8>,
    at: usize,
    cancel: &'a AtomicBool,
}

impl<'a> Watched<'a> {
    fn new(mut source: impl Read + Send + 'static, cancel: &'a AtomicBool) -> Self {
        let (tx, chunks) = std::sync::mpsc::sync_channel(4);
        std::thread::spawn(move || {
            loop {
                let mut buf = vec![0; 64 * 1024];
                let chunk = match source.read(&mut buf) {
                    Ok(n) => {
                        buf.truncate(n);
                        Ok(buf)
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => Err(e),
                };
                let end = !matches!(&chunk, Ok(b) if !b.is_empty());
                if tx.send(chunk).is_err() || end {
                    return;
                }
            }
        });
        Self {
            chunks,
            rest: Vec::new(),
            at: 0,
            cancel,
        }
    }
}

impl Read for Watched<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.at == self.rest.len() {
            let waited = std::time::Instant::now();
            self.rest = loop {
                if self.cancel.load(Ordering::Relaxed) {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
                }
                match self.chunks.recv_timeout(Duration::from_millis(250)) {
                    Ok(chunk) => break chunk?,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if waited.elapsed() >= STALL {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "no data for a minute: the connection stalled",
                            ));
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
                }
            };
            self.at = 0;
        }
        let n = out.len().min(self.rest.len() - self.at);
        out[..n].copy_from_slice(&self.rest[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// [`install`] from any source of the archive's bytes.
pub(crate) fn install_from(
    mut source: impl Read,
    total: u64,
    dir: &Path,
    archive: &Archive,
    usable: fn(&Path) -> bool,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, Error> {
    let io_error = |e: io::Error| Error::Download(e.to_string());
    fs::create_dir_all(dir).map_err(io_error)?;
    let part = dir.join(format!("{}.tar.bz2.part", archive.name));
    let unpacked = dir.join(format!("{}.part", archive.name));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&part)
            .map_err(io_error)?;
        let mut hash = Sha256::new();
        let mut buf = vec![0; 64 * 1024];
        let mut done = 0u64;
        progress(0, total);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let n = match source.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_error(e)),
            };
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(io_error)?;
            done += n as u64;
            progress(done, total.max(done));
        }
        let got = hex(&hash.finalize());
        if !got.eq_ignore_ascii_case(archive.sha256) {
            return Err(Error::Download(format!(
                "{}: SHA-256 {got}, expected {}",
                archive.name, archive.sha256
            )));
        }
        file.rewind().map_err(io_error)?;
        let _ = fs::remove_dir_all(&unpacked);
        unpack(file, &unpacked, cancel)?;
        // The archive holds its files in a folder of its own name; take that, or the whole of it.
        let inner = unpacked.join(archive.name);
        let root = if inner.is_dir() {
            inner
        } else {
            unpacked.clone()
        };
        if !usable(&root) {
            return Err(Error::Model(format!(
                "{}: not what was expected in the archive",
                archive.name
            )));
        }
        let path = dir.join(archive.name);
        let _ = fs::remove_dir_all(&path);
        fs::rename(&root, &path).map_err(io_error)?;
        Ok(path)
    })();
    let _ = fs::remove_file(&part);
    let _ = fs::remove_dir_all(&unpacked);
    match &result {
        Ok(path) => log::info!("installed {}", path.display()),
        Err(Error::Cancelled) => log::info!("{}: download cancelled", archive.name),
        Err(e) => log::error!("{}: {e}", archive.name),
    }
    result
}

/// Deletes `dir/<name>` (nothing when it is not there).
pub fn remove(dir: &Path, name: &str) -> io::Result<()> {
    match fs::remove_dir_all(dir.join(name)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Unpacks a .tar.bz2 into `to`, refusing entries that would land outside it and leaving out the
/// sample recordings (`test_wavs/`) model archives carry.
fn unpack(file: impl Read + Seek, to: &Path, cancel: &AtomicBool) -> Result<(), Error> {
    let bad = |e: io::Error| Error::Download(format!("archive: {e}"));
    fs::create_dir_all(to).map_err(bad)?;
    let mut archive = tar::Archive::new(bzip2::read::BzDecoder::new(io::BufReader::new(file)));
    for entry in archive.entries().map_err(bad)? {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let mut entry = entry.map_err(bad)?;
        let path = entry.path().map_err(bad)?.into_owned();
        if path.components().any(|c| c.as_os_str() == "test_wavs") {
            continue;
        }
        // unpack_in refuses (returns false for) paths that leave `to`.
        if !entry.unpack_in(to).map_err(bad)? {
            return Err(Error::Download(format!(
                "archive: unsafe path {}",
                path.display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
