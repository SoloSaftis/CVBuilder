# CVBuilder

**Version 1.0** · Windows, Mac and Linux · free and open source

Build a CV that recruiters and applicant-tracking systems (ATS) read correctly: one text column,
standard headings, real text, two A4 pages recommended.

- **Import your old CV**: Word, PDF, RTF, text and more, read offline into jobs, education,
  skills and the rest. Or start from scratch.
- **See the real pages while you type**: the preview is exactly the PDF and Word file you download.
- **31 designs** (CVBuilder's own plus single-column remakes of Reactive Resume and RenderCV
  templates) and your own imported designs. Colours per CV or per section. Page layout (size,
  margins, columns, text size, spacing, page breaks and a page target) is saved with each CV.
- **Recruiter checklist and job match**: paste the job ad and see which keywords your CV is missing.
- **86 languages**, for both the menus and the CV headings and dates. CVBuilder starts in your
  computer's language and asks on first start.
- **Saves like Word**: Ctrl+S, a reminder before you close with unsaved changes, and autosave every
  5 minutes.
- **Optional AI chat** that edits the CV while you watch (every change can be undone): Claude,
  ChatGPT, Gemini or Grok with your own API key, a local model (Ollama, LM Studio …) on your own
  computer, or your own Claude Code. CVBuilder is also an MCP server, so other AI apps can work on
  the open CV.

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
| [`crates/`](crates/) | **The app, shared by every platform.** CV rules and Word/PDF output (`cv-core`), AI tools and MCP (`cv-mcp`), AI services (`cv-ai`) and the window (`app`). Change behaviour here. |
| [`web/`](web/) | The reference implementation of the CV rules in JavaScript; a parity test keeps the app's rules identical to it. |
| [`assets/`](assets/) | Icons, fonts (DM Sans, Phosphor icons, `craft-fonts` for other scripts), the bundled interface languages (`languages/`), CV headings and dates per language (`cv-languages.json`) and licence texts. |
| [`platforms/`](platforms/) | One folder each for `windows`, `linux` and `macos`: the launcher and the packaging (zip and MSIX; AppImage and tar.gz; `CVBuilder.app`). |
| [`packaging/`](packaging/) | Scripts every platform uses: the third-party licence list and the language-pack tools. |
| [`.github/`](.github/) | The build (`workflows/build.yml`: tests, then each platform, and releases from `v*` tags) and the release page template. |
| [`.signpath/`](.signpath/) | The SignPath configuration for signing the Windows program. |

Each platform launcher is a few lines that call `cvbuilder_app::run()` from `crates/app`. Anything
that is the same on every platform belongs in `crates/`.

## Building

Build for this computer: `cargo build -p cvbuilder-windows --release` (or `-linux`, `-macos`).
Package: see each platform folder. Tests: `cargo test --workspace --release`. The parity test
(`crates/cv-core/tests/parity.rs`) runs the app's rules and the reference rules in `web/` on the
same CVs and requires identical results.

Interface languages are `assets/languages/<code>.cvlang.json`; `node packaging/lang-tools.cjs`
makes a template, lists missing strings and checks a pack.

## Privacy

No personal data belongs in this repository. Tests use fictional people only. Your own CVs,
photos and working files go in `outputs/` and `work/`, which git ignores.

## Designs

Every design is one column of real text with standard headings, so each reads the same in an
applicant-tracking system. Designs from Reactive Resume (MIT, © Amruth Pillai) and RenderCV (MIT,
© Sina Atalay and contributors) are remakes: their look (fonts, colours, header and headings) on
CVBuilder's single-column layout; sidebars and columns are left out on purpose. Licence texts are in
`assets/licenses/`. Your own designs are `.cvdesign.json` files: see
[`crates/cv-core/DESIGN-FORMAT.md`](crates/cv-core/DESIGN-FORMAT.md).

## Licence

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Unless you say otherwise, a contribution you submit is licensed the same way, with no additional
terms. Bundled components keep their own licences (see `THIRD-PARTY.txt` in each package). WordCraft and the Craft AI integrations are by the ArtCraft/storytold contributors.
