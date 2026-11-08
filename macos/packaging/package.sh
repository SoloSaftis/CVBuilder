#!/usr/bin/env bash
# Builds macos/dist/CVBuilder-<version>-macos-<arch>.zip containing CVBuilder.app.
# Unsigned unless DEVELOPER_ID is set (e.g. "Developer ID Application: Name (TEAMID)").
set -euo pipefail
cd "$(dirname "$0")/../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
short=${version%%-*}
cargo build -p cvbuilder-macos --release --locked
app="macos/dist/CVBuilder.app"
rm -rf "$app" && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/CVBuilder "$app/Contents/MacOS/CVBuilder"
iconset="macos/dist/cvbuilder.iconset"; rm -rf "$iconset"; mkdir -p "$iconset"
for n in 16 32 128 256 512; do
  sips -z $n $n shared/assets/cvbuilder-512.png --out "$iconset/icon_${n}x${n}.png" >/dev/null
  d=$((n*2)); [ $d -le 512 ] && sips -z $d $d shared/assets/cvbuilder-512.png --out "$iconset/icon_${n}x${n}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/CVBuilder.icns" && rm -rf "$iconset"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>CVBuilder</string>
  <key>CFBundleIdentifier</key><string>no.cvbuilder.app</string>
  <key>CFBundleExecutable</key><string>CVBuilder</string>
  <key>CFBundleIconFile</key><string>CVBuilder</string>
  <key>CFBundleShortVersionString</key><string>${short}</string>
  <key>CFBundleVersion</key><string>${short}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
node shared/packaging/third-party.cjs "$app/Contents/Resources/THIRD-PARTY.txt"
if [ -n "${DEVELOPER_ID:-}" ]; then codesign --force --deep --options runtime --sign "$DEVELOPER_ID" "$app"; fi
zip_name="CVBuilder-${version}-macos-$(uname -m).zip"
(cd macos/dist && ditto -c -k --keepParent CVBuilder.app "$zip_name")
echo "macOS: macos/dist/${zip_name}"
