#!/bin/sh
# Builds RustLauncherMC-x86_64.AppImage from target/release/rustlauncher.
# Usage: packaging/linux/build-appimage.sh <out dir>
# Needs appimagetool on PATH, or APPIMAGETOOL pointing at it.
set -eu
out=${1:?usage: build-appimage.sh <out dir>}
root=$(cd "$(dirname "$0")/../.." && pwd)
tool=${APPIMAGETOOL:-appimagetool}
appdir=$(mktemp -d)
trap 'rm -rf "$appdir"' EXIT

install -Dm755 "$root/target/release/rustlauncher" "$appdir/usr/bin/rustlauncher"
install -Dm644 "$root/packaging/linux/rustlaunchermc.desktop" "$appdir/rustlaunchermc.desktop"
install -Dm644 "$root/packaging/linux/rustlaunchermc.desktop" "$appdir/usr/share/applications/rustlaunchermc.desktop"
install -Dm644 "$root/assets/logo-256.png" "$appdir/rustlaunchermc.png"
install -Dm644 "$root/assets/logo-256.png" "$appdir/usr/share/icons/hicolor/256x256/apps/rustlaunchermc.png"
install -Dm644 "$root/LICENSE" "$appdir/usr/share/doc/rustlaunchermc/LICENSE"
install -Dm644 "$root/THIRD_PARTY_NOTICES.md" "$appdir/usr/share/doc/rustlaunchermc/THIRD_PARTY_NOTICES.md"
ln -s usr/bin/rustlauncher "$appdir/AppRun"

mkdir -p "$out"
# The runtime is embedded, so building needs no FUSE.
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$out/RustLauncherMC-x86_64.AppImage"
