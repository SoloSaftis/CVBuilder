//! Designs. Same strict rules as the HTML builder: one text column, the same section names, real
//! text only (no tables, text boxes, icons or skill bars), contact details in the body, standard
//! fonts. A design only changes typography, colour and spacing, so the machine-read text is the
//! same in every design.
//!
//! Built-in designs are CVBuilder's own four plus remakes of open-source templates
//! (`designs.json`, each with its credit). People can import their own as `.cvdesign.json`
//! files; the format is in `DESIGN-FORMAT.md` and checked by [`parse`].
use std::collections::BTreeMap;
use std::sync::{LazyLock, RwLock};

use serde::{Deserialize, Serialize};
use wordcraft_doc::Rgb;

use crate::model::{ColumnsChoice, Palette, Project};

pub const FORMAT: &str = "cvbuilder-design/1";

/// Fonts a design may use: on every Windows and Mac with Office, and read correctly by ATS.
pub const FONTS: [&str; 14] =
    ["Arial", "Calibri", "Cambria", "Georgia", "Garamond", "Times New Roman", "Verdana", "Trebuchet MS", "Tahoma", "Palatino Linotype", "Book Antiqua", "Century Gothic", "Segoe UI", "Aptos"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingStyle {
    /// A thin rule under the heading.
    Rule,
    /// A thin rule over the heading.
    RuleAbove,
    /// Rules over and under.
    Double,
    /// A short bar to the left of the heading.
    Bar,
    /// A shaded band behind the heading.
    Band,
    /// No line.
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateStyle {
    /// On the title's line, right-aligned.
    Right,
    /// On its own line under the title.
    Below,
}

/// What kind of job a design suits: (id, label). The design list is grouped by these.
pub const CATEGORIES: [(&str, &str); 7] = [
    ("formell", "Formell · jus, finans, offentlig sektor, ledelse"),
    ("teknisk", "Teknisk · IT, ingeniør, industri"),
    ("naeringsliv", "Næringsliv · salg, konsulent, prosjekt"),
    ("kreativ", "Kreativ · design, media, markedsføring"),
    ("akademisk", "Akademisk · forskning, undervisning, helse"),
    ("kompakt", "Kompakt · lang erfaring, mye innhold"),
    ("forstejobb", "Første jobb · studenter og nyutdannede"),
];

pub fn category_label(id: &str) -> &'static str {
    CATEGORIES.iter().find(|(c, _)| *c == id).map(|(_, l)| *l).unwrap_or("Andre")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderPlace {
    /// Name and headline across the full width, above both columns.
    Top,
    /// At the top of the main column.
    Main,
    /// At the top of the sidebar.
    Side,
}

/// A sidebar: a second column for short sections (contact, skills, languages …). Two columns
/// look good but are read less reliably by older applicant-tracking systems, so CVBuilder warns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Columns {
    pub side: Side,
    /// The sidebar's share of the text width, 0.25–0.42.
    pub width: f32,
    #[serde(default)]
    pub fill: Option<String>,
    /// Text colour in the sidebar (with `fill`).
    #[serde(default)]
    pub text: Option<String>,
    /// Heading colour in the sidebar.
    #[serde(default)]
    pub heading: Option<String>,
    /// Section keys in the sidebar; `contact` lists the contact details there.
    pub sections: Vec<String>,
    pub header: HeaderPlace,
}

/// One design, exactly as stored in a `.cvdesign.json` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Design {
    pub format: String,
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub credit: String,
    pub body_font: String,
    pub head_font: String,
    /// Body size in points.
    pub size: f32,
    pub name_size: f32,
    pub name_bold: bool,
    #[serde(default)]
    pub name_upper: bool,
    #[serde(default)]
    pub name_spacing: f32,
    pub accent: String,
    pub ink: String,
    pub muted: String,
    pub rule: String,
    pub header_align: Align,
    #[serde(default)]
    pub header_fill: Option<String>,
    #[serde(default)]
    pub header_text: Option<String>,
    pub heading_style: HeadingStyle,
    #[serde(default)]
    pub heading_fill: Option<String>,
    pub heading_upper: bool,
    #[serde(default)]
    pub heading_small_caps: bool,
    #[serde(default)]
    pub heading_spacing: f32,
    pub heading_size: f32,
    pub date_style: DateStyle,
    #[serde(default)]
    pub company_italic: bool,
    /// Spacing multiplier.
    pub gap: f32,
    /// A colour per section (`experience`, `skills` …) for its heading, line and organisation
    /// names. Sections not listed use `accent`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub section_colors: BTreeMap<String, String>,
    /// What kind of job the design suits (see [`CATEGORIES`]).
    #[serde(default)]
    pub category: String,
    /// Two columns, or `None` for the single column ATS read best.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Columns>,
    /// Set by CVBuilder, not in files: true for designs the person imported.
    #[serde(skip)]
    pub imported: bool,
}

