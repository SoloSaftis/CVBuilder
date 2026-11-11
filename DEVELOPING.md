# Developing CVBuilder

## Repository layout

| Folder | What it is |
|---|---|
| [`crates/`](crates/) | **The app, shared by every platform.** CV rules and Word/PDF output (`cv-core`), AI tools and MCP (`cv-mcp`), AI services (`cv-ai`) and the window (`app`). Change behaviour here. |
| [`web/`](web/) | The reference implementation of the CV rules in JavaScript; a parity test keeps the app's rules identical to it. |
| [`assets/`](assets/) | Icons, fonts (DM Sans, Phosphor icons, `craft-fonts` for other scripts), the bundled interface languages (`languages/`), CV headings and dates per language (`cv-languages.json`), screenshots and licence texts. |
| [`platforms/`](platforms/) | One folder each for `windows`, `linux` and `macos`: the launcher and the packaging (zip and MSIX; AppImage and tar.gz; `CVBuilder.app`). |
| [`packaging/`](packaging/) | Scripts every platform uses: the third-party licence list and the language-pack tools. |
| [`.github/`](.github/) | The build (`workflows/build.yml`: tests, then each platform, and releases from `v*` tags) and the release page template. |
| [`.signpath/`](.signpath/) | The SignPath configuration for signing the Windows program. |

Each platform launcher is a few lines that call `cvbuilder_app::run()` from `crates/app`. Anything
that is the same on every platform belongs in `crates/`.

## Building and testing

- Build for this computer: `cargo build -p cvbuilder-windows --release` (or `-linux`, `-macos`).
  Packaging: see each platform folder.
- Tests: `cargo test --workspace --release`. The parity test (`crates/cv-core/tests/parity.rs`)
  runs the app's rules and the reference rules in `web/` on the same CVs and requires identical
  results.
- Every screen in every language: `cargo test --release -p cvbuilder-app ui_qa -- --ignored --nocapture --test-threads=1`
  (`CVBUILDER_QA_LANGS=de,fr` for some languages only).
- Screenshots: `CVBUILDER_SHOTS=<folder> CVBUILDER_QA_LANGS=en cargo test --release -p cvbuilder-app store_shots -- --ignored`.

## Languages

Interface languages are `assets/languages/<code>.cvlang.json`; `node packaging/lang-tools.cjs`
makes a template, lists missing strings and checks a pack. CV headings, dates and the words the
CV reader needs are in `assets/cv-languages.json` (`lang-tools.cjs cvdata`). Language names in
every interface language come from `lang-tools.cjs names`.

## Releasing

Set the version in `Cargo.toml`, merge to `main`, then push a tag such as `v1.0.1`. The build
makes a draft release with the page from `.github/release-notes.md`, adds each platform's files
and publishes it when all of them are there.

## Designs

Every design is one column of real text with standard headings, so each reads the same in an
applicant-tracking system. Designs from Reactive Resume (MIT, © Amruth Pillai) and RenderCV (MIT,
© Sina Atalay and contributors) are remakes: their look (fonts, colours, header and headings) on
CVBuilder's single-column layout; sidebars and columns are left out on purpose. Licence texts are in
`assets/licenses/`. Your own designs are `.cvdesign.json` files: see
[`crates/cv-core/DESIGN-FORMAT.md`](crates/cv-core/DESIGN-FORMAT.md).

## Personal data

No personal data belongs in this repository. Tests and screenshots use fictional people only.
Your own CVs, photos and working files go in `outputs/`, `work/` or `private/`, which git ignores.
