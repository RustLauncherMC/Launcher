#!/bin/sh
# Builds rustlaunchermc_<version>_amd64.deb from target/release/rustlauncher.
# Usage: packaging/linux/build-deb.sh <version> <out dir>
set -eu
version=${1:?usage: build-deb.sh <version> <out dir>}
out=${2:?usage: build-deb.sh <version> <out dir>}
root=$(cd "$(dirname "$0")/../.." && pwd)
pkg=$(mktemp -d)
trap 'rm -rf "$pkg"' EXIT

install -Dm755 "$root/target/release/rustlauncher" "$pkg/usr/bin/rustlauncher"
install -Dm644 "$root/packaging/linux/rustlaunchermc.desktop" "$pkg/usr/share/applications/rustlaunchermc.desktop"
install -Dm644 "$root/assets/logo-256.png" "$pkg/usr/share/icons/hicolor/256x256/apps/rustlaunchermc.png"
install -Dm644 "$root/LICENSE" "$pkg/usr/share/doc/rustlaunchermc/copyright"
install -Dm644 "$root/THIRD_PARTY_NOTICES.md" "$pkg/usr/share/doc/rustlaunchermc/THIRD_PARTY_NOTICES.md"

mkdir -p "$pkg/DEBIAN"
cat > "$pkg/DEBIAN/control" <<CONTROL
Package: rustlaunchermc
Version: $version
Section: games
Priority: optional
Architecture: amd64
Depends: libc6 (>= 2.35), libgcc-s1, libxcb1, libxcb-xkb1, libxkbcommon0, libxkbcommon-x11-0
Recommends: libvulkan1, libwayland-client0, libfontconfig1, gnome-keyring | kwalletmanager
Maintainer: RustLauncherMC <https://github.com/RustLauncherMC/Launcher>
Homepage: https://github.com/RustLauncherMC/Launcher
Description: Fan-made launcher for Minecraft rewritten in Rust
 Installs, updates and starts Horizon (Minecraft: Java Edition rewritten in
 Rust) and Cinnabar (a Minecraft: Bedrock Edition client in Rust), with a
 layout like the official launcher. Not affiliated with Mojang or Microsoft.
CONTROL

mkdir -p "$out"
dpkg-deb --build --root-owner-group "$pkg" "$out/rustlaunchermc_${version}_amd64.deb"
