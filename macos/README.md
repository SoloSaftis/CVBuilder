# CVBuilder – macOS

macOS only. The app itself is in [`../shared`](../shared).

- `src/main.rs` – starts the shared app.
- `packaging/package.sh` – builds `macos/dist/CVBuilder-<version>-macos-<arch>.zip` containing
  `CVBuilder.app` with its icon. Run it on a Mac with Rust 1.95+ and Node.js.

Unsigned builds open with right-click → Open the first time. To sign, set
`DEVELOPER_ID="Developer ID Application: Name (TEAMID)"` before running the script; notarising
for wider distribution is a further step with `xcrun notarytool`.
