# Getting started

[Back to the user guide](README.md)

## Download and install

CVBuilder is free. Pick the version for your computer.

### Windows 10 or 11

You have two choices.

**Microsoft Store (easiest)**

- Open [CVBuilder in the Microsoft Store](https://apps.microsoft.com/detail/9NCKXG5ZTL7L)
  and click **Get**.
- It installs like any other app and keeps itself up to date.
- No warning from Windows on the first start.

**Download from GitHub (no installation)**

1. Download
   [CVBuilder-windows-x64-portable.zip](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-windows-x64-portable.zip).
2. Unzip it: right-click the file, then **Extract All**.
   Do not run the program from inside the zip file.
3. Open the new folder and double-click **CVBuilder**.

The first time, Windows may say "Windows protected your PC". Click
**More info**, then **Run anyway**. See [Troubleshooting](troubleshooting.md#windows-protected-your-pc).

### Mac (Apple chip, M1 or newer)

1. Download
   [CVBuilder-macos-arm64.zip](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-macos-arm64.zip).
2. Open the download and drag **CVBuilder** into **Applications**.
3. Open CVBuilder from Applications.

Not sure which Mac you have? Click the Apple menu, then **About This Mac**. If
it says *Chip: Apple M…*, this is the right download. Older Macs with an Intel
chip are not supported by this download.

The first time, the Mac may say Apple "could not verify" CVBuilder. See
[Troubleshooting](troubleshooting.md#mac-apple-could-not-verify-cvbuilder).

### Linux (64-bit PC)

**One-file version (AppImage)**

1. Download
   [CVBuilder-linux-x86_64.AppImage](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-linux-x86_64.AppImage).
2. Right-click the file, choose **Properties**, and allow running it as a
   program. (In a terminal: `chmod +x CVBuilder-linux-x86_64.AppImage`.)
3. Double-click it.

**Folder version**

1. Download
   [CVBuilder-linux-x86_64-portable.tar.gz](https://github.com/SoloSaftis/CVBuilder/releases/latest/download/CVBuilder-linux-x86_64-portable.tar.gz).
2. Unpack it anywhere, for example on a USB stick.
3. Run `CVBuilder` in the unpacked folder.

All versions and what changed in each:
[Releases](https://github.com/SoloSaftis/CVBuilder/releases).

## The first start

### The welcome window

The first time you open CVBuilder, a small **Welcome to CVBuilder** window asks
two things. Your computer's language is already chosen for both.

- **Menu language** – the language of the menus and buttons.
- **The CV's language** – the language of the headings and dates in your CV
  (for example "Work experience" and "now").

These are two separate choices. You can, for example, have English menus and a
German CV. Click **Start** when you are happy.

You can change both later:

- Menu language: **File → Settings**.
- CV language: the language list on the **Home** tab.

More in [Languages](languages.md).

### The example CV

CVBuilder opens with an example CV. The person and the companies in it are made
up. It shows a recommended layout and order.

- Type straight into the example to make it yours, or
- choose **File → New CV** (or **Home → New CV**) to start from an empty CV.

You can open the example again at any time with **File → Open example**.

### What you see

The window has three parts. You can hide any of them on the **View** tab.

- **Form** (left) – where you type your CV.
- **Preview** (middle) – the real pages, exactly as they will be downloaded.
- **AI and checklist** (right) – the optional AI assistant and the recruiter
  checklist.

At the top is a toolbar with tabs, like in Word: **File**, **Home**, **Insert**,
**Layout**, **Design**, **View**, **AI** and **Help**. The small arrow at the top
right folds the toolbar away to give you more room.

At the bottom is a status bar. It shows messages, an **Undo** button after a big
change, and the box **Remember the draft on this computer**.

## Where your CV is kept

CVBuilder keeps everything on your own computer. Nothing goes to the internet.

### Two ways to keep your CV

1. **The remembered draft.** While **Remember the draft on this computer** is
   ticked (in the status bar and in **File → Settings**), CVBuilder remembers
   your CV as you type. Next time you open the program, it is there again.
   Turn this off on a shared or public computer.
2. **Your own file.** Use **File → Save** (Ctrl+S) to save the CV as a file you
   choose, for example in your Documents folder. This is the safe copy you can
   keep, back up or move to another computer. See
   [Writing your CV](writing-your-cv.md#saving).

### Portable mode or the normal data folder

CVBuilder keeps its own things (the remembered draft, a backup copy, settings,
your own designs, language files and checklist) in a data folder.

**Portable mode.** If there is a folder named `CVBuilder-data` next to the
program, everything is kept in that folder. The status bar then shows
**Portable mode**.

- The Windows zip download and the Linux folder version come with this folder.
- You can put the whole program folder on a USB stick and use it on any computer.
- If you delete the `CVBuilder-data` folder, CVBuilder uses the normal data
  folder instead.

**The normal data folder.** Without a `CVBuilder-data` folder (Microsoft Store,
Mac, Linux AppImage), CVBuilder uses your own user folder:

| Computer | Usual place |
|---|---|
| Windows | `C:\Users\<you>\AppData\Roaming\CVBuilder` |
| Mac | `~/Library/Application Support/CVBuilder` |
| Linux | `~/.local/share/CVBuilder` |

To see the exact folder on your computer, open **File → Settings** and look
under **Data**. **Help → About CVBuilder** also says which mode you are using.

### API keys are not in this folder

If you connect an AI service, its key is kept in your computer's own keychain
(Windows Credential Manager, the macOS Keychain, or Secret Service on Linux), not
in the data folder and never in your CV file. See
[The AI assistant](ai-assistant.md).

## Next step

[Writing your CV](writing-your-cv.md)
