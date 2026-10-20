#!/bin/sh
# GUI smoke test for the Linux build: starts it under Xvfb (no desktop, no tray host), checks that
# a second start wakes the first, saves a screenshot to /out/linux-smoke.png and prints the log.
# Runs in the build container with an /out folder (docs/LINUX_PLAN.md, "Тестирование"):
#   tools\linux\check.ps1 is not used here, as the script needs /out:
#   podman run --rm -v "${PWD}:/src:ro" -v joystick-gobloon-target:/target
#     -v joystick-gobloon-cargo:/usr/local/cargo/registry -v joystick-gobloon-rustup:/usr/local/rustup
#     -v "<folder>:/out" joystick-gobloon-linux sh /src/tools/linux/gui_smoke.sh [start page]
set -e
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends xvfb x11-apps imagemagick libgl1-mesa-dri libegl1 libxkbcommon-x11-0 libxcursor1 libxi6 libxrandr2 >/dev/null 2>&1
# APP: the program to start (e.g. an AppImage); by default a fresh debug build.
if [ -z "${APP:-}" ]; then
    cargo build -q -p joystick-gobloon
    APP=/target/debug/JoystickGobloon
fi
# No FUSE in containers: an AppImage runs from its extracted copy.
export APPIMAGE_EXTRACT_AND_RUN=1
export HOME=/tmp/smokehome
rm -rf "$HOME" && mkdir -p "$HOME"
Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &
sleep 2
export DISPLAY=:99
export JOYSTICK_GOBLOON_START_PAGE="${1:-about}"
"$APP" &
PID=$!
sleep 10
# A second start must wake the first and exit at once.
timeout 5 "$APP" && echo "second start exited" || echo "second start: exit $?"
xwd -root -silent | convert xwd:- png:/out/linux-smoke.png
kill $PID 2>/dev/null || true
sleep 1
echo "--- data dir:"
ls -la "$HOME/.config/JoystickGobloon" 2>&1
echo "--- log:"
cat "$HOME/.config/JoystickGobloon/JoystickGobloon.log" 2>&1 | tail -40
