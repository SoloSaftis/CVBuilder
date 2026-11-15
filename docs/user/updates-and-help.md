# Updates and help

[Back to the user guide](README.md)

## Checking for updates

CVBuilder never checks for updates by itself, and never downloads anything by
itself. You check when you want to.

1. Choose **Help → Check for updates** (also in the **File** menu).
2. CVBuilder asks GitHub for the number of the newest version. Nothing about you
   or your CV is sent.
3. The window tells you one of these:
   - "You have the newest version (1.0.1)."
   - "CVBuilder 1.x.x is out. You have 1.0.1." Click **Open the download page**
     to get it in your browser.
   - "Could not check: …" – for example when you are offline. You can still
     open the download page.

### Installing a new version

- **Windows zip:** download the new zip and unzip it. To keep your drafts and
  settings, copy your old `CVBuilder-data` folder into the new folder (or unzip
  the new program files over the old folder).
- **Mac:** download the new zip and drag CVBuilder into Applications again.
  Replace the old one.
- **Linux AppImage:** download the new file and use it instead of the old one.
- **Linux folder version:** unpack it and copy your old `CVBuilder-data` folder
  into the new folder.

Your saved CV files (`.json`) open in the new version as before.

### Microsoft Store

If you installed CVBuilder from the Microsoft Store, the Store keeps it up to
date by itself. **Check for updates** says so, and offers **Open in the
Microsoft Store** instead of the download page.

## The About window

**Help → About CVBuilder** (also in the **File** menu) shows:

- **Version** – for example 1.0.1,
- **Build (git)** – the exact source code the program was built from,
- **System** – your operating system and processor type,
- **Data** – whether CVBuilder runs in portable mode or uses your user data
  folder,
- the licences: MIT or Apache-2.0, and the components CVBuilder builds on.

**Copy version info** copies the version, build and system in one line. Paste it
into a problem report.

## Getting help

- **Tips everywhere:** rest the mouse pointer on any button or field to see what
  it does.
- **The status bar** at the bottom tells you what just happened, and what to do
  next.
- **This guide:** see the [user guide](README.md) and
  [Troubleshooting](troubleshooting.md).

## Reporting a problem

Report problems and ideas on GitHub:
<https://github.com/SoloSaftis/CVBuilder/issues>

1. Click **New issue** (you need a free GitHub account).
2. Describe what you did, what you expected, and what happened instead.
3. Paste the line from **Help → About CVBuilder → Copy version info**.
4. A screenshot helps.

**Never attach your real CV or personal details** to an issue. Issues are
public. If a CV file is needed to show the problem, use the example CV or
made-up details.

## Privacy

CVBuilder has no account, no analytics and no advertising. Read the full
[privacy policy](../../PRIVACY.md).

## Next step

[Troubleshooting](troubleshooting.md)
