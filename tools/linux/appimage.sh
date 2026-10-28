#!/bin/sh
# Builds the Linux release file JoystickGobloon-<version>-x86_64.AppImage
# from a built program. Used by .github/workflows/release.yml; locally in Podman:
#   podman run --rm -v "${PWD}:/src:ro" -v joystick-gobloon-target:/target ... joystick-gobloon-linux
#     sh /src/tools/linux/appimage.sh <version> /target/appimage /target/release/JoystickGobloon
#
# The AppImage carries the program, its extra languages and the desktop entry; GTK, the
# AppIndicator and the X11/Wayland libraries come from the system (every desktop has them),
# except BUNDLED: the window library loads them at run time, and systems with GTK may not
# have them (the AppImage catalog's test machine does not).
# appimagetool and the AppImage runtime are pinned by SHA-256 like the Windows drivers.
set -eu

VERSION=$1
OUT=$2
BIN=${3:-target/release/JoystickGobloon}
SRC=$(cd "$(dirname "$0")/../.." && pwd)

TOOL_URL=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
TOOL_SHA=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
RUNTIME_URL=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
RUNTIME_SHA=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

# Not libxkbcommon.so.0 itself: GTK needs it, so the system's (newer) copy is always there.
BUNDLED="libxkbcommon-x11.so.0 libxcb-xkb.so.1"

TOOLS="$OUT/tools"
mkdir -p "$TOOLS"
fetch() { # url sha file
    if [ ! -f "$3" ] || ! echo "$2  $3" | sha256sum -c --status; then
        curl -sSfL -o "$3" "$1"
    fi
    echo "$2  $3" | sha256sum -c --quiet || { echo "$3: SHA-256 mismatch" >&2; exit 1; }
}
fetch "$TOOL_URL" "$TOOL_SHA" "$TOOLS/appimagetool"
fetch "$RUNTIME_URL" "$RUNTIME_SHA" "$TOOLS/runtime"
chmod +x "$TOOLS/appimagetool"

APPDIR="$OUT/JoystickGobloon.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin/lang" "$APPDIR/usr/lib" "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps"
install -m 755 "$BIN" "$APPDIR/usr/bin/JoystickGobloon"
for lib in $BUNDLED; do
    path=$(/sbin/ldconfig -p | awk -v l="$lib" '$1 == l && /x86-64/ { print $NF; exit }')
    [ -n "$path" ] || { echo "$lib not found: install it on the build system" >&2; exit 1; }
    install -m 644 "$path" "$APPDIR/usr/lib/$lib"
done
# Languages beyond the embedded ones load from <exe>/lang/.
cp "$SRC"/lang/*.ftl "$APPDIR/usr/bin/lang/"
cp "$SRC/LICENSE" "$APPDIR/usr/bin/"
cp "$SRC/assets/icon/JoystickGobloon-256.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/joystick-gobloon.png"
cp "$SRC/assets/icon/JoystickGobloon-256.png" "$APPDIR/joystick-gobloon.png"
cat > "$APPDIR/joystick-gobloon.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Joystick Gobloon
Comment=Device hiding, virtual joysticks and input mapping
Exec=JoystickGobloon
Icon=joystick-gobloon
Categories=Game;
Terminal=false
X-AppImage-Version=$VERSION
EOF
cp "$APPDIR/joystick-gobloon.desktop" "$APPDIR/usr/share/applications/"
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE=$(dirname "$(readlink -f "$0")")
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$HERE/usr/bin/JoystickGobloon" "$@"
EOF
chmod +x "$APPDIR/AppRun"

FILE="$OUT/JoystickGobloon-$VERSION-x86_64.AppImage"
# No FUSE in containers and CI: appimagetool runs from its extracted copy.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$TOOLS/appimagetool" --no-appstream \
    --runtime-file "$TOOLS/runtime" "$APPDIR" "$FILE"
echo "$FILE"
