// Included by the build scripts of joystick-gobloon and system-gobloon: both report the same
// version, so the program can tell whether the installed service is its own.

/// The version CI sets; else, in a git checkout, the latest release tag and how far this build
/// is from it (`0.9.0-alpha.45+3.g728daf5`); else the crate version.
fn version() -> String {
    if let Ok(v) = std::env::var("JOYSTICK_GOBLOON_VERSION") {
        return v;
    }
    git_version().unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default())
}

fn git(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_version() -> Option<String> {
    // `v0.9.0-alpha.45-3-g728daf5`: tag, commits since it, commit.
    let described = git(&["describe", "--tags", "--long", "--match", "v[0-9]*"])?;
    let mut parts = described.rsplitn(3, '-');
    let (hash, count, tag) = (parts.next()?, parts.next()?, parts.next()?);
    // Built again when HEAD moves (commit, checkout) or a tag arrives.
    // A path that does not exist would run this script on every build.
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        let head = git(&["symbolic-ref", "-q", "HEAD"]);
        let paths = ["HEAD", "packed-refs", "refs/tags"]
            .into_iter()
            .chain(head.as_deref());
        for path in paths.map(|p| format!("{dir}/{p}")) {
            if std::path::Path::new(&path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    Some(format!("{}+{count}.{hash}", tag.trim_start_matches('v')))
}
