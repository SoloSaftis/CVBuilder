# Designs and colours

[Back to the user guide](README.md)

A design changes only how your CV looks: fonts, sizes, colours, the header and
the headings. The text and its order stay the same in every design. So every
design is read the same way by applicant tracking systems (ATS).

## Choosing a design

Open the **Design** tab and click the list that says **Design: …**.

- There are **31 one-column designs**, grouped by the kind of job they suit:
  - **Formal** – law, finance, public sector, management
  - **Technical** – IT, engineering, industry
  - **Business** – sales, consulting, projects
  - **Creative** – design, media, marketing
  - **Academic** – research, teaching, health
  - **Compact** – long experience, lots of content
  - **First job** – students and recent graduates
- Each design has a short description. Rest the pointer on a design to see where
  it comes from.
- Some designs also come in a **Two columns** version, marked with an orange
  label. They look good, but older applicant tracking systems may read the two
  columns in the wrong order. Use a one-column design when you apply through
  large job portals.
- Designs you imported yourself are listed under **Your designs**.

Four designs are CVBuilder's own: **Klassisk** (Classic), **Moderne** (Modern),
**Nordisk** (Nordic) and **Kompakt** (Compact). Kompakt fits the most text per
page. The others are remakes of open-source templates from Reactive Resume and
RenderCV, adapted to one column.

All designs use standard fonts that are on most computers and that ATS read
correctly: Arial, Calibri, Cambria, Georgia, Garamond, Times New Roman, Verdana,
Trebuchet MS, Tahoma, Palatino Linotype, Book Antiqua, Century Gothic, Segoe UI
and Aptos.

Switching design never changes your text. You can try as many as you like.

## Colours

Colours are separate from the design. If you switch design, the colours you
chose are kept.

### Colours for the whole CV

On the **Design** tab, click **Colors**. You can:

- pick one of the **Ready-made palettes**: Navy, Petrol, Forest, Wine, Plum,
  Terracotta, Cobalt, Graphite, or Black and white. **The design's own** keeps
  the design's colours.
- set **Your colors** one by one:
  - **Main color** – headings and details that stand out,
  - **Body text** – ordinary text,
  - **Dates and details** – dates, places and other helper text,
  - **Lines** – the dividing lines,
  - **Color band at the top** – a coloured band behind your name and contact
    details (tick to turn on), and **Text in the color band**,
  - **Band behind headings** – a coloured band behind each section heading.
- click **Design** next to a colour to go back to the design's own colour.
- click **Back to the design's colors** to remove all your own colours,
  including the sections' colours.

When you have chosen your own colours, the button says **Colors · custom**.

### A colour for one section

Each section in the form has a small colour button on its header.

- Click it and pick a colour. The section's heading, line and organisation
  names get that colour.
- Right-click the button to use the main colour again.

### Keep it readable

Dark text on a light background is easiest to read. If a colour is too light,
the **Before download** card in the checklist warns you, for example "The body
text is too faint". The warning is advice; you decide your colours.

## Your own designs

You can make your own design as a file and import it. A design file is a small
text file that ends in `.cvdesign.json`.

### The easiest way to start

1. Choose a design you like.
2. Choose **Design → Save as template file** (or, in the design list,
   **Save this design as a template file …**).
3. Open the file in a text editor (such as Notepad or TextEdit) and change the
   fonts, sizes or colours.
4. Change `"id"` and `"name"` so it does not clash with a built-in design.
5. Choose **Design → Import design**, then **Choose file …**, and pick your file.

The design is checked when you import it. If something is wrong, CVBuilder lists
what to fix. A good design is copied into CVBuilder's data folder, chosen right
away, and listed under **Your designs** from then on.

### What a design file can set

**Help → Format for your own designs** shows a short list of every field. In
brief:

- the body and heading fonts (from the list above),
- the text size (9.5–12 pt) and name size (16–34 pt),
- colours, written like `#2F4A66`, for headings, body text, dates and lines
  (body text needs enough contrast),
- the header: left or centred, with or without a colour band,
- the heading style: a line under, a line over, double lines, a bar, a band,
  or plain,
- where dates go: on the same line, right-aligned, or on their own line,
- the space between parts,
- optionally a sidebar (two columns). Leave it out to keep the one column ATS
  read best.

Not possible, on purpose: tables, text boxes, icons, skill bars and text in the
page header or footer. They make CVs harder for ATS to read.

The full format, with an example file, is in
[DESIGN-FORMAT.md](../../crates/cv-core/DESIGN-FORMAT.md).

## Next step

[Languages](languages.md)