/// `#RRGGBB` → colour (black when unreadable; [`parse`] refuses such files).
pub fn rgb(hex: &str) -> Rgb {
    let h = hex.trim_start_matches('#');
    let byte = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0);
    Rgb(byte(0), byte(2), byte(4))
}

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// WCAG contrast ratio between two colours.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let lum = |c: Rgb| {
        let ch = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.039_28 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

const WHITE: Rgb = Rgb(255, 255, 255);

impl Design {
    pub fn heading(&self, text: &str) -> String {
        if self.heading_upper { text.to_uppercase() } else { text.to_string() }
    }

    /// Everything wrong with the design, in plain Norwegian. Empty when it can be used.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.format != FORMAT {
            out.push(format!("«format» må være «{FORMAT}»."));
        }
        if !(2..=40).contains(&self.id.len()) || !self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            out.push("«id» må ha 2–40 tegn: små bokstaver a–z, tall og bindestrek.".into());
        }
        if self.name.trim().is_empty() {
            out.push("«name» mangler.".into());
        }
        for (field, font) in [("body_font", &self.body_font), ("head_font", &self.head_font)] {
            if !FONTS.contains(&font.as_str()) {
                out.push(format!("«{field}»: «{font}» er ikke en av de tillatte skriftene ({}).", FONTS.join(", ")));
            }
        }
        let range = |out: &mut Vec<String>, field: &str, v: f32, lo: f32, hi: f32| {
            if !(lo..=hi).contains(&v) {
                out.push(format!("«{field}» må være mellom {lo} og {hi}."));
            }
        };
        range(&mut out, "size", self.size, 9.5, 12.0);
        range(&mut out, "name_size", self.name_size, 16.0, 34.0);
        range(&mut out, "name_spacing", self.name_spacing, 0.0, 3.0);
        range(&mut out, "heading_spacing", self.heading_spacing, 0.0, 3.0);
        range(&mut out, "heading_size", self.heading_size, 9.0, 16.0);
        range(&mut out, "gap", self.gap, 0.75, 1.3);
        let mut colors: Vec<(&str, &str)> = vec![("accent", &self.accent), ("ink", &self.ink), ("muted", &self.muted), ("rule", &self.rule)];
        for (field, value) in [("header_fill", &self.header_fill), ("header_text", &self.header_text), ("heading_fill", &self.heading_fill)] {
            if let Some(v) = value {
                colors.push((field, v));
            }
        }
        for (key, value) in &self.section_colors {
            colors.push(("section_colors", value));
            if !crate::text::SECTION_KEYS.contains(&key.as_str()) {
                out.push(format!("«section_colors»: «{key}» er ikke en seksjon ({}).", crate::text::SECTION_KEYS.join(", ")));
            }
        }
        if !self.category.is_empty() && !CATEGORIES.iter().any(|(c, _)| *c == self.category) {
            out.push(format!("«category» må være en av: {}.", CATEGORIES.iter().map(|(c, _)| *c).collect::<Vec<_>>().join(", ")));
        }
        if let Some(cols) = &self.columns {
            range(&mut out, "columns.width", cols.width, 0.25, 0.42);
            for k in &cols.sections {
                if k != "contact" && !crate::text::SECTION_KEYS.contains(&k.as_str()) {
                    out.push(format!("«columns.sections»: «{k}» er ikke en seksjon."));
                }
            }
            for (field, value) in [("columns.fill", &cols.fill), ("columns.text", &cols.text), ("columns.heading", &cols.heading)] {
                if let Some(v) = value {
                    colors.push((field, v));
                }
            }
            if let (Some(fill), true) = (&cols.fill, cols.fill.as_deref().is_some_and(is_hex)) {
                let text = cols.text.as_deref().unwrap_or(&self.ink);
                if is_hex(text) && contrast(rgb(fill), rgb(text)) < 4.5 {
                    out.push("«columns.text» er for svak mot «columns.fill» (kontrast under 4,5:1).".into());
                }
                let heading = cols.heading.as_deref().unwrap_or(&self.accent);
                if is_hex(heading) && contrast(rgb(fill), rgb(heading)) < 3.0 {
                    out.push("«columns.heading» er for svak mot «columns.fill» (kontrast under 3:1).".into());
                }
            }
        }
        let mut colors_ok = true;
        for (field, value) in colors {
            if !is_hex(value) {
                out.push(format!("«{field}» må være en farge som «#2F4A66»."));
                colors_ok = false;
            }
        }
        if self.header_fill.is_some() != self.header_text.is_some() {
            out.push("«header_fill» og «header_text» må settes sammen (eller begge være null).".into());
        }
        if self.heading_style == HeadingStyle::Band && self.heading_fill.is_none() {
            out.push("«heading_style» «band» trenger «heading_fill».".into());
        }
        if colors_ok {
            if contrast(rgb(&self.ink), WHITE) < 4.5 {
                out.push("«ink» er for lys til brødtekst (kontrast under 4,5:1 mot hvitt).".into());
            }
            if contrast(rgb(&self.muted), WHITE) < 3.0 {
                out.push("«muted» er for lys (kontrast under 3:1 mot hvitt).".into());
            }
            if let (Some(fill), Some(text)) = (&self.header_fill, &self.header_text) {
                if contrast(rgb(fill), rgb(text)) < 4.5 {
                    out.push("«header_text» er for svak mot «header_fill» (kontrast under 4,5:1).".into());
                }
            }
            let heading_bg = self.heading_fill.as_deref().filter(|_| self.heading_style == HeadingStyle::Band).map(rgb).unwrap_or(WHITE);
            if contrast(rgb(&self.accent), heading_bg) < 3.0 {
                out.push("«accent» er for svak for overskrifter (kontrast under 3:1).".into());
            }
            for (key, value) in &self.section_colors {
                if contrast(rgb(value), heading_bg) < 3.0 {
                    out.push(format!("«section_colors» «{key}» er for svak for overskrifter (kontrast under 3:1)."));
                }
            }
        }
        out
    }
}

