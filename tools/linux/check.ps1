# Runs a cargo command for Linux in Podman, default: clippy and tests.
#   tools\linux\check.ps1                      clippy -D warnings + tests
#   tools\linux\check.ps1 cargo check --workspace
#   tools\linux\check.ps1 --devices cargo test ...   privileged, with the machine's /dev: uinput and
#                                                    the event nodes it creates (tests marked ignored)
# The sources are mounted read-only; build output and the cargo registry live in Podman volumes, so
# later runs are incremental. Needs a started Podman machine (podman machine start).
$Command = $args
$devices = @()
if ($Command.Count -gt 0 -and $Command[0] -eq "--devices") {
    $devices = @("--privileged", "-v", "/dev:/dev")
    $Command = $Command[1..($Command.Count - 1)]
}
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
podman run --rm @devices `
    -v "${root}:/src:ro" `
    -v joystick-gobloon-target:/target `
    -v joystick-gobloon-cargo:/usr/local/cargo/registry `
    -v joystick-gobloon-rustup:/usr/local/rustup `
    $image @Command
exit $LASTEXITCODE
