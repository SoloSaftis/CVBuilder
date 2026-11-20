# Privacy policy

CVBuilder runs on your own computer. It has no account, no analytics, no advertising and no
automatic update check, and it does not send your CV or anything else to the CVBuilder project.

## What stays on your computer

- Your CV, drafts, photo and settings. In the portable version they are kept in the
  `CVBuilder-data` folder next to the program; otherwise in your user folder. You can turn off
  "Remember the draft on this computer" in the status bar.
- Your ready answers for application forms (work permit, notice period, salary expectation,
  references and the like), if you write any. They are kept in the same data folder, never in
  the CV file or the CV, and are not sent to an AI. "Delete all answers" removes them.
- API keys you enter for an AI service are kept in your operating system's keychain (Windows
  Credential Manager, macOS Keychain, Secret Service on Linux), not in a file.

## When CVBuilder uses the internet

Only when you ask it to:

- **Fetching a job ad** from a link you enter: CVBuilder downloads that page.
- **Checking for updates** (Help → Check for updates): CVBuilder asks GitHub for the number of
  the newest version. Nothing about you or your CV is sent, and nothing is downloaded by itself.
- **The AI assistant**, if you connect one: what you send in the chat, and the parts of your CV
  the assistant needs, go to the service you chose (Anthropic Claude, OpenAI ChatGPT, Google
  Gemini or xAI Grok), under that service's own privacy policy. A local model (Ollama,
  LM Studio …) stays on your computer. Without a connected AI, nothing is sent.
- **MCP:** while CVBuilder is open, AI apps on the same computer that you connect to it (such as
  Claude Code) can read and edit the open CV. The connection is only on this computer
  (127.0.0.1) and needs a key that only CVBuilder knows; nothing is reachable from other
  computers.

## Contact

Questions or concerns: open an issue at https://github.com/SoloSaftis/CVBuilder/issues.
