#!/usr/bin/env bash
# Builds the Linux release into target/dist/ (up to 0.2.4, each name ended in "-experimental"):
#   Swoopdl-<version>-linux-x86_64.AppImage        runs on most distros
#   Swoopdl-<version>-linux-x86_64.tar.gz          the plain programs
#   Swoopdl-<version>-linux-x86_64.deb             Debian, Ubuntu, Mint
#   Swoopdl-<version>-linux-x86_64.rpm             Fedora, RHEL, openSUSE
# Update now (crates/tools/src/update.rs), the website (tools/website/build.py) and the docs
# look for these names, and still for the "-experimental" ones of older releases.
# Needs: cargo, PowerShell, Node.js, python3 with Pillow (the icon), dpkg-deb, rpmbuild, network access (appimagetool is fetched),
# Build on the oldest Ubuntu you want to support: the programs need its glibc or newer.
#   tools/installer/build-linux.sh [--no-build]
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
dist="$root/target/dist"
bin="$root/target/release"
version="$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$root/Cargo.toml" | head -1)"
[ -n "$version" ] || { echo "no version in Cargo.toml" >&2; exit 1; }
name="Swoopdl-$version-linux-x86_64"
programs=(swoopdl swoopdl-nmhost swoopdl-cli swoopdl-mcp)

# The extension is compiled from the same checkout and shipped without an install-time download.
pwsh -NoLogo -NoProfile -File "$root/tools/extension/pack.ps1"
node "$root/tools/extension/check-pack.mjs"

if [ "${1:-}" != "--no-build" ]; then
  # The build machine's paths out of the programs (#25): panic locations and debug info say
  # "cargo/..." for cargo's folder and "sdl/..." for this checkout. Encoded flags (unit separators),
  # as those paths may hold spaces.
  CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=cargo"$'\x1f'"--remap-path-prefix=$root=sdl" \
    cargo build --release -p sdl-app -p sdl-nmhost -p sdl-cli -p sdl-mcp
fi

mkdir -p "$dist"
stage="$dist/stage-linux"
rm -rf "$stage"
mkdir -p "$stage"

# One tree in the usual Linux layout; every package is made from it.
tree="$stage/tree"
mkdir -p "$tree/usr/bin" "$tree/usr/share/applications" \
  "$tree/usr/share/icons/hicolor/256x256/apps" "$tree/usr/share/swoopdl" \
  "$tree/usr/share/doc/swoopdl"
for p in "${programs[@]}"; do install -m 755 "$bin/$p" "$tree/usr/bin/$p"; done
# Crash reports: the release's symbols from the programs' debug info (the -symbols zip), then the
# programs without it or their symbol table (#25; the build id that matches them to the symbols,
# and the unwind tables, stay).
python3 "$root/tools/installer/symbols.py" linux-x86_64 "$version" "$dist" "${programs[@]/#/$bin/}"
for p in "${programs[@]}"; do strip --strip-all "$tree/usr/bin/$p"; done
# The oldest glibc the programs run on: the newest GLIBC_x.y symbol version they ask for. The
# .deb and .rpm require it, so a system too old refuses the package instead of installing
# programs that can't start ("GLIBC_2.38 not found" on Debian 12 from a build on Ubuntu 24.04).
glibc=$(for p in "${programs[@]}"; do objdump -T "$tree/usr/bin/$p"; done \
  | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -1)
[ -n "$glibc" ] || { echo "no glibc version found in the programs" >&2; exit 1; }
echo "the programs need glibc $glibc or newer"
cp "$root/LICENSE" "$root/ATTRIBUTION.md" "$tree/usr/share/doc/swoopdl/"
cp -R "$root/target/extension/bundle" "$tree/usr/share/swoopdl/extension"
chmod +x "$tree/usr/share/swoopdl/extension/install-local.sh"
cat > "$tree/usr/share/applications/swoopdl-browser.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Swoopdl Browser Setup
Comment=Set up the bundled browser extension
Exec=sh /usr/share/swoopdl/extension/install-local.sh
Terminal=true
Categories=Network;
EOF
python3 - "$root/art/Swoopdl.png" \
  "$tree/usr/share/icons/hicolor/256x256/apps/swoopdl.png" <<'EOF'
import sys
from PIL import Image
Image.open(sys.argv[1]).convert("RGBA").resize((256, 256), Image.LANCZOS).save(sys.argv[2])
EOF
cat > "$tree/usr/share/applications/swoopdl.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Swoopdl
Comment=Download manager
Exec=swoopdl %U
Icon=swoopdl
Terminal=false
Categories=Network;FileTransfer;
MimeType=x-scheme-handler/magnet;application/x-bittorrent;
EOF

