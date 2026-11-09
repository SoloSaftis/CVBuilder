# SHARED – used by Windows, Linux and macOS

Everything in this folder is shared by all platforms. Platform folders (`../windows`, `../linux`,
`../macos`) only start the app and package it. Fix a bug or change a rule here, once.

| Folder | Contents |
|---|---|
| `cv-core/` | The project file format, recruiter/ATS checks, the designs (`src/layouts.rs`, remakes in `src/designs.json`, import format in `DESIGN-FORMAT.md`), and Word + PDF output through WordCraft. No UI, no network. |
| `cv-mcp/` | The CV editing tools an AI may use (`cv_read`, `cv_check`, `cv_job_save` …) and an MCP server (JSON-RPC 2.0) that offers them. The in-app chat and outside AI apps use the same tools. |
| `cv-ai/` | Optional AI: the chat's tool loop for Claude (Anthropic Messages API), ChatGPT, Gemini and Grok (OpenAI-compatible) with the person's own key (kept in the OS keychain), and for the person's installed Claude Code, which reaches the CV over MCP. |
| `app/` | The desktop window (egui): form, live page preview, AI chat, checklist, photo editor, and the local MCP link. A library; each platform calls `cvbuilder_app::run()`, which with `--mcp` runs as an MCP server on stdin/stdout instead. |
| `html-app/` | The HTML builder (v1) and the reference implementation of the rules. Its tests and `tools/parity-fixture.cjs` keep the Rust rules identical. |
| `assets/` | Icons (regenerate with `cargo run -p cvbuilder-app --example make_icons`) the DM Sans font (OFL), and `craft-fonts/`: Japanese, Chinese, Korean, Arabic, Hebrew, Thai, Devanagari and Georgian fonts (OFL, about 31 MB), built into the app and the PDF export through `CRAFT_FONTS_DIR` in `.cargo/config.toml`. |
| `packaging/` | Steps every package uses, such as the third-party licence list. |

Keeping both apps in step: after changing a rule in `html-app`, run
`node shared/html-app/tools/parity-fixture.cjs` (and `rust-wordlists.cjs` for word lists), then
`cargo test -p cv-core`.

## AI chat and MCP

The chat beside the CV sends the person's request to the AI with CVBuilder's tools. The AI reads
the CV, changes it with the tools, and the preview updates as it works. Every change made in one
answer is listed under it with «Angre disse endringene». The rules (never invent facts, write
`[tall]` where a number is missing, recruiter/ATS practice) are in `cv-mcp/src/tools.rs`
(`INSTRUCTIONS`), and the export checks still block placeholders and ask for approval.

- **API services** (Claude, ChatGPT, Gemini, Grok): the tool calls come back to the window and run
  on the open CV.
- **Local models** (Ollama, LM Studio, llama.cpp, Jan, vLLM, LocalAI, or any OpenAI-compatible
  address): the same tool loop, no key, and with a localhost address nothing leaves the computer.
  «Finn lokale servere» checks the usual ports.
- **Claude Code** (the person's own `claude` program and sign-in): started headless with
  `--tools ""` (no file, shell or web tools), `--strict-mcp-config` and CVBuilder as its only MCP
  server, so its tool calls reach the window over MCP.
- **Any MCP app**: `CVBuilder --mcp` is an MCP server on stdin/stdout. It hands each call to the open
  window over 127.0.0.1 with a random token (`mcp-link.json` in the data folder). Claude Code:
  `claude mcp add cvbuilder -- "<path to CVBuilder>" --mcp`. The «MCP» button in the chat shows the
  exact command and config for this computer.
