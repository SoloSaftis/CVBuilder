# CVBuilder design files (`.cvdesign.json`)

A design changes only how the CV looks: fonts, sizes, colours, the header and the section
headings, and optionally a sidebar. The text and its order stay the same in every design. One
column is read most reliably by applicant-tracking systems (ATS); designs with a sidebar are
marked «To spalter» in CVBuilder and the checklist warns about them.

Import one with **Design → Importer design …** in CVBuilder. **Design → Lagre mal-fil …** saves
the current design as a file to start from.

```json
{
  "format": "cvbuilder-design/1",
  "id": "min-design",
  "name": "Min design",
  "description": "Én setning om hvem designet passer for.",
  "credit": "",
  "body_font": "Calibri",
  "head_font": "Georgia",
  "size": 10.5,
  "name_size": 24,
  "name_bold": true,
  "name_upper": false,
  "name_spacing": 0,
  "accent": "#2F4A66",
  "ink": "#1F2C3A",
  "muted": "#5B6B7D",
  "rule": "#BECAD7",
  "header_align": "left",
  "header_fill": null,
  "header_text": null,
  "heading_style": "rule",
  "heading_fill": null,
  "heading_upper": true,
  "heading_small_caps": false,
  "heading_spacing": 0.5,
  "heading_size": 11,
  "date_style": "right",
  "company_italic": false,
  "gap": 1.0,
  "section_colors": { "experience": "#7A2638", "skills": "#0F5E66" }
}
```

| Field | Allowed values | What it does |
|---|---|---|
| `format` | `"cvbuilder-design/1"` | Required, so CVBuilder knows the file. |
| `id` | 2–40 of `a-z`, `0-9`, `-` | Saved in the CV file. An imported design with the id of a built-in one is refused. |
| `name`, `description` | text | Shown in the design list. |
| `credit` | text (optional) | Who made the design it is based on. |
| `body_font`, `head_font` | `Arial`, `Calibri`, `Cambria`, `Georgia`, `Garamond`, `Times New Roman`, `Verdana`, `Trebuchet MS`, `Tahoma`, `Palatino Linotype`, `Book Antiqua`, `Century Gothic`, `Segoe UI`, `Aptos` | Standard fonts that Word, PDF readers and ATS handle. |
| `size` | 9.5–12 | Body text size in points. |
| `name_size` | 16–34 | Name size in points. |
| `name_bold`, `name_upper` | true/false | Bold name; name in capitals. |
| `name_spacing` | 0–3 | Letter spacing in the name, points. |
| `accent`, `ink`, `muted`, `rule` | `"#RRGGBB"` | Headings and company names; body text; dates and contact line; lines. Body text must be dark enough to read (contrast at least 4.5:1 on white). |
| `header_align` | `left`, `center` | Name, headline and contact line. |
| `header_fill`, `header_text` | `"#RRGGBB"` or `null` | A coloured band behind the header, and its text colour. Both or neither. |
| `heading_style` | `rule` (line under), `rule_above` (line over), `double` (over and under), `bar` (bar on the left), `band` (shaded), `plain` | Section headings. |
| `heading_fill` | `"#RRGGBB"` or `null` | Background for `band`. |
| `heading_upper`, `heading_small_caps` | true/false | Capitals or small capitals in headings. |
| `heading_spacing` | 0–3 | Letter spacing in headings, points. |
| `heading_size` | 9–16 | Heading size in points. |
| `date_style` | `right` (same line, right-aligned), `below` (own line under the title) | Dates and places in experience and education. |
| `company_italic` | true/false | Company names in italics. |
| `gap` | 0.75–1.3 | Spacing between parts; lower fits more on a page. |
| `category` | `formell`, `teknisk`, `naeringsliv`, `kreativ`, `akademisk`, `kompakt`, `forstejobb` | What kind of job the design suits; the design list is grouped by it. |
| `columns` | `null` or an object (below) | A sidebar. Leave it out for one column. |
| `section_colors` | optional: section key → `"#RRGGBB"` | Its own colour for a section's heading, line and organisation names. Keys: `profile`, `skills`, `experience`, `education`, `certifications`, `languages`, `interests`. Contrast at least 3:1. |

`columns`: `{"side": "left"|"right", "width": 0.25–0.42, "fill": "#RRGGBB"|null, "text": "#RRGGBB"|null,
"heading": "#RRGGBB"|null, "sections": ["contact", "skills", …], "header": "top"|"main"|"side"}`.
`width` is the sidebar's share of the text width; `text` needs contrast 4.5:1 and `heading` 3:1 on
`fill`; `sections` are the section keys in the sidebar, plus `contact` to list the contact details
there; `header` puts the name across the top, at the top of the main column, or in the sidebar.

Things a design cannot do, on purpose: tables, text boxes, icons, skill bars, images other than
the optional portrait, or text in the page header or footer. Those make CVs harder for ATS to read.

Colours are independent of the design: in CVBuilder, **Farger** puts a palette (ready-made or your
own) over any design, and each section in the form has its own colour button. A design file's
colours are only its defaults.
