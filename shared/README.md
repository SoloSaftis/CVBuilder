# SHARED – used by Windows, Linux and macOS

Everything in this folder is shared by all platforms. Platform folders (`../windows`, `../linux`,
`../macos`) only start the app and package it. Fix a bug or change a rule here, once.

| Folder | Contents |
|---|---|
| `cv-core/` | The project file format, recruiter/ATS checks, the four layouts, and Word + PDF output through WordCraft. No UI, no network. |
| `cv-ai/` | Optional AI help: Claude (Anthropic Messages API), ChatGPT, Gemini and Grok (OpenAI-compatible), keys in the OS keychain. |
| `app/` | The desktop window (egui): form, live page preview, checklist, photo editor, AI dialogs. A library; each platform calls `cvbuilder_app::run()`. |
| `html-app/` | The HTML builder (v1) and the reference implementation of the rules. Its tests and `tools/parity-fixture.cjs` keep the Rust rules identical. |
| `assets/` | Icons. Regenerate with `cargo run -p cvbuilder-app --example make_icons`. |
| `packaging/` | Steps every package uses, such as the third-party licence list. |

Keeping both apps in step: after changing a rule in `html-app`, run
`node shared/html-app/tools/parity-fixture.cjs` (and `rust-wordlists.cjs` for word lists), then
`cargo test -p cv-core`.
