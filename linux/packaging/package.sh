#!/usr/bin/env bash
# Builds linux/dist/CVBuilder-<version>-linux-x86_64-portable.tar.gz: unpack anywhere and run ./CVBuilder.
# Needs Rust 1.95+ and the usual desktop development packages (see README in this folder).
set -euo pipefail
cd "$(dirname "$0")/../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cargo build -p cvbuilder-linux --release --locked
name="CVBuilder-${version}-linux-$(uname -m)-portable"
stage="linux/dist/${name}"
rm -rf "$stage" && mkdir -p "$stage/CVBuilder-data"
cp target/release/CVBuilder "$stage/"
cp shared/assets/cvbuilder-256.png "$stage/cvbuilder.png"
cat > "$stage/cvbuilder.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=CVBuilder
Comment=CV-er som rekrutterere og søknadssystemer leser riktig
Exec=CVBuilder
Icon=cvbuilder
Categories=Office;
DESKTOP
cp linux/packaging/LES-MEG.txt "$stage/LES MEG.txt"
echo "Her lagrer CVBuilder utkast og innstillinger i bærbar modus." > "$stage/CVBuilder-data/Om denne mappen.txt"
node shared/packaging/third-party.cjs "$stage/THIRD-PARTY.txt"
tar -C linux/dist -czf "linux/dist/${name}.tar.gz" "$name"
echo "Linux: linux/dist/${name}.tar.gz"
