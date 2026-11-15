# The AI assistant

[Back to the user guide](README.md)

The AI assistant is **optional**. Everything else in CVBuilder works without it.

With an AI connected, you can ask for changes in your own words, for example
"make the profile shorter" or "tailor the CV to the job ad". The AI changes your
CV directly. You see the changes in the preview as it works, and you can undo
them.

The AI follows strict rules:

- It never makes up facts, employers, tools, numbers, results or dates.
- Where a number would make a point stronger but you have not given it, it
  writes `[tall]` ("[number]"). CVBuilder warns you before download until you
  fill it in or remove it.
- It writes the CV in the CV's language, and answers you in the language you
  write to it in.
- It follows recruiter and ATS practice: short bullet points that start with a
  verb, the ad's own words where they are true, no "I", no personal data such as
  date of birth.

## Choosing an AI

Open **AI → Connect AI** (or the **AI** section in **File → Settings**). You can
use:

| Service | Made by | What you need |
|---|---|---|
| **Claude** | Anthropic | Your own API key |
| **ChatGPT** | OpenAI | Your own API key |
| **Gemini** | Google | Your own API key |
| **Grok** | xAI | Your own API key |
| **Local model** (shown as "Lokal modell") | You, on your own computer | A program such as Ollama or LM Studio. No key, no cost |
| **Claude Code** | Anthropic | Claude Code installed and signed in on this computer |

### Claude, ChatGPT, Gemini or Grok

1. In **Connect AI**, choose the service.
2. **Create an API key** on the service's web page. The window has a link:
   - Claude: <https://console.anthropic.com/settings/keys>
   - ChatGPT: <https://platform.openai.com/api-keys>
   - Gemini: <https://aistudio.google.com/app/apikey>
   - Grok: <https://console.x.ai>
3. **Paste the key** into the box.
4. Read what is sent and who charges for it, then tick
   **I understand and want to connect**.
5. Click **3. Test and connect**. CVBuilder tries the key and picks a
   recommended model. You can choose another in **Model**.

The service charges you for what you use, on your own key.

The key is kept in your computer's keychain (Windows Credential Manager, the
macOS Keychain, or Secret Service on Linux). It is never saved in your CV file or
in CVBuilder's data folder. **Disconnect and delete the key** removes it again.

### A model on your own computer (Ollama, LM Studio …)

A local model runs on your own computer. Nothing leaves your computer, and it
costs nothing.

1. Install a local AI program, for example
   [Ollama](https://ollama.com/download), and download a model, for example by
   running `ollama pull qwen3`.
2. In **Connect AI**, choose the local model option ("Lokal modell").
3. Enter the server address, or click a name to fill it in:
   Ollama (`http://localhost:11434/v1`), LM Studio (`http://localhost:1234/v1`),
   llama.cpp, Jan, vLLM or LocalAI. **Find local servers** looks for them.
4. A key is only needed if your server asks for one.
5. Click **3. Test and connect**.

Small models can struggle with CVBuilder's tools. Models such as qwen3 or
llama3.1 work best.

If the address is not on this computer, CVBuilder warns you that your messages
and CV text are sent there.

### Claude Code

If you have [Claude Code](https://claude.com/claude-code) installed and signed
in, CVBuilder finds it by itself. No API key is needed in CVBuilder: Claude Code
uses your own Claude account.

- In the chat, open the list under **You are talking to** and choose
  **Claude Code · your own sign-in, via MCP**.
- Claude Code runs on your computer with only CVBuilder's CV tools. It cannot
  open your files, run commands or browse the web from CVBuilder.

## Using the chat

Open the chat with the **AI assistant** tab on the right, or **AI → Open the
chat**.

- **You are talking to** shows which AI you use. Change it at any time.
  Under it, a line says where your text goes.
- Type your request and press **Enter** to send. **Shift+Enter** adds a new line.
- When the chat is empty, it suggests requests, for example
  "Tailor the CV to the job ad", "Write a short profile" or
  "Get the CV down to two pages". Click one to send it.
- While the AI works, you see each step, for example "Reading the CV" or
  "Changed the profile". **Stop** stops it. Changes it already made stay, and
  can be undone.
- **New conversation** starts over. Your CV is not changed.

There are also AI buttons in the form. They send a ready request to the chat:

- **Tailor the CV with AI** and **Find keywords with AI** on the job ad card,
- **Write profile with AI** under the profile,
- **Improve the bullet points with AI** on each job,
- **Ask the AI about the missing keywords** in the checklist.

With an AI connected, you can also import an old CV with AI, translate the CV,
and add a menu language. See
[Importing an old CV](import-and-job-ads.md#importing-a-cv-you-already-have) and
[Languages](languages.md).

## Seeing and undoing changes

After each answer, the chat shows a box called **Changes to the CV** with one
line for each change.

- **Undo these changes** puts the CV back as it was before that answer.
- Undone boxes say "Undone".

When you download, CVBuilder still asks you to confirm that everything in the CV
is true – also what the AI wrote.

## What is sent

Nothing is sent unless you connect an AI and use it.

- **Claude, ChatGPT, Gemini or Grok:** your chat messages, the parts of your CV
  the AI reads, and the job ad go to that company, under its own privacy policy.
- **Importing a CV with AI:** the text of the CV file goes to the AI service.
- **Translating with AI:** your CV text goes to the AI service.
- **Adding a menu language with AI:** only CVBuilder's menu texts go to the AI.
- **A local model on this computer:** everything stays on your computer.
- **Claude Code:** runs on your computer and uses your Claude account; the CV
  text it reads goes to Anthropic like any Claude Code conversation.

Your photo is never sent to the AI. It only sees that there is one.

## Using CVBuilder from other AI apps (MCP)

CVBuilder is also an **MCP server**. MCP is a standard way for AI apps to use
tools. AI apps that support MCP, such as Claude Code and Claude Desktop, can read
and change the CV that is open in CVBuilder. You see every change at once, and
each one can be undone in CVBuilder's chat.

### How to connect

1. Open CVBuilder.
2. Choose **AI → MCP for other apps**, or click **MCP** at the top of the chat.
3. The window **Use CVBuilder from another AI app** shows exactly what to use on
   your computer:
   - **Claude Code:** a command to run in a terminal. It looks like this, with
     the real path to CVBuilder on your computer:

     ```
     claude mcp add cvbuilder -- "<path to CVBuilder>" --mcp
     ```

   - **Other apps (for example Claude Desktop):** a short piece of settings to
     add to the app's MCP settings. It looks like this:

     ```json
     {
       "mcpServers": {
         "cvbuilder": {
           "command": "<path to CVBuilder>",
           "args": ["--mcp"]
         }
       }
     }
     ```

4. Click **Copy** next to the one you need, and paste it into the terminal or the
   app's settings. Use the text CVBuilder shows, since it has the right path.

### Good to know

- **CVBuilder must be open** while the other AI app works. If it is not, the AI
  app gets the message "CVBuilder is not open".
- The connection stays on this computer (127.0.0.1) and needs a key that only
  CVBuilder knows. Nothing is opened to the internet.
- The status bar shows **MCP on** when other AI apps can connect.
- Changes from another AI app show in CVBuilder's chat as
  **Changed by an AI app over MCP**, with **Undo these changes**.

## Next step

[Updates and help](updates-and-help.md)
