# CVBuilder

Build a CV that recruiters and applicant-tracking systems (ATS) read correctly: one text column,
standard headings, real text, at most two A4 pages. Four layouts, Word and PDF from one layout,
a recruiter checklist, and optional AI help with your own API key (Claude, ChatGPT, Gemini, Grok).

## Repository layout

| Folder | What it is |
|---|---|
| [`shared/`](shared/) | **SHARED by all platforms.** The CV rules, layouts, Word/PDF output, AI layer, the desktop window, icons, and the HTML builder (v1). Change behaviour here. |
| [`windows/`](windows/) | Windows only: the launcher, the exe icon and version info, portable zip and MSIX packaging. |
| [`linux/`](linux/) | Linux only: the launcher and portable tar.gz packaging with a desktop entry. |
| [`macos/`](macos/) | macOS only: the launcher and the `CVBuilder.app` bundle. |
| `.github/workflows/build.yml` | Tests the shared code, then builds each platform from its own folder. |

Each platform launcher is a few lines that call `cvbuilder_app::run()` from `shared/app`. Anything
that is the same on every platform belongs in `shared/`.

## Two apps

- **v1 – HTML app** (`shared/html-app`): one self-contained `CVBuilder.html`, double-click to run
  in Edge or Chrome, fully offline. Build: `node shared/html-app/tools/build-single.cjs`.
- **v2 – desktop app** (Rust): a live preview of the exact exported pages, photo crop and light
  fixes, and optional AI. Build for this computer: `cargo build -p cvbuilder-windows --release`
  (or `-linux`, `-macos`). Package: see each platform folder.

Both apps read and write the same project files and apply the same rules; a parity test
(`shared/cv-core/tests/parity.rs`) runs both on the same CVs and requires identical results.

## Privacy

No personal data belongs in this repository. Tests use fictional people only. Your own CVs,
photos and working files go in `outputs/` and `work/`, which git ignores.

## Licence

MIT OR Apache-2.0. Bundled components keep their own licences (see `THIRD-PARTY.txt` in each
package). WordCraft and the Craft AI integrations are by the ArtCraft/storytold contributors.
