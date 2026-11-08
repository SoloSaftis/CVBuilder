# CVBuilder – Windows

Windows only. The app itself is in [`../shared`](../shared).

- `src/main.rs` – starts the shared app without a console window.
- `build.rs` – embeds the icon and version details into `CVBuilder.exe`.
- `packaging/package.ps1` – builds `windows/dist/`:
  - `CVBuilder-<version>-windows-x64-portable.zip`: unzip and run. The `CVBuilder-data` folder keeps
    drafts and settings next to the exe (USB stick); delete it to use the user profile instead.
  - `CVBuilder-<version>-x64.msix`: installer. Without `-Pfx`, it is signed with a throwaway
    test certificate and installs only where `CVBuilder-test-certificate.cer` is trusted. For
    friends' PCs, sign with a real code-signing certificate:
    `./windows/packaging/package.ps1 -Pfx cert.pfx -PfxPassword … -Publisher "CN=…"`.

Needs Rust 1.95+, Node.js, and the Windows SDK (`makeappx`, `signtool`) for the MSIX.