# tar.gz: programs, licence, and local browser setup in one folder.
plain="$stage/$name"
mkdir -p "$plain"
for p in "${programs[@]}"; do cp "$tree/usr/bin/$p" "$plain/"; done
cp "$root/LICENSE" "$root/ATTRIBUTION.md" "$plain/"
cp -R "$tree/usr/share/swoopdl/extension" "$plain/extension"
tar -C "$stage" -czf "$dist/$name.tar.gz" "$name"

# .deb. Its dependencies, and the .rpm's: the libraries the window loads at run time. Without
# libxkbcommon-x11 (winit's keyboard under X11, and XWayland, SDL's default in a Wayland session)
# the window panics at start; EGL draws Wayland windows; without the CA certificates no download
# starts, HTTP ones too ("No CA certificates were loaded from the system"). GTK pulls in the rest.
# Browser native-host manifests are intentionally not packaged.
system="$stage/system"
mkdir -p "$system"
deb="$stage/deb"
cp -r "$tree" "$deb"
# Debian and Ubuntu's Firefox reads /usr/lib/mozilla; lib64's is Fedora's and openSUSE's.
cp -r "$system/." "$deb/"
rm -rf "$deb/usr/lib64"
mkdir -p "$deb/DEBIAN"
cat > "$deb/DEBIAN/control" <<EOF
Package: swoopdl
Version: $version
Architecture: amd64
Maintainer: Swoopdl Project
Depends: libc6 (>= $glibc), libgtk-3-0, libgl1, libegl1, libxkbcommon0, libxkbcommon-x11-0, ca-certificates
Section: net
Priority: optional
Description: Download manager
 Speeds up downloads with several connections at once, resumes them, downloads videos and torrents.
EOF
dpkg-deb --root-owner-group --build "$deb" "$dist/$name.deb"

# .rpm, from the same tree (nothing is compiled by rpmbuild).
rpm="$stage/rpm"
mkdir -p "$rpm"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
cat > "$rpm/SPECS/sdl.spec" <<EOF
Name: swoopdl
Version: $version
Release: 1
Summary: Download manager
License: GPL-3.0-or-later
Requires: glibc >= $glibc, gtk3, mesa-libGL, mesa-libEGL, libxkbcommon, libxkbcommon-x11, ca-certificates
AutoReqProv: no

%description
Speeds up downloads with several connections at once, resumes them, catches them from the
browser, and downloads videos and torrents.

%install
cp -a "$tree/." %{buildroot}/
cp -a "$system/." %{buildroot}/

%files
/usr/bin/swoopdl
/usr/bin/swoopdl-nmhost
/usr/bin/swoopdl-cli
/usr/bin/swoopdl-mcp
/usr/share/applications/swoopdl.desktop
/usr/share/applications/swoopdl-browser.desktop
/usr/share/icons/hicolor/256x256/apps/swoopdl.png
/usr/share/swoopdl
/usr/share/doc/swoopdl
$(cd "$system" && find . -mindepth 1 -type d | sed 's|^\.||' | grep -Ev '^/(etc|etc/opt|usr|usr/lib|usr/lib64)$' | sort | sed 's|^|%dir |')
$(cd "$system" && find . -type f | sed 's|^\.||' | sort)
EOF
rpmbuild --define "_topdir $rpm" --define "_build_id_links none" -bb "$rpm/SPECS/sdl.spec"
cp "$rpm"/RPMS/x86_64/*.rpm "$dist/$name.rpm"

# AppImage: the same tree as an AppDir.
appdir="$stage/AppDir"
cp -r "$tree" "$appdir"
ln -s usr/bin/swoopdl "$appdir/AppRun"
cp "$tree/usr/share/applications/swoopdl.desktop" "$appdir/"
cp "$tree/usr/share/icons/hicolor/256x256/apps/swoopdl.png" "$appdir/"
tool="$stage/appimagetool"
python3 "$root/tools/installer/fetch_release.py" AppImage/appimagetool continuous appimagetool-x86_64.AppImage "$tool"
chmod +x "$tool"
# blocks make the AppImage start slower (AppImage/appimagetool#64).
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$tool" \
  --mksquashfs-opt -Xcompression-level --mksquashfs-opt 22 "$appdir" "$dist/$name.AppImage"

rm -rf "$stage"
(cd "$dist" && sha256sum "$name".* > SHA256SUMS-linux.txt)
ls -l "$dist"
