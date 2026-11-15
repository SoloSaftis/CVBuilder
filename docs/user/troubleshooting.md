# Troubleshooting

[Back to the user guide](README.md)

## Starting the program

### Windows protected your PC

The first time you start the downloaded Windows version, Windows SmartScreen may
say **"Windows protected your PC"**. This happens because the program is not
signed with a known certificate yet.

1. Click **More info**.
2. Click **Run anyway**.

Windows remembers your choice. The version from the
[Microsoft Store](https://apps.microsoft.com/detail/9NCKXG5ZTL7L) does not show
this warning.

Also check that you unzipped the download first. Do not run CVBuilder from inside
the zip file.

### Mac: Apple could not verify CVBuilder

The first time, the Mac may say Apple **"could not verify"** CVBuilder.

1. Click **Done**.
2. Open **System Settings → Privacy & Security**.
3. Scroll down and click **Open Anyway** next to CVBuilder.
4. Confirm, and CVBuilder opens. The Mac remembers your choice.

The Mac download is for Macs with an Apple chip (M1 or newer).

### Linux: nothing happens when I double-click the AppImage

The file must be allowed to run as a program. Right-click it, choose
**Properties**, and allow running it as a program. Or, in a terminal:
`chmod +x CVBuilder-linux-x86_64.AppImage`.

### All views are hidden

If the window says **All views are hidden**, click **Show all**, or use
**View → Show all**.

### "The saved draft could not be opened"

CVBuilder could not read the remembered draft. It kept it as a backup
(`sikkerhetskopi.json` in the data folder) and shows the example instead. If you
saved your CV as a file, open it with **File → Open**.

## The AI assistant

### AI not connected

If the chat says **No AI connected**, or **Connect an AI first**:

- Choose **AI → Connect AI** and follow the steps in
  [The AI assistant](ai-assistant.md#choosing-an-ai).
- Everything else in CVBuilder works without AI.

### The key was not accepted

- Check that you pasted the **whole** API key, without spaces.
- Check on the service's web page that the key is active and that your account
  has credit.
- If the message says the service is busy or has reached a limit, wait a little
  and try again.

### The key is forgotten when I close CVBuilder (Linux)

If the computer has no keychain, CVBuilder says "This computer has no keychain,
so the key is only remembered until you close the program". Install and start a
Secret Service keyring (for example GNOME Keyring or KWallet), or paste the key
again next time.

### No local AI server found

- Start Ollama or LM Studio first, and make sure a model is downloaded.
- Check the address. Ollama uses `http://localhost:11434/v1`, LM Studio
  `http://localhost:1234/v1`.
- If the chat says it could not reach the local AI server, check that the server
  is still running.
- Small models may fail to use the tools. Try qwen3 or llama3.1.

### Claude Code is not signed in

If the chat says **"Claude Code is not signed in"**:

1. Open a terminal.
2. Run `claude`.
3. Sign in and follow the steps.
4. Go back to CVBuilder and try again.

If Claude Code does not appear in the chat's list at all, it is not installed, or
it is not where CVBuilder looks (on your PATH or in its usual install folder).
Install it, then restart CVBuilder.

If the chat says "The MCP connection could not be started on this computer, so
Claude Code cannot change the CV", restart CVBuilder. Security software that
blocks local connections on 127.0.0.1 can also cause this.

### "The AI used too many steps"

The request was too big for one answer. Split it into smaller requests, for
example one job at a time.

## Other AI apps (MCP)

### "CVBuilder is not open"

The other AI app (for example Claude Code or Claude Desktop) could not reach
CVBuilder.

- Open CVBuilder, then ask the AI app to try again.
- Use the exact command or settings shown in **AI → MCP for other apps**. If you
  moved CVBuilder to another folder, copy them again: the path has changed.
- If you have two copies of CVBuilder (for example a portable one and the Store
  one), each keeps its own data folder. Use the copy that the AI app's settings
  point to.

### "CVBuilder did not answer in time"

CVBuilder was busy, or a window in it was waiting for you. Ask the AI app to read
the CV again (it can call `cv_read`) to see whether the change was made, before
it tries again.

## Importing and job ads

### Hardly any text was found in the file

The file is probably a scanned PDF, which is a picture of text. Use the original
Word file, or copy the text and paste it in.

### The ad could not be fetched

- **The page requires sign-in:** open the ad in your browser, copy the text and
  paste it into **Ad text**.
- **The page showed almost no text:** the site builds its page with JavaScript.
  Copy the text from your browser and paste it in.

See [Using the job ad](import-and-job-ads.md#using-the-job-ad).

## Fonts and scripts

### Letters are missing or show as boxes

CVBuilder comes with fonts for the scripts of all its 86 languages: Latin,
Greek, Cyrillic, Arabic, Hebrew, Armenian, Georgian, Ethiopic, Thai, Khmer,
Japanese, Chinese, Korean and the Indian scripts. They are used both in the
window and in the PDF, wherever the design's own font does not have a letter.
If you still see boxes:

- Check that you have the newest version (**Help → Check for updates**).
- Report it on GitHub with the language and, if you can, a made-up example text.

### The Word file looks a little different

The Word file uses standard fonts such as Calibri, Arial or Georgia. If a font is
missing on the computer that opens the file, Word uses a similar one, and line
and page breaks can move. Always open the Word file and check the page breaks
before you send it.

### Copying text from a PDF gives scrambled letters

For scripts whose letters are joined or reordered when they are drawn – Arabic,
Hebrew and other right-to-left scripts, Hindi and the other Indian scripts, Thai
and Khmer – the PDF **looks** right, but when you copy text out of it (or an
applicant tracking system reads it) the letters can come out scrambled.

This is a known limit of the PDF writer CVBuilder uses.

**What to do:** send the **Word** file instead. The Word file has the real text
and is read correctly. Other scripts (Latin, Greek, Cyrillic, Chinese, Japanese,
Korean and so on) are not affected.

## Saving

### "Could not save"

- Choose another folder, for example Documents.
- On a USB stick, check that it is not full or write-protected.
- If the file is open in another program, close it there first.

### My CV was gone when I opened CVBuilder

- If **Remember the draft on this computer** was off, the draft was not kept.
  Open your saved file with **File → Open**.
- If you use the portable version, check that you started the copy with your
  `CVBuilder-data` folder next to it.

## Still stuck?

Open an issue at <https://github.com/SoloSaftis/CVBuilder/issues>. See
[Reporting a problem](updates-and-help.md#reporting-a-problem). Never attach
your real CV.