fn own(id: &str, name: &str, description: &str, body: &str, head: &str) -> Design {
    Design {
        format: FORMAT.into(),
        id: id.into(),
        name: name.into(),
        description: description.into(),
        credit: String::new(),
        body_font: body.into(),
        head_font: head.into(),
        size: 10.5,
        name_size: 24.0,
        name_bold: true,
        name_upper: false,
        name_spacing: 0.0,
        accent: "#2F4A66".into(),
        ink: "#1F2C3A".into(),
        muted: "#5B6B7D".into(),
        rule: "#BECAD7".into(),
        header_align: Align::Left,
        header_fill: None,
        header_text: None,
        heading_style: HeadingStyle::Rule,
        heading_fill: None,
        heading_upper: true,
        heading_small_caps: false,
        heading_spacing: 0.0,
        heading_size: 11.0,
        date_style: DateStyle::Right,
        company_italic: false,
        gap: 1.0,
        section_colors: BTreeMap::new(),
        category: String::new(),
        columns: None,
        imported: false,
    }
}

/// CVBuilder's own four, unchanged from the HTML builder.
fn originals() -> Vec<Design> {
    let classic = own("classic", "Klassisk", "Rolig og tidløs. Passer de fleste bransjer.", "Arial", "Arial");
    let modern = Design {
        size: 10.5,
        name_size: 25.0,
        accent: "#1D6670".into(),
        ink: "#18262B".into(),
        muted: "#566A70".into(),
        rule: "#1D6670".into(),
        heading_style: HeadingStyle::Bar,
        gap: 1.02,
        ..own("modern", "Moderne", "Moderne skrift og tydelig fargeaksent. Teknologi og konsulent.", "Calibri", "Calibri")
    };
    let nordic = Design {
        name_size: 26.0,
        name_bold: false,
        accent: "#3E5C55".into(),
        ink: "#1D2826".into(),
        muted: "#5F6F6B".into(),
        rule: "#C9D3CF".into(),
        heading_upper: false,
        heading_size: 12.0,
        gap: 1.04,
        ..own("nordic", "Nordisk", "Serif i navn og overskrifter, dempede farger. Rådgivning, offentlig sektor, ledelse.", "Arial", "Georgia")
    };
    let compact = Design {
        size: 10.0,
        name_size: 21.0,
        accent: "#222222".into(),
        ink: "#1A1A1A".into(),
        muted: "#555555".into(),
        rule: "#9A9A9A".into(),
        heading_style: HeadingStyle::Plain,
        heading_size: 10.5,
        gap: 0.82,
        ..own("compact", "Kompakt", "Tettere og nøktern. Når lang erfaring må få plass på to sider.", "Arial", "Arial")
    };
    let set = |mut d: Design, c: &str| {
        d.category = c.into();
        d
    };
    vec![set(classic, "naeringsliv"), set(modern, "teknisk"), set(nordic, "formell"), set(compact, "kompakt")]
}

