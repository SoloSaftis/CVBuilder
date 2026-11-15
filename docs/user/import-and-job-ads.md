# Importing an old CV and using the job ad

[Back to the user guide](README.md)

## Importing a CV you already have

If you already have a CV, CVBuilder can read it and fill in the form for you.

1. Choose **File → Import CV from file**, or **Home → Import CV**.
2. Choose **With AI** or **Without AI** (see below).
3. Click **Choose file** and pick your CV.

You get a new CV with the design and colours you have chosen. The CV you had
before can be brought back with **Undo**.

### File types that can be read

- PDF with real text (not a scanned picture)
- Word (`.docx`, `.doc`, and also `.docm`, `.dotx`)
- OpenDocument (`.odt`, `.fodt`)
- RTF
- Apple Pages (`.pages`)
- Text (`.txt`, `.md`)
- Web page (`.html`, `.htm`, `.mht`, `.mhtml`)
- A CVBuilder draft (`.json`) – opens as it is

Files over 50 MB are not read.

### Without AI

- Everything is read on your computer. Nothing is sent anywhere.
- CVBuilder fills in what it can recognise with confidence: contact details,
  profile, jobs, education, skills, languages, courses, interests, and
  sections such as projects.
- It never makes anything up. Every value comes from your own file.
- Anything it could not place is shown at the end in a window called
  **This still needs a place**. You can **Copy the text** and paste it into the
  form yourself, or, if an AI is connected, click **Let the AI add it**.

Check the result. Dates and bullet points in unusual layouts may need a quick fix.

### With AI

This option is only available when an AI is connected (see
[The AI assistant](ai-assistant.md)).

- The text of your CV is sent to the AI service you connected.
- The AI fills in every part, including jobs with dates and bullet points.
- You see the changes in the preview as it works, and you can undo them.
- At the end it tells you what you should check.

### If a file will not import

- **Scanned PDF**: a scanned page is a picture, not text. Open the original
  document instead, or copy the text and paste it in.
- **Password-protected PDF or Word file**: remove the password, or copy the
  text and paste it in.
- **Old or damaged file**: open it in Word (or Pages) and save it again as
  `.docx` or PDF.

## Using the job ad

A CV that uses the job ad's own words does better with both recruiters and
applicant tracking systems (ATS). The first card in the form,
**The job ad you are applying to**, is for this. The ad itself is never printed
in your CV.

### Three ways to bring in the ad

1. **Paste it.** Copy the whole ad from the web page and paste it into
   **Ad text** (Ctrl+V). When you paste a whole ad at once, CVBuilder reads it
   right away. You can also click **Fill in from the ad**.
   Shortcut: **Home → Paste ad** takes you to the box.
2. **Fetch it from a link.** Paste the ad's address into **Link to the ad** and
   click **Fetch the ad** (or **Home → Fetch from link**). CVBuilder downloads
   that one page. Nothing else is sent.
3. **Read it from a file.** Click **From file** (or **Home → From file**) and
   pick a PDF, Word, text file or a saved web page.

### What CVBuilder fills in

From the ad, CVBuilder fills in, without AI:

- **Job title** – the job you are applying for.
- **Employer** – the company.
- **Important keywords from the ad** – one per line, up to 30.

It also looks through your CV. If a keyword from the ad already appears in your
CV (for example in a job description) but is not in your skills list, it is
added to your skills in a group called "Fra annonsen" ("From the ad"). You can
rename the group or move the skills.

What you typed yourself is kept. If you paste a different ad later, what
CVBuilder filled in from the old ad makes way for the new one.

A note under the ad box says what was filled in. **Undo** brings back the CV as
it was before.

### When fetching does not work

Some job sites cannot be fetched:

- **The page needs sign-in.** Open the ad in your browser, copy the text and
  paste it in.
- **The page shows almost no text.** Some sites build the page with
  JavaScript. Open it in your browser, copy the text and paste it in.
- The link must start with `https://` (CVBuilder adds it if you leave it out).

## Keywords

Keywords are the skills, tools, qualifications and job words the employer is
looking for.

- You can edit the list in **Important keywords from the ad** – one keyword per
  line.
- With an AI connected, **Find keywords with AI** reads the ad and saves up to
  25 of the most important keywords.

### Seeing which keywords are missing

Open the **Checklist** on the right (see
[The checklist and downloading](checklist-and-export.md)). Under
**Keywords from the ad** you see:

- **green** keywords – already in your CV,
- **orange** keywords – in the ad, but not in your CV.

Add a missing keyword only if it is true for you. With an AI connected,
**Ask the AI about the missing keywords** starts a conversation where the AI asks
which ones you can honestly include, before anything is added.

### Tailoring the whole CV

With an AI connected, **Tailor the CV with AI** (on the ad card) goes through
your CV against the ad. It uses the ad's words where they match what your CV
already says, brings the most relevant points forward, and never adds anything
you do not have. At the end it tells you what you could add yourself.

## Next step

[The checklist and downloading](checklist-and-export.md)
