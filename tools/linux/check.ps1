# Runs a cargo command for Linux in Podman (docs/LINUX_PLAN.md), default: clippy and tests.
#   tools\linux\check.ps1                      clippy -D warnings + tests
#   tools\linux\check.ps1 cargo check --workspace
# The sources are mounted read-only; build output and the cargo registry live in Podman volumes, so
# later runs are incremental. Needs a started Podman machine (podman machine start).
$Command = $args
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$image = "joystick-gobloon-linux"

podman image exists $image
if ($LASTEXITCODE -ne 0) {
    podman build -t $image -f "$PSScriptRoot\Containerfile" $PSScriptRoot
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (-not $Command) {
    $Command = @("sh", "-c", "cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace")
}
podman run --rm `
    -v "${root}:/src:ro" `
    -v joystick-gobloon-target:/target `
    -v joystick-gobloon-cargo:/usr/local/cargo/registry `
    -v joystick-gobloon-rustup:/usr/local/rustup `
    $image @Command
exit $LASTEXITCODE