/// Every built-in design: the four originals first, then the open-source remakes.
pub static BUILT_IN: LazyLock<Vec<Design>> = LazyLock::new(|| {
    let mut all = originals();
    let remakes: Vec<Design> = serde_json::from_str(include_str!("designs.json")).unwrap_or_default();
    for d in remakes {
        if d.problems().is_empty() && !all.iter().any(|x| x.id == d.id) {
            all.push(d);
        }
    }
    all
});

static IMPORTED: RwLock<Vec<Design>> = RwLock::new(Vec::new());

/// Built-in and imported designs, in list order.
pub fn all() -> Vec<Design> {
    let mut out = BUILT_IN.clone();
    if let Ok(imported) = IMPORTED.read() {
        out.extend(imported.iter().cloned());
    }
    out
}

/// The design with this id; the classic one when it is unknown (e.g. an imported design that is
/// not on this computer).
pub fn design(id: &str) -> Design {
    all().into_iter().find(|d| d.id == id).unwrap_or_else(|| BUILT_IN[0].clone())
}

pub fn exists(id: &str) -> bool {
    all().iter().any(|d| d.id == id)
}

/// Replaces the list of imported designs (the app loads them from its data folder).
pub fn set_imported(designs: Vec<Design>) {
    if let Ok(mut list) = IMPORTED.write() {
        *list = designs.into_iter().filter(|d| !BUILT_IN.iter().any(|b| b.id == d.id)).map(|d| Design { imported: true, ..d }).collect();
    }
}

