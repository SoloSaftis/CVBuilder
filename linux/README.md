# CVBuilder – Linux

Linux only. The app itself is in [`../shared`](../shared).

- `src/main.rs` – starts the shared app.
- `packaging/package.sh` – builds `linux/dist/CVBuilder-<version>-linux-<arch>-portable.tar.gz`
  with the binary, a `.desktop` entry, the icon and a `CVBuilder-data` folder (portable mode).

## Building

Rust 1.95 or newer (`rustup`), Node.js for the licence list, and desktop development packages:

- Fedora/Rocky/RHEL: `sudo dnf install gcc pkgconf-pkg-config libxkbcommon-devel wayland-devel libX11-devel dbus-devel fontconfig-devel`
- Debian/Ubuntu: `sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libdbus-1-dev libfontconfig-dev`

Then, from the repository root: `linux/packaging/package.sh`.
