#!/usr/bin/env bash
# Builds, in platforms/linux/dist/:
#   CVBuilder-<version>-x86_64.AppImage: one file; make it executable and double-click it.
#   CVBuilder-<version>-linux-x86_64-portable.tar.gz: unpack anywhere and run ./CVBuilder.
# Needs Rust 1.95+ and the usual desktop development packages (see README in this folder).
set -euo pipefail
cd "$(dirname "$0")/../../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cargo build -p cvbuilder-linux --release --locked
name="CVBuilder-${version}-linux-$(uname -m)-portable"
stage="platforms/linux/dist/${name}"
rm -rf "$stage" && mkdir -p "$stage/CVBuilder-data"
cp target/release/CVBuilder "$stage/"
cp assets/cvbuilder-256.png "$stage/cvbuilder.png"
cat > "$stage/cvbuilder.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=CVBuilder
Comment=CV-er som rekrutterere og søknadssystemer leser riktig
Exec=CVBuilder
Icon=cvbuilder
Categories=Office;
DESKTOP
cp platforms/linux/packaging/LES-MEG.txt "$stage/LES MEG.txt"
echo "Her lagrer CVBuilder utkast og innstillinger i bærbar modus." > "$stage/CVBuilder-data/Om denne mappen.txt"
node packaging/third-party.cjs "$stage/THIRD-PARTY.txt"
tar -C platforms/linux/dist -czf "platforms/linux/dist/${name}.tar.gz" "$name"
echo "Linux: platforms/linux/dist/${name}.tar.gz"

# AppImage: the program, its icon and desktop entry in one file. It has no CVBuilder-data folder
# next to it, so the app keeps drafts and settings in the user's own folders.
appdir="platforms/linux/dist/CVBuilder.AppDir"
rm -rf "$appdir" && mkdir -p "$appdir/usr/bin"
cp target/release/CVBuilder "$appdir/usr/bin/CVBuilder"
cp assets/cvbuilder-256.png "$appdir/cvbuilder.png"
cp "$stage/cvbuilder.desktop" "$appdir/cvbuilder.desktop"
cat > "$appdir/AppRun" <<'APPRUN'
#!/bin/sh
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/CVBuilder" "$@"
APPRUN
chmod +x "$appdir/AppRun"
# appimagetool, pinned to a release and checked against its published checksum.
tool="platforms/linux/dist/tools/appimagetool-1.9.1-x86_64"
if [ ! -x "$tool" ]; then
  mkdir -p "$(dirname "$tool")"
  curl -fsSL -o "$tool" https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
  echo "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0  $tool" | sha256sum -c -
  chmod +x "$tool"
fi
appimage="platforms/linux/dist/CVBuilder-${version}-$(uname -m).AppImage"
ARCH="$(uname -m)" APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$appimage"
echo "Linux: $appimage"