/// Reads a `.cvdesign.json` file. The error lists everything to fix, in plain Norwegian.
pub fn parse(text: &str) -> Result<Design, String> {
    let design: Design = serde_json::from_str(text).map_err(|e| format!("Filen er ikke en CVBuilder-design: {e}"))?;
    let mut problems = design.problems();
    if BUILT_IN.iter().any(|b| b.id == design.id) {
        problems.push(format!("«id» «{}» brukes allerede av et innebygd design. Velg en annen id.", design.id));
    }
    if problems.is_empty() { Ok(Design { imported: true, ..design }) } else { Err(problems.join("\n")) }
}

/// A design as a file people can edit and import again.
pub fn to_file(design: &Design) -> String {
    serde_json::to_string_pretty(design).unwrap_or_default()
}

impl Design {
    /// The colour of a section's heading, line and organisation names.
    pub fn section_accent(&self, key: &str) -> &str {
        self.section_colors.get(key).map(String::as_str).unwrap_or(&self.accent)
    }

    /// What a two-column design costs in applicant-tracking systems.
    pub fn ats_warning(&self) -> Option<&'static str> {
        self.columns.as_ref().map(|_| "Designet har to spalter. Det ser bra ut, men eldre søknadssystemer (ATS) kan lese spaltene i feil rekkefølge. Bruk et design med én spalte når du søker gjennom store portaler.")
    }

    /// Readability problems with the colours in use, in plain Norwegian (warnings, not errors:
    /// the person decides their colours, and ATS reads the text whatever its colour).
    pub fn color_warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if contrast(rgb(&self.ink), WHITE) < 4.5 {
            out.push("Brødteksten har for svak farge til å leses godt. Velg en mørkere tekstfarge.".into());
        }
        if contrast(rgb(&self.muted), WHITE) < 3.0 {
            out.push("Datoer og kontaktlinjen har for svak farge. Velg en mørkere farge for detaljer.".into());
        }
        let heading_bg = self.heading_fill.as_deref().filter(|_| self.heading_style == HeadingStyle::Band).map(rgb).unwrap_or(WHITE);
        let weak: Vec<&str> = std::iter::once(("", self.accent.as_str())).chain(self.section_colors.iter().map(|(k, v)| (k.as_str(), v.as_str()))).filter(|(_, c)| contrast(rgb(c), heading_bg) < 3.0).map(|(k, _)| k).collect();
        if !weak.is_empty() {
            out.push("En eller flere overskriftsfarger er for lyse. Velg mørkere farger, så overskriftene leses godt.".into());
        }
        if let (Some(fill), Some(text)) = (&self.header_fill, &self.header_text) {
            if contrast(rgb(fill), rgb(text)) < 4.5 {
                out.push("Teksten i fargefeltet øverst leses dårlig mot bakgrunnen.".into());
            }
        }
        out
    }
}

fn hex_or_none(v: &str) -> bool {
    v == "none" || is_hex(v)
}

/// The design the CV uses, with the person's palette, section colours and page setup on top.
pub fn effective(project: &Project) -> Design {
    let mut d = design(&project.options.layout);
    apply_palette(&mut d, &project.options.palette);
    let page = &project.options.page;
    let delta = page.text_delta.clamp(-1.5, 2.0);
    if delta != 0.0 {
        d.size += delta;
        d.heading_size += delta;
        d.name_size += delta * 2.0;
    }
    d.gap = (d.gap * page.spacing.clamp(0.8, 1.3)).clamp(0.6, 1.6);
    match page.columns {
        Some(ColumnsChoice::One) => d.columns = None,
        Some(choice) => {
            let side = if choice == ColumnsChoice::Left { Side::Left } else { Side::Right };
            match &mut d.columns {
                Some(c) => c.side = side,
                None => {
                    // A light tint of the main colour behind the sidebar.
                    let a = rgb(&d.accent);
                    let mix = |v: u8| (f32::from(v) * 0.1 + 255.0 * 0.9).round() as u8;
                    d.columns = Some(Columns {
                        side,
                        width: 0.32,
                        fill: Some(format!("#{:02X}{:02X}{:02X}", mix(a.0), mix(a.1), mix(a.2))),
                        text: None,
                        heading: None,
                        sections: ["contact", "skills", "languages", "certifications", "interests"].iter().map(|s| s.to_string()).collect(),
                        header: HeaderPlace::Main,
                    });
                }
            }
        }
        None => {}
    }
    for (key, value) in &project.options.section_colors {
        if is_hex(value) {
            d.section_colors.insert(key.clone(), value.clone());
        }
    }
    d
}

