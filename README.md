# CVBuilder

Build a CV that recruiters and applicant-tracking systems (ATS) read correctly: one text column,
standard headings, real text, two A4 pages recommended. 31 designs (CVBuilder's own plus single-column
remakes of Reactive Resume and RenderCV templates) and your own imported designs, Word and PDF from one layout,
a recruiter checklist, and an optional AI chat that edits the CV while you watch: Claude, ChatGPT,
Gemini or Grok with your own API key, a local model (Ollama, LM Studio …) on your own computer, or
your own Claude Code over MCP. Colours are chosen separately from the design, per CV or per section. CVBuilder is also an MCP
server, so other AI apps can work on the open CV. Page layout (size, margins, columns, text size,
spacing, page breaks and a page target) is saved with each CV, and the interface is in 21 languages
(it follows the system by default).

## Download

Free, ready to run, nothing to install. Click the one for your computer; it is always the newest version.

### ⬇️ [Download for Windows](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-windows-x64-portable.zip)
Windows 10 or 11. Open the downloaded folder and double-click **CVBuilder**.

### ⬇️ [Download for Mac](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-macos-arm64.zip)
Mac with an Apple chip (M1 or newer). Open the download and drag **CVBuilder** to Applications.

### ⬇️ [Download for Linux](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-linux-x86_64.AppImage)
One file. Right-click it → Properties → allow it to run as a program, then double-click it.
([Folder version](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-linux-x86_64-portable.tar.gz) for those who prefer it.)

**The first time you open it**, your computer may warn you, because the app is not signed yet:

- **Windows** says "Windows protected your PC": click **More info**, then **Run anyway**.
- **Mac** says Apple "could not verify" CVBuilder: click **Done**. Open System Settings →
  Privacy & Security, scroll down and click **Open Anyway** next to CVBuilder, then confirm. You
  only do this once.

Your CV stays on your own computer ([privacy policy](PRIVACY.md)). All versions and what changed:
[Releases](https://github.com/SoloSaftis/CVBuilder/releases). [Code signing policy](CODE_SIGNING_POLICY.md):
free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

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
  fixes, and an AI chat beside the CV that changes it live through MCP tools (every change can be
  undone). Build for this computer: `cargo build -p cvbuilder-windows --release`
  (or `-linux`, `-macos`). Package: see each platform folder.

Both apps read and write the same project files and apply the same rules; a parity test
(`shared/cv-core/tests/parity.rs`) runs both on the same CVs and requires identical results.

## Privacy

No personal data belongs in this repository. Tests use fictional people only. Your own CVs,
photos and working files go in `outputs/` and `work/`, which git ignores.

## Designs

Every design is one column of real text with standard headings, so each reads the same in an
applicant-tracking system. Designs from Reactive Resume (MIT, © Amruth Pillai) and RenderCV (MIT,
© Sina Atalay and contributors) are remakes: their look (fonts, colours, header and headings) on
CVBuilder's single-column layout; sidebars and columns are left out on purpose. Licence texts are in
`shared/assets/licenses/`. Your own designs are `.cvdesign.json` files: see
[`shared/cv-core/DESIGN-FORMAT.md`](shared/cv-core/DESIGN-FORMAT.md).

## Licence

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Unless you say otherwise, a contribution you submit is licensed the same way, with no additional
terms. Bundled components keep their own licences (see `THIRD-PARTY.txt` in each package). WordCraft and the Craft AI integrations are by the ArtCraft/storytold contributors.
