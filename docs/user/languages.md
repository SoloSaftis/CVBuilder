# Languages

[Back to the user guide](README.md)

CVBuilder has two separate language settings:

- **The menu language** – the language of the menus, buttons and messages.
- **The CV's language** – the language of your CV's headings ("Work
  experience", "Education" …) and dates ("09.2021 – now").

They do not depend on each other. You can have English menus and a French CV,
or Polish menus and an English CV.

Both are chosen for you on the first start, from your computer's language. See
[The first start](getting-started.md#the-first-start).

## The menu language

Change it in **File → Settings → Menu language**.

- **Follow system** uses your computer's language when CVBuilder has it, and
  English otherwise.
- **86 menu languages** are included: Norwegian (bokmål), English, and 84 more,
  from Afrikaans to Vietnamese, including Arabic, Chinese, Hindi, Japanese,
  Korean, Ukrainian and many others.
- The menus change right away. Nothing in your CV changes.

In added languages, a few messages with numbers and names in them (for example
some checklist messages) are shown in English.

## The CV's language

Change it on the **Home** tab, in the **Language** group. The list shows your
computer's language first, then all languages from A to Z.

- **86 CV languages** have ready-made headings and dates, and all of them work
  offline.
- When you pick a language, the **headings and dates change right away**.
- **Your own text stays as it was.** Your profile, job titles and bullet points
  are not translated automatically. Under each field, CVBuilder shows the
  original text ("Original before the language switch") to help you translate.
- **Undo** brings back the CV as it was before the switch.

A new CV starts in the CV language you chose at the first start.

## Translating the CV

Click **Home → Translate** (or choose **Other language …** at the bottom of the
language list). The **Translate the CV** window opens.

1. Choose a language from the list, or type any language in the box next to it,
   written in that language, for example "Kiswahili" or "日本語".
2. For a language in the list, the headings are ready. For any other language,
   the headings must be written: click **Headings in the CV** and fill them in,
   or let the AI do it.
3. Choose how to translate the text.

### With AI

If an AI is connected, click **Translate with AI**.

- The AI translates all your text and the headings: headline, profile, every job
  title and bullet point, skills, education, courses, languages, interests and
  your own sections.
- It keeps all facts, company names and numbers.
- You see the changes as it works, and you can undo them in the chat.

Your CV text is sent to the AI service you connected. See
[The AI assistant](ai-assistant.md).

### Without AI

Click **Only switch headings**.

- The headings and dates change now.
- You translate the text yourself. The original text is shown under each field
  while you write.

If no AI is connected, the window says **AI is not connected** and offers
**Connect AI** as well.

### Tip: keep one file per language

If you apply in more than one language, save a separate file for each, for
example with **File → Save as**. Then you can keep both versions up to date.

## Adding a menu language

If your language is not among the 86, you can add it. This is also how you can
correct or improve a language.

Open **File → Settings → Menu language**. There you find these buttons:

- **Add a language with AI …** – choose or type a language, its name in that
  language, and its code (for example `de`, `pl` or `pt-BR`). Click
  **Translate**. The connected AI translates every text in CVBuilder. You can
  follow the progress in Settings and at the bottom of the window, and **Stop**
  at any time; what is done is kept.
- **Import language file …** – add a language from a `.cvlang.json` file, for
  example one a friend made.
- **Export a template for translation …** – save a file with every text in
  English. Translate the values in a text editor, fill in `"code"` and `"name"`,
  and import it. Keep the words in `{curly braces}` as they are.
- **Edit …** – fix texts in the chosen language yourself. English is on the
  left, the translation on the right. An empty box is shown in English.
- **Make again** – let the AI translate the chosen language again.
- **Export the language …** – save the chosen language as a file to share it.
- **Reset language** / **Delete language** – undo your own changes to a
  language that comes with CVBuilder, or remove a language you added.

Language files are kept in the `languages` folder in CVBuilder's data folder.

When you translate a menu language with AI, only CVBuilder's own menu texts are
sent to the AI service. Your CV is not sent.

## Next step

[The AI assistant](ai-assistant.md)