/// Puts a palette over a design. Unreadable colour values are ignored.
pub fn apply_palette(d: &mut Design, p: &Palette) {
    let set = |field: &mut String, v: &Option<String>| {
        if let Some(v) = v.as_deref().filter(|v| is_hex(v)) {
            *field = v.to_string();
        }
    };
    set(&mut d.accent, &p.accent);
    set(&mut d.ink, &p.ink);
    set(&mut d.muted, &p.muted);
    set(&mut d.rule, &p.rule);
    if p.accent.as_deref().is_some_and(is_hex) {
        // A new main colour replaces the design's own section colours too.
        d.section_colors.clear();
    }
    match p.header_fill.as_deref() {
        Some("none") => {
            d.header_fill = None;
            d.header_text = None;
        }
        Some(v) if is_hex(v) => {
            d.header_fill = Some(v.to_string());
            d.header_text = Some(p.header_text.clone().filter(|t| is_hex(t)).unwrap_or_else(|| if contrast(rgb(v), WHITE) >= 4.5 { "#FFFFFF".into() } else { "#1A1A1A".into() }));
        }
        _ => {
            if let (Some(t), Some(_)) = (p.header_text.as_deref().filter(|t| is_hex(t)), &d.header_fill) {
                d.header_text = Some(t.to_string());
            }
        }
    }
    match p.heading_fill.as_deref() {
        Some("none") => {
            if d.heading_style == HeadingStyle::Band {
                d.heading_style = HeadingStyle::Plain;
            }
            d.heading_fill = None;
        }
        Some(v) if is_hex(v) => {
            d.heading_fill = Some(v.to_string());
            d.heading_style = HeadingStyle::Band;
        }
        _ => {}
    }
}

/// Whether a palette value can be used: a colour, or `none` for the bands.
pub fn palette_value_ok(v: &str) -> bool {
    hex_or_none(v)
}

/// Ready-made palettes, each readable on white. «Designets egne» (empty) keeps the design's colours.
pub fn palettes() -> Vec<(&'static str, Palette)> {
    let p = |accent: &str, ink: &str, muted: &str, rule: &str| Palette { accent: Some(accent.into()), ink: Some(ink.into()), muted: Some(muted.into()), rule: Some(rule.into()), ..Default::default() };
    vec![
        ("Designets egne", Palette::default()),
        ("Marine", p("#1F3A5F", "#1B2430", "#55606E", "#B9C4D2")),
        ("Petrol", p("#0F5E66", "#162629", "#4F6467", "#A9C8CB")),
        ("Skog", p("#2E5E3E", "#1C2620", "#56645B", "#B7CBBD")),
        ("Vin", p("#7A2638", "#2A1C20", "#6A5458", "#D9B9C1")),
        ("Plomme", p("#5B3270", "#231B29", "#62566A", "#CDBBD8")),
        ("Terrakotta", p("#9A4A26", "#2A201B", "#6B5A52", "#E1C3B4")),
        ("Kobolt", p("#2346B8", "#1A1F2E", "#525B73", "#BCC8EE")),
        ("Grafitt", p("#333333", "#1A1A1A", "#5C5C5C", "#BDBDBD")),
        ("Sort og hvitt", p("#000000", "#000000", "#444444", "#000000")),
    ]
}
